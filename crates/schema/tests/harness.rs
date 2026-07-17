//! Tests porting the six base criterion clauses from the countersigned
//! Python reference (regulate runs 591204fc / 42a6b6d5 / f91d4d23).

use schema::agent::SchemaAgent;
use schema::backtest::{run_backtest, MismatchKind};
use schema::deliberators::{CartDeliberator, ToggleDeliberator};
use schema::envs::{CartDock, ToggleMaze, ACTIONS};
use schema::executor::{commit_actions, Environment};
use schema::model::{Prediction, WorldModel};
use schema::planner::{run_bfs, GoalVia};
use schema::timeline::{Grid, Timeline};

fn true_toggle_model() -> WorldModel {
    // Mirror of ToggleMaze's hidden rule, used to build a "correct" model.
    WorldModel::new(
        Box::new(|state: &Grid, action: &str| {
            let (px, py) = schema::envs::find_color(state, 1)[0];
            let (dx, dy) = schema::envs::delta(action);
            let (tx, ty) = ((px as i32 + dx) as usize, (py as i32 + dy) as usize);
            let mut g = state.clone();
            match state[ty][tx] {
                0 | 7 => {
                    g[py][px] = 0;
                    g[ty][tx] = 1;
                    Prediction::of(g)
                }
                3 => {
                    g[py][px] = 0;
                    g[ty][tx] = 1;
                    Prediction::win(g)
                }
                _ => Prediction::of(g),
            }
        }),
        Box::new(|_| false),
        "true toggle model",
    )
}

fn wrong_toggle_model() -> WorldModel {
    // Deliberately wrong: believes 7 is a wall (no-op).
    WorldModel::new(
        Box::new(|state: &Grid, action: &str| {
            let (px, py) = schema::envs::find_color(state, 1)[0];
            let (dx, dy) = schema::envs::delta(action);
            let (tx, ty) = ((px as i32 + dx) as usize, (py as i32 + dy) as usize);
            let mut g = state.clone();
            match state[ty][tx] {
                0 => {
                    g[py][px] = 0;
                    g[ty][tx] = 1;
                    Prediction::of(g)
                }
                3 => {
                    g[py][px] = 0;
                    g[ty][tx] = 1;
                    Prediction::win(g)
                }
                _ => Prediction::of(g), // 7 treated as blocking
            }
        }),
        Box::new(|_| false),
        "wrong toggle model",
    )
}

fn record_walk(env: &mut ToggleMaze, timeline: &mut Timeline, actions: &[&str]) -> Grid {
    let obs = env.reset();
    let mut cur = obs.grid;
    for &a in actions {
        let obs = env.step(a);
        timeline.append(cur.clone(), a, obs.grid.clone(), obs.level_up, obs.dead, obs.win);
        cur = obs.grid;
    }
    cur
}

// -- (1) Timeline is append-only (type-enforced) ---------------------------

#[test]
fn timeline_is_append_only_by_construction() {
    // In Rust the guarantee is structural: Timeline exposes no &mut access
    // to recorded transitions and no removal API. This test asserts the
    // surface: records persist identically across further appends, and the
    // public API is read-only (which the compiler enforces — attempts to
    // write `timeline.get(0).state_before = ...` or call a `pop` simply do
    // not compile; see docs/SCHEMA_HARNESS.md for the compile-fail notes).
    let mut env = ToggleMaze::new();
    let mut tl = Timeline::new();
    record_walk(&mut env, &mut tl, &["right", "left"]);
    let first = tl.get(0).unwrap().clone();
    record_walk(&mut env, &mut tl, &["right"]);
    assert_eq!(tl.get(0).unwrap(), &first);
    assert_eq!(tl.len(), 3);
}

// -- (2) Backtest: exact N/N for correct model, first mismatch for wrong ---

#[test]
fn backtest_exact_for_correct_model() {
    let mut env = ToggleMaze::new();
    let mut tl = Timeline::new();
    record_walk(&mut env, &mut tl, &["right", "right", "right", "right"]); // win on last
    let rep = run_backtest(&true_toggle_model(), &tl);
    assert!(rep.green());
    assert_eq!(rep.total, 4);
    assert_eq!(rep.skipped, 1); // terminal win step
    assert_eq!(rep.exact, rep.total - rep.skipped);
    assert!(rep.summary().contains("0 mismatch(es)"));
}

#[test]
fn backtest_reports_first_mismatch_for_wrong_model() {
    let mut env = ToggleMaze::new();
    let mut tl = Timeline::new();
    record_walk(&mut env, &mut tl, &["right", "right", "right"]); // 3rd enters color 7
    let rep = run_backtest(&wrong_toggle_model(), &tl);
    assert!(!rep.green());
    let fm = rep.first_mismatch().expect("mismatch expected");
    assert_eq!(fm.index, 1); // the step onto 7 mispredicted
    assert_eq!(fm.kind, MismatchKind::Grid);
    assert!(fm.detail.contains("cell ("));
}

// -- (3) commit_actions halts on first misprediction -----------------------

#[test]
fn commit_halts_discards_and_surfaces_counterexample() {
    let mut env = ToggleMaze::new();
    let obs = env.reset();
    let mut tl = Timeline::new();
    let model = wrong_toggle_model();
    let plan: Vec<String> = ["right", "right", "right", "right"].map(String::from).to_vec();
    let rep = commit_actions(&mut env, &model, &mut tl, &obs.grid, &plan);
    assert!(rep.halted());
    assert_eq!(rep.mispredictions, 1);
    assert_eq!(rep.executed, vec!["right", "right"]); // halted ON the 7-step
    assert_eq!(rep.discarded, vec!["right", "right"]); // rest discarded
    let ce = rep.counterexample.expect("counterexample");
    assert_eq!(ce.index, tl.len() - 1); // recorded before halt
    assert_eq!(env.actions_taken(), 2); // no further real actions
}

#[test]
fn commit_clean_execution_zero_mispredictions() {
    let mut env = ToggleMaze::new();
    let obs = env.reset();
    let mut tl = Timeline::new();
    let plan: Vec<String> = ["right", "right", "right", "right"].map(String::from).to_vec();
    let rep = commit_actions(&mut env, &true_toggle_model(), &mut tl, &obs.grid, &plan);
    assert!(!rep.halted());
    assert!(rep.win);
    assert!(rep.summary().contains("0 mispredictions"));
    assert!(rep.summary().ends_with("WIN"));
}

// -- (4) run_bfs plans inside the model, spending no environment actions ---

#[test]
fn bfs_never_touches_environment() {
    let mut env = ToggleMaze::new();
    let obs = env.reset();
    let before = env.actions_taken();
    let rep = run_bfs(&true_toggle_model(), &obs.grid, &ACTIONS, 100_000);
    assert_eq!(env.actions_taken(), before); // zero env actions spent
    assert!(rep.found());
    assert_eq!(rep.plan.as_deref().unwrap(), ["right", "right", "right", "right"]);
    assert_eq!(rep.goal_via, Some(GoalVia::Win));
    assert!(rep.expanded >= 1 && rep.distinct >= 1);
    assert!(rep.summary().contains("expanded") && rep.summary().contains("distinct states"));
}

// -- (5) End-to-end mechanism discovery on a hidden-rule game --------------

#[test]
fn toggle_maze_discovers_mechanism_and_wins() {
    let mut env = ToggleMaze::new();
    let first = env.reset();
    let delib = ToggleDeliberator::new(&first.grid);
    let mut agent = SchemaAgent::new(env, WorldModel::vacuous(), delib);
    let log = agent.run();
    assert!(log.won);
    assert!(log.executions.iter().any(|e| e.contains("[experiment]")));
    assert!(log.revisions_seen >= 1);
    let final_bt = run_backtest(&agent.model, &agent.timeline);
    assert!(final_bt.green());
    assert_eq!(final_bt.exact, final_bt.total - final_bt.skipped);
    let last = log.executions.last().unwrap();
    assert!(last.contains("[plan]") && last.contains("0 mispredictions") && last.ends_with("WIN"));
}

// -- (6) Joint representation revision (Evidence 2B analogue) --------------

#[test]
fn cart_dock_counterexample_forces_representation_change() {
    let mut env = CartDock::new();
    let first = env.reset();
    let delib = CartDeliberator::new(&first.grid);
    let mut agent = SchemaAgent::new(env, WorldModel::vacuous(), delib);
    let log = agent.run();
    assert!(log.won);
    // a committed experiment halted on the dock counterexample
    assert!(log.executions.iter().any(|e| e.contains("[experiment]") && e.contains("halted")));
    // the revision history records a REPRESENTATION change
    assert!(agent.model.history.iter().any(|h| h.contains("representation revision")));
    // the learned reflow matches the env's hidden gate
    assert_eq!(
        agent.deliberator.reflow.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>(),
        vec![(CartDock::GATE, 0u8)]
    );
    let final_bt = run_backtest(&agent.model, &agent.timeline);
    assert!(final_bt.green());
    let last = log.executions.last().unwrap();
    assert!(last.contains("0 mispredictions") && last.ends_with("WIN"));
}
