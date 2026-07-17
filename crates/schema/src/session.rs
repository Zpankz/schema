//! Session persistence — save and resume a running harness session.
//!
//! jcode agents run across many turns and process boundaries; a harness that
//! augments them must be able to persist its state (the append-only Timeline
//! and the current world program) and resume exactly. The original port had
//! no serialization at all, so a session could not survive a turn boundary.
//! This module closes that gap: a [`Session`] snapshot round-trips through
//! JSON, and reloading reconstructs a program-backed [`WorldModel`] plus a
//! Timeline whose recorded transitions replay identically.

use serde::{Deserialize, Serialize};

use crate::model::WorldModel;
use crate::program::RuleProgram;
use crate::protocol::TransitionDigest;
use crate::timeline::Timeline;

/// A serializable snapshot of a harness session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub task: String,
    pub program: RuleProgram,
    pub revision: u32,
    pub history: Vec<String>,
    pub timeline: Vec<TransitionDigest>,
}

impl Session {
    /// Capture the current program-backed model and timeline.
    pub fn capture(task: &str, model: &WorldModel, timeline: &Timeline) -> Self {
        Self {
            task: task.to_string(),
            program: model.program().cloned().unwrap_or_default(),
            revision: model.revision,
            history: model.history.clone(),
            timeline: timeline.iter().map(TransitionDigest::from).collect(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("session serializes")
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Rebuild a program-backed model from the snapshot. The revision counter
    /// and history are restored so the resumed model is indistinguishable
    /// from the one that was captured.
    pub fn restore_model(&self) -> WorldModel {
        let mut m = WorldModel::from_program(self.program.clone(), "restored from session");
        m.revision = self.revision;
        m.history = self.history.clone();
        m
    }

    /// Rebuild the append-only Timeline from the snapshot.
    pub fn restore_timeline(&self) -> Timeline {
        let mut tl = Timeline::new();
        for t in &self.timeline {
            tl.append(
                t.state_before.clone(),
                &t.action,
                t.state_after.clone(),
                t.level_up,
                t.dead,
                t.win,
            );
        }
        tl
    }
}
