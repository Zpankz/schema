//! Tests for the significant improvement: the serializable, interpretable
//! RuleProgram world model; the JSON deliberation protocol; session
//! persistence; and an end-to-end run where the world program is authored
//! entirely via JSON (the working jcode-provider seam).

use schema::agent::SchemaAgent;
use schema::demo_provider::toggle_provider;
use schema::envs::{CartDock, ToggleMaze};
use schema::executor::Environment;
use schema::model::WorldModel;
use schema::program::{CellWrite, Effect, Rule, RuleProgram};
use schema::protocol::{DeliberationResponse, ProtocolDeliberator};
use schema::run_backtest;
use schema::session::Session;
use schema::timeline::{Grid, Timeline};

fn toggle_program() -> RuleProgram {
    RuleProgram {
        rules: vec![
            Rule::new(0, Effect::advance()),
            Rule::new(3, Effect::advance_win()),
            Rule::new(7, Effect::advance()),
        ],
        goal_absent_colors: vec![],
    }
}

fn cart_program() -> RuleProgram {
    let push = Effect {
        player_advances: true,
        push: true,
        writes: vec![CellWrite { x: 3, y: 2, color: 0 }],
        writes_when_beyond: Some(6),
        ..Effect::default()
    };
    RuleProgram {
        rules: vec![
            Rule::new(0, Effect::advance()),
            Rule::new(3, Effect::advance_win()),
            Rule { on_target: 4, beyond_any_of: Some(vec![0, 6]), note: String::new(), effect: push },
        ],
        goal_absent_colors: vec![],
    }
}

// -- RuleProgram JSON round-trips ------------------------------------------

#[test]
fn ruleprogram_json_roundtrips() {
    for p in [toggle_program(), cart_program(), RuleProgram::vacuous()] {
        let json = p.to_json();
        let back = RuleProgram::from_json(&json).expect("parse");
        assert_eq!(p, back, "round-trip must be identity");
    }
}

// -- Interpreter reproduces the native envs exactly ------------------------

fn cross_check<E: Environment>(mut env: E, prog: &RuleProgram, seq: &[&str]) -> usize {
    let mut cur: Grid = env.reset().grid;
    let mut mism = 0;
    for &a in seq {
        let pred = prog.step(&cur, a);
        let obs = env.step(a);
        let terminal = obs.win || obs.dead || obs.level_up;
        let flags_ok = pred.win == obs.win && pred.dead == obs.dead && pred.level_up == obs.level_up;
        let grid_ok = terminal || pred.grid == obs.grid;
        if !(flags_ok && grid_ok) {
            mism += 1;
        }
        cur = obs.grid;
        if terminal {
            break;
        }
    }
    mism
}

#[test]
fn interpreter_matches_toggle_maze() {
    // includes a wall bump (up) and a redundant left, then the winning line
    let seq = ["up", "right", "left", "right", "right", "right"];
    assert_eq!(cross_check(ToggleMaze::new(), &toggle_program(), &seq), 0);
}

#[test]
fn interpreter_matches_cart_dock_including_reflow() {
    // push cart onto dock (opens gate at (3,2)), then descend to the goal
    let seq = ["right", "right", "right", "right", "down", "down"];
    assert_eq!(cross_check(CartDock::new(), &cart_program(), &seq), 0);
}

// -- program diff ----------------------------------------------------------

#[test]
fn program_diff_detects_added_and_changed_rules() {
    let base = RuleProgram {
        rules: vec![Rule::new(0, Effect::advance()), Rule::new(3, Effect::advance_win())],
        goal_absent_colors: vec![],
    };
    let revised = toggle_program(); // adds the color-7 rule
    let d = revised.diff(&base);
    assert!(d.iter().any(|l| l.contains("+ rule on color 7")), "diff: {d:?}");

    // change an effect
    let mut changed = base.clone();
    changed.rules[0].effect = Effect { dead: true, ..Effect::default() };
    let d2 = changed.diff(&base);
    assert!(d2.iter().any(|l| l.contains("~ rule on color 0")), "diff: {d2:?}");
}

// -- ProtocolDeliberator parses JSON and applies the revised program -------

#[test]
fn protocol_deliberator_applies_json_program() {
    let resp = DeliberationResponse {
        revised_program: Some(toggle_program()),
        intent: "plan".into(),
        actions: vec![],
        note: "install toggle program".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();

    let mut env = ToggleMaze::new();
    let first = env.reset();
    let mut model = WorldModel::from_program(RuleProgram::vacuous(), "vacuous");
    let mut delib = ProtocolDeliberator::new(move |_prompt: &str| json.clone());
    // drive one deliberation turn directly
    use schema::agent::{Deliberator, Intent};
    let tl = Timeline::new();
    let bt = run_backtest(&model, &tl);
    let d = delib.deliberate(&first.grid, &mut model, &tl, &bt, None);
    assert_eq!(d.intent, Intent::Plan);
    assert!(model.program().unwrap().rules.iter().any(|r| r.on_target == 7));
    assert_eq!(model.revision, 1);
    assert_eq!(delib.transcript.len(), 1);
}

// -- Session save/load round-trip ------------------------------------------

#[test]
fn session_roundtrips_mid_game() {
    // play a few real steps to build a timeline, with a program-backed model
    let mut env = ToggleMaze::new();
    let mut cur = env.reset().grid;
    let mut tl = Timeline::new();
    let model = WorldModel::from_program(toggle_program(), "toggle");
    for a in ["right", "right"] {
        let obs = env.step(a);
        tl.append(cur.clone(), a, obs.grid.clone(), obs.level_up, obs.dead, obs.win);
        cur = obs.grid;
    }
    let before = run_backtest(&model, &tl);
    assert!(before.green());

    let snap = Session::capture("toggle demo", &model, &tl);
    let json = snap.to_json();
    let restored = Session::from_json(&json).expect("parse session");

    let model2 = restored.restore_model();
    let tl2 = restored.restore_timeline();
    assert_eq!(tl2.len(), tl.len());
    assert_eq!(model2.revision, model.revision);
    assert_eq!(model2.program(), model.program());
    // the resumed model certifies the resumed timeline identically
    let after = run_backtest(&model2, &tl2);
    assert_eq!(after.summary(), before.summary());
    assert!(after.green());
}

// -- End-to-end: the world program is authored entirely via JSON -----------

#[test]
fn protocol_driven_discovery_wins() {
    let mut env = ToggleMaze::new();
    env.reset();
    let delib = ProtocolDeliberator::new(toggle_provider);
    let model = WorldModel::from_program(RuleProgram::vacuous(), "vacuous program");
    let mut agent = SchemaAgent::new(env, model, delib);
    let log = agent.run();

    assert!(log.won, "protocol-driven run must win");
    // discovery happened through committed experiments (probes)
    assert!(log.executions.iter().any(|e| e.contains("[experiment]")));
    // the winning commit was a certified plan with zero mispredictions
    let last = log.executions.last().unwrap();
    assert!(last.contains("[plan]") && last.contains("0 mispredictions") && last.ends_with("WIN"));
    // the final model is certified N/N exact over the full history
    let bt = run_backtest(&agent.model, &agent.timeline);
    assert!(bt.green());
    // and the model it converged on is an interpretable, text-readable program
    let prog = agent.model.program().expect("program-backed");
    assert!(prog.rules.iter().any(|r| r.on_target == 7 && r.effect.player_advances));
    // every deliberation turn was a real JSON exchange
    assert!(agent.deliberator.transcript.len() >= 2);
    for (req, resp) in &agent.deliberator.transcript {
        assert!(serde_json::from_str::<serde_json::Value>(req).is_ok(), "request is JSON");
        assert!(serde_json::from_str::<serde_json::Value>(resp).is_ok(), "response is JSON");
    }
}
