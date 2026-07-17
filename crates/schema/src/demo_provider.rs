//! A scripted, provider-shaped deliberator stand-in.
//!
//! This is NOT the harness. It is a bounded stand-in for a jcode provider
//! turn: a pure `&str -> String` function that receives a
//! [`crate::protocol::DeliberationRequest`] as JSON and returns a
//! [`crate::protocol::DeliberationResponse`] as JSON — the same contract a
//! real LLM completion would satisfy. It is deliberately given *no* access
//! to any environment's hidden rules; it reasons only from the observation
//! and the recorded Timeline, exactly as the model in the deliberation seat
//! must. Its presence proves the protocol seam is real: swap this function
//! for a provider call and the harness is unchanged.
//!
//! The reasoning it performs — classify each color's semantics from recorded
//! transitions, author the world program accordingly, and probe the nearest
//! still-ambiguous color — is the same "action for discovery" the paper
//! describes; here it is emitted as JSON program edits rather than executed
//! as Rust.

use std::collections::VecDeque;

use crate::program::{Effect, Rule, RuleProgram};
use crate::protocol::{DeliberationRequest, DeliberationResponse, TransitionDigest};
use crate::timeline::Grid;

const DIRS: [(&str, i32, i32); 4] =
    [("up", 0, -1), ("down", 0, 1), ("left", -1, 0), ("right", 1, 0)];

fn player(grid: &Grid) -> Option<(usize, usize)> {
    grid.iter().enumerate().find_map(|(y, row)| {
        row.iter().position(|&v| v == 1).map(|x| (x, y))
    })
}

fn player_in(t: &TransitionDigest, after: bool) -> Option<(usize, usize)> {
    player(if after { &t.state_after } else { &t.state_before })
}

/// Classify a color from the recorded history: what happened when the player
/// stepped toward a cell of that color.
#[derive(Clone, Copy, PartialEq)]
enum Sem {
    Passable,
    Winning,
    Lethal,
    Blocking,
}

fn classify(timeline: &[TransitionDigest]) -> std::collections::BTreeMap<u8, Sem> {
    let mut out = std::collections::BTreeMap::new();
    for t in timeline {
        let (Some(pb), (dx, dy)) = (player_in(t, false), dir_delta(&t.action)) else {
            continue;
        };
        let tx = pb.0 as i32 + dx;
        let ty = pb.1 as i32 + dy;
        if ty < 0 || tx < 0 || ty as usize >= t.state_before.len() || tx as usize >= t.state_before[0].len() {
            continue;
        }
        let target = t.state_before[ty as usize][tx as usize];
        if target == 0 || target == 1 {
            continue; // floor/self carry no mechanism to learn
        }
        let moved = player_in(t, true) == Some((tx as usize, ty as usize));
        let sem = if t.win {
            Sem::Winning
        } else if t.dead {
            Sem::Lethal
        } else if moved {
            Sem::Passable
        } else {
            Sem::Blocking
        };
        out.insert(target, sem);
    }
    out
}

fn dir_delta(action: &str) -> (i32, i32) {
    DIRS.iter().find(|(a, _, _)| *a == action).map(|&(_, x, y)| (x, y)).unwrap_or((0, 0))
}

/// Build the maximal world program consistent with what has been observed.
fn author_program(sems: &std::collections::BTreeMap<u8, Sem>) -> RuleProgram {
    // Priors the agent is allowed (stated in the demo): floor advances, the
    // goal color 3 wins on entry.
    let mut rules = vec![
        Rule { on_target: 0, beyond_any_of: None, note: "floor".into(), effect: Effect::advance() },
        Rule { on_target: 3, beyond_any_of: None, note: "goal".into(), effect: Effect::advance_win() },
    ];
    for (&color, &sem) in sems {
        if color == 0 || color == 3 {
            continue;
        }
        let effect = match sem {
            Sem::Passable => Effect::advance(),
            Sem::Winning => Effect::advance_win(),
            Sem::Lethal => Effect { dead: true, ..Effect::default() },
            Sem::Blocking => continue, // no rule = blocked
        };
        rules.push(Rule {
            on_target: color,
            beyond_any_of: None,
            note: format!("learned {}", sem_name(sem)),
            effect,
        });
    }
    RuleProgram { rules, goal_absent_colors: vec![] }
}

fn sem_name(s: Sem) -> &'static str {
    match s {
        Sem::Passable => "passable",
        Sem::Winning => "winning",
        Sem::Lethal => "lethal",
        Sem::Blocking => "blocking",
    }
}

/// Colors present in the grid whose semantics are not yet observed.
fn unknown_colors(grid: &Grid, sems: &std::collections::BTreeMap<u8, Sem>) -> Vec<u8> {
    let mut seen = std::collections::BTreeSet::new();
    for &v in grid.iter().flatten() {
        if v != 0 && v != 1 && v != 3 && !sems.contains_key(&v) {
            seen.insert(v);
        }
    }
    seen.into_iter().collect()
}

/// Shortest action sequence over floor (0) cells ending with a step INTO a
/// cell of one of `targets` — a discriminating probe.
fn probe_path(grid: &Grid, targets: &[u8]) -> Option<Vec<String>> {
    let start = player(grid)?;
    let mut frontier = VecDeque::from([(start, Vec::<String>::new())]);
    let mut seen = std::collections::BTreeSet::from([start]);
    while let Some(((x, y), path)) = frontier.pop_front() {
        for (a, dx, dy) in DIRS {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if ny < 0 || nx < 0 || ny as usize >= grid.len() || nx as usize >= grid[0].len() {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let v = grid[ny][nx];
            let mut next = path.clone();
            next.push(a.to_string());
            if targets.contains(&v) {
                return Some(next);
            }
            if v == 0 && seen.insert((nx, ny)) {
                frontier.push_back(((nx, ny), next));
            }
        }
    }
    None
}

/// The provider-shaped responder: JSON in, JSON out.
pub fn toggle_provider(prompt: &str) -> String {
    let req: DeliberationRequest = match serde_json::from_str(prompt) {
        Ok(r) => r,
        Err(e) => {
            return serde_json::to_string(&DeliberationResponse {
                revised_program: None,
                intent: "experiment".into(),
                actions: vec![],
                note: format!("could not parse request: {e}"),
            })
            .unwrap();
        }
    };

    let sems = classify(&req.timeline);
    let program = author_program(&sems);
    let unknown = unknown_colors(&req.current_grid, &sems);

    let (intent, actions, note) = if !unknown.is_empty() {
        match probe_path(&req.current_grid, &unknown) {
            Some(p) => (
                "experiment",
                p.clone(),
                format!("probing ambiguous color(s) {unknown:?} via {p:?} to learn their semantics"),
            ),
            None => ("plan", vec![], "no reachable probe; planning with current model".into()),
        }
    } else {
        ("plan", vec![], "mechanism fully classified; certify and plan to goal".into())
    };

    serde_json::to_string(&DeliberationResponse {
        revised_program: Some(program),
        intent: intent.into(),
        actions,
        note,
    })
    .unwrap()
}
