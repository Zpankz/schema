//! commit_actions — the sole channel from thinking to real interaction.
//!
//! Observables ("The outer loop" stage 3, "The Claude row" constraint 3):
//! per-step validation against a prediction fixed BEFORE the action; a
//! single mismatch halts immediately and discards the remaining plan; every
//! real transition — including the mismatched one — lands on the Timeline
//! before the halt, becoming the counterexample.

use crate::model::WorldModel;
use crate::timeline::{Grid, Timeline, Transition};

/// What an environment returns per interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub grid: Grid,
    pub level_up: bool,
    pub dead: bool,
    pub win: bool,
}

impl Observation {
    pub fn of(grid: Grid) -> Self {
        Self { grid, level_up: false, dead: false, win: false }
    }

    pub fn win(grid: Grid) -> Self {
        Self { grid, level_up: false, dead: false, win: true }
    }
}

/// Minimal ARC-AGI-3-style environment interface (abducted). Rules are
/// hidden behind it; only observations cross.
pub trait Environment {
    fn reset(&mut self) -> Observation;
    fn step(&mut self, action: &str) -> Observation;
    fn actions_taken(&self) -> usize;
    fn legal_actions(&self) -> &[&'static str];
}

#[derive(Debug)]
pub struct ExecutionReport {
    pub committed: Vec<String>,
    pub executed: Vec<String>,
    pub discarded: Vec<String>,
    pub mispredictions: usize,
    pub counterexample: Option<Transition>,
    pub counterexample_detail: String,
    pub final_state: Grid,
    pub win: bool,
    pub level_up: bool,
    pub dead: bool,
}

impl ExecutionReport {
    pub fn halted(&self) -> bool {
        self.mispredictions > 0
    }

    pub fn summary(&self) -> String {
        let n = self.executed.len();
        if !self.halted() {
            let tail = if self.win {
                "WIN"
            } else if self.level_up {
                "level_up"
            } else {
                "ok"
            };
            format!("{}/{} executed with 0 mispredictions -> {}", n, self.committed.len(), tail)
        } else {
            format!(
                "halted after {}/{} actions: {}; {} queued action(s) discarded",
                n,
                self.committed.len(),
                self.counterexample_detail,
                self.discarded.len()
            )
        }
    }
}

/// Execute `plan` with per-step gating. Prediction precedes the action; the
/// transition is recorded before any halt decision; the first mismatch
/// halts, discards the remainder, and surfaces the counterexample.
pub fn commit_actions(
    env: &mut dyn Environment,
    model: &WorldModel,
    timeline: &mut Timeline,
    current: &Grid,
    plan: &[String],
) -> ExecutionReport {
    let mut cur = current.clone();
    let mut executed: Vec<String> = Vec::new();
    let mut last: Option<Observation>;

    for (i, action) in plan.iter().enumerate() {
        let pred = model.step(&cur, action); // prediction precedes action
        let obs = env.step(action); //          the only real interaction
        let t = timeline
            .append(cur.clone(), action, obs.grid.clone(), obs.level_up, obs.dead, obs.win)
            .clone();
        executed.push(action.clone());
        cur = obs.grid.clone();
        let terminal = t.terminal();
        last = Some(obs);

        let flags_ok = pred.level_up == t.level_up && pred.dead == t.dead && pred.win == t.win;
        let ok = flags_ok && (terminal || pred.grid == cur);
        if !ok {
            let detail = format!(
                "{} at committed step {}",
                if flags_ok { "grid misprediction" } else { "flag misprediction" },
                i
            );
            let l = last.expect("just set");
            return ExecutionReport {
                committed: plan.to_vec(),
                executed,
                discarded: plan[i + 1..].to_vec(),
                mispredictions: 1,
                counterexample: Some(t),
                counterexample_detail: detail,
                final_state: cur,
                win: l.win,
                level_up: l.level_up,
                dead: l.dead,
            };
        }
        if terminal {
            let l = last.expect("just set");
            return ExecutionReport {
                committed: plan.to_vec(),
                executed,
                discarded: plan[i + 1..].to_vec(),
                mispredictions: 0,
                counterexample: None,
                counterexample_detail: String::new(),
                final_state: cur,
                win: l.win,
                level_up: l.level_up,
                dead: l.dead,
            };
        }
    }

    ExecutionReport {
        committed: plan.to_vec(),
        executed,
        discarded: vec![],
        mispredictions: 0,
        counterexample: None,
        counterexample_detail: String::new(),
        final_state: cur,
        win: false,
        level_up: false,
        dead: false,
    }
}
