//! The editable program world model: step(state, action) + is_goal(state).
//!
//! Publication observables ("Core idea" / "The Claude row"): the world model
//! is an interpretable, editable program — "Encode the current world model
//! as a runnable step() program" — replayable and searchable. Revisions are
//! joint over representation and rules, and each is recorded (Evidence 2C's
//! structured-revision narratives).

use crate::timeline::Grid;

/// What the model claims the next observation will be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    pub grid: Grid,
    pub level_up: bool,
    pub dead: bool,
    pub win: bool,
}

impl Prediction {
    pub fn of(grid: Grid) -> Self {
        Self { grid, level_up: false, dead: false, win: false }
    }

    pub fn win(grid: Grid) -> Self {
        Self { grid, level_up: false, dead: false, win: true }
    }

    pub fn dead(grid: Grid) -> Self {
        Self { grid, level_up: false, dead: true, win: false }
    }

    pub fn terminal(&self) -> bool {
        self.level_up || self.dead || self.win
    }
}

pub type StepFn = Box<dyn Fn(&Grid, &str) -> Prediction>;
pub type GoalFn = Box<dyn Fn(&Grid) -> bool>;

/// Holds the current executable hypothesis about the game's mechanism.
/// `revise` swaps step()/is_goal() jointly — the publication is explicit
/// that state representation and transition rules revise together.
pub struct WorldModel {
    step: StepFn,
    is_goal: GoalFn,
    pub revision: u32,
    pub history: Vec<String>,
}

impl WorldModel {
    pub fn new(step: StepFn, is_goal: GoalFn, description: &str) -> Self {
        Self { step, is_goal, revision: 0, history: vec![description.to_string()] }
    }

    /// A model that predicts "nothing changes" — the honest zero hypothesis.
    pub fn vacuous() -> Self {
        Self::new(
            Box::new(|g: &Grid, _a: &str| Prediction::of(g.clone())),
            Box::new(|_g: &Grid| false),
            "vacuous initial model",
        )
    }

    pub fn step(&self, state: &Grid, action: &str) -> Prediction {
        (self.step)(state, action)
    }

    pub fn is_goal(&self, state: &Grid) -> bool {
        (self.is_goal)(state)
    }

    pub fn revise(&mut self, step: Option<StepFn>, is_goal: Option<GoalFn>, description: &str) {
        if let Some(s) = step {
            self.step = s;
        }
        if let Some(g) = is_goal {
            self.is_goal = g;
        }
        self.revision += 1;
        self.history.push(description.to_string());
    }
}
