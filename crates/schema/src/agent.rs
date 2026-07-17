//! The outer loop: Observe -> Deliberate -> Execute -> Record.
//!
//! Code plane (this file): loop control, stage ordering, Timeline recording,
//! the backtest gate before planning, and the rule that a misprediction
//! returns control to deliberation carrying the counterexample.
//!
//! Inference plane: [`Deliberator`] — in the demos a bounded-hypothesis
//! implementation; in jcode, the seat where a provider session plugs in
//! (see docs/SCHEMA_HARNESS.md).

use crate::backtest::{run_backtest, BacktestReport};
use crate::executor::{commit_actions, Environment};
use crate::model::WorldModel;
use crate::planner::run_bfs;
use crate::timeline::{Grid, Timeline, Transition};

/// What a deliberator returns: an (optionally revised) model plus either a
/// goal-directed plan request or a discriminating experiment.
pub struct Deliberation {
    pub actions: Vec<String>,
    pub intent: Intent,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Plan,
    Experiment,
}

/// The inference plane. Sees ONLY: current observation grid, the Timeline,
/// the current model, backtest reports, and counterexamples — never the
/// environment's hidden rules.
pub trait Deliberator {
    fn deliberate(
        &mut self,
        current: &Grid,
        model: &mut WorldModel,
        timeline: &Timeline,
        backtest: &BacktestReport,
        counterexample: Option<&Transition>,
    ) -> Deliberation;
}

#[derive(Debug, Default)]
pub struct EpisodeLog {
    pub backtests: Vec<String>,
    pub bfs_reports: Vec<String>,
    pub executions: Vec<String>,
    pub notes: Vec<String>,
    pub revisions_seen: u32,
    pub won: bool,
    pub env_actions: usize,
}

/// Runs the observe-deliberate-execute-record cycle until win or budget.
pub struct SchemaAgent<E: Environment, D: Deliberator> {
    pub env: E,
    pub model: WorldModel,
    pub deliberator: D,
    pub timeline: Timeline,
    pub max_cycles: usize,
}

impl<E: Environment, D: Deliberator> SchemaAgent<E, D> {
    pub fn new(env: E, model: WorldModel, deliberator: D) -> Self {
        Self { env, model, deliberator, timeline: Timeline::new(), max_cycles: 60 }
    }

    pub fn run(&mut self) -> EpisodeLog {
        let mut log = EpisodeLog::default();
        let obs = self.env.reset(); // Observe
        let mut current = obs.grid;
        let mut counterexample: Option<Transition> = None;

        for _ in 0..self.max_cycles {
            // ---- Deliberate ----------------------------------------------
            let report = run_backtest(&self.model, &self.timeline);
            log.backtests.push(report.summary());
            let mut delib = self.deliberator.deliberate(
                &current,
                &mut self.model,
                &self.timeline,
                &report,
                counterexample.as_ref(),
            );
            counterexample = None;
            if !delib.note.is_empty() {
                log.notes.push(delib.note.clone());
            }

            // Backtest gate: goal-directed plans are trusted only when the
            // model is certified against the complete history.
            if delib.intent == Intent::Plan {
                let post = run_backtest(&self.model, &self.timeline);
                if !(post.green() || self.timeline.is_empty()) {
                    delib.intent = Intent::Experiment;
                } else {
                    let actions = self.env.legal_actions().to_vec();
                    let bfs = run_bfs(&self.model, &current, &actions, 100_000);
                    log.bfs_reports.push(bfs.summary());
                    if let Some(plan) = bfs.plan {
                        delib.actions = plan;
                    }
                }
            }

            if delib.actions.is_empty() {
                continue;
            }

            // ---- Execute + Record ----------------------------------------
            let result = commit_actions(
                &mut self.env,
                &self.model,
                &mut self.timeline,
                &current,
                &delib.actions,
            );
            let tag = match delib.intent {
                Intent::Plan => "[plan]",
                Intent::Experiment => "[experiment]",
            };
            log.executions.push(format!("{} {}", tag, result.summary()));
            current = result.final_state.clone();

            if result.win {
                log.won = true;
                break;
            }
            if result.dead {
                let obs = self.env.reset();
                current = obs.grid;
                continue;
            }
            if result.halted() {
                counterexample = result.counterexample; // back to deliberation
            }
        }

        log.revisions_seen = self.model.revision;
        log.env_actions = self.env.actions_taken();
        log
    }
}
