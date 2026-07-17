//! The deliberation protocol — the working seam into jcode's agent runtime.
//!
//! In the original port the `Deliberator` was a Rust trait returning
//! `Box<dyn Fn>` closures: an LLM cannot emit a Rust closure, so the "a
//! jcode provider session sits in the deliberation seat" claim was only a
//! doc note. This module makes it real. A deliberation turn is now:
//!
//!   DeliberationRequest (observation + timeline digest + current program,
//!   all JSON)  ->  a String->String call  ->  DeliberationResponse (an
//!   optional revised RuleProgram + intent + actions + note, all JSON).
//!
//! That `Fn(&str) -> String` is exactly the shape of a provider turn: prompt
//! in, completion out. [`ProtocolDeliberator`] adapts any such function into
//! the harness's [`Deliberator`] trait, so a real jcode provider runtime —
//! or, in tests and the demo, a scripted stand-in — drives the loop by
//! authoring the world program as text.

use serde::{Deserialize, Serialize};

use crate::agent::{Deliberation, Deliberator, Intent};
use crate::backtest::BacktestReport;
use crate::model::WorldModel;
use crate::program::RuleProgram;
use crate::timeline::{Grid, Timeline, Transition};

/// A compact, serializable record of one recorded transition, for the digest
/// handed to the deliberator (grids included so the model can reason).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDigest {
    pub index: usize,
    pub action: String,
    pub state_before: Grid,
    pub state_after: Grid,
    pub level_up: bool,
    pub dead: bool,
    pub win: bool,
}

impl From<&Transition> for TransitionDigest {
    fn from(t: &Transition) -> Self {
        Self {
            index: t.index,
            action: t.action.clone(),
            state_before: t.state_before.clone(),
            state_after: t.state_after.clone(),
            level_up: t.level_up,
            dead: t.dead,
            win: t.win,
        }
    }
}

/// What the harness hands the deliberator each turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliberationRequest {
    pub current_grid: Grid,
    pub legal_actions: Vec<String>,
    /// The full recorded history — ground truth the model must fit.
    pub timeline: Vec<TransitionDigest>,
    /// The current world program (what the model believes), for revision.
    pub current_program: RuleProgram,
    /// The latest backtest summary against the timeline.
    pub backtest_summary: String,
    /// The most recent counterexample, if the last commit halted.
    pub counterexample: Option<TransitionDigest>,
}

/// What the deliberator returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliberationResponse {
    /// An optional revised world program. When present, the harness installs
    /// it (as a joint revision) before planning.
    #[serde(default)]
    pub revised_program: Option<RuleProgram>,
    /// "plan" (certify + BFS to the goal) or "experiment" (run these actions
    /// to gather evidence).
    pub intent: String,
    /// For an experiment: the actions to commit. Ignored for a plan (the
    /// harness plans via in-model BFS).
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub note: String,
}

impl DeliberationRequest {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("request serializes")
    }
}

/// Adapts a provider-shaped `Fn(&str) -> String` into a [`Deliberator`].
pub struct ProtocolDeliberator<F: FnMut(&str) -> String> {
    provider: F,
    /// Records each raw JSON exchange, for audit and the demo transcript.
    pub transcript: Vec<(String, String)>,
}

impl<F: FnMut(&str) -> String> ProtocolDeliberator<F> {
    pub fn new(provider: F) -> Self {
        Self { provider, transcript: Vec::new() }
    }
}

impl<F: FnMut(&str) -> String> Deliberator for ProtocolDeliberator<F> {
    fn deliberate(
        &mut self,
        current: &Grid,
        model: &mut WorldModel,
        timeline: &Timeline,
        backtest: &BacktestReport,
        counterexample: Option<&Transition>,
    ) -> Deliberation {
        let request = DeliberationRequest {
            current_grid: current.clone(),
            legal_actions: ["up", "down", "left", "right"].map(String::from).to_vec(),
            timeline: timeline.iter().map(TransitionDigest::from).collect(),
            current_program: model.program().cloned().unwrap_or_default(),
            backtest_summary: backtest.summary(),
            counterexample: counterexample.map(TransitionDigest::from),
        };
        let prompt = request.to_json();
        let raw = (self.provider)(&prompt);
        self.transcript.push((prompt, raw.clone()));

        let resp: DeliberationResponse = match serde_json::from_str(&raw) {
            Ok(r) => r,
            Err(e) => {
                // A malformed response is a no-op turn with the error noted —
                // the loop simply asks again next cycle.
                return Deliberation {
                    actions: vec![],
                    intent: Intent::Experiment,
                    note: format!("unparseable deliberation response: {e}"),
                };
            }
        };

        if let Some(prog) = resp.revised_program {
            model.revise_program(prog, if resp.note.is_empty() { "protocol revision" } else { &resp.note });
        }

        let intent = if resp.intent == "plan" { Intent::Plan } else { Intent::Experiment };
        Deliberation { actions: resp.actions, intent, note: resp.note }
    }
}
