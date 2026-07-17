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
    /// The current program, when this model is program-backed. `None` for
    /// closure-backed models (the original demos). When present, this is the
    /// serializable, diffable, LLM-authored source of the model's behavior.
    program: Option<crate::program::RuleProgram>,
}

impl WorldModel {
    pub fn new(step: StepFn, is_goal: GoalFn, description: &str) -> Self {
        Self { step, is_goal, revision: 0, history: vec![description.to_string()], program: None }
    }

    /// Build a model whose behavior IS an interpretable [`RuleProgram`]. The
    /// program is retained so it can be serialized, diffed, and re-authored.
    pub fn from_program(program: crate::program::RuleProgram, description: &str) -> Self {
        let p_step = program.clone();
        let p_goal = program.clone();
        Self {
            step: Box::new(move |g: &Grid, a: &str| p_step.step(g, a)),
            is_goal: Box::new(move |g: &Grid| p_goal.is_goal(g)),
            revision: 0,
            history: vec![description.to_string()],
            program: Some(program),
        }
    }

    /// The current program, if this model is program-backed.
    pub fn program(&self) -> Option<&crate::program::RuleProgram> {
        self.program.as_ref()
    }

    /// Install a new program as a joint revision, recording the text diff
    /// against the prior program in the revision history.
    pub fn revise_program(&mut self, program: crate::program::RuleProgram, description: &str) {
        let diff = match &self.program {
            Some(prev) => {
                let d = program.diff(prev);
                if d.is_empty() { "no structural change".to_string() } else { d.join("; ") }
            }
            None => "installed initial program".to_string(),
        };
        let p_step = program.clone();
        let p_goal = program.clone();
        self.step = Box::new(move |g: &Grid, a: &str| p_step.step(g, a));
        self.is_goal = Box::new(move |g: &Grid| p_goal.is_goal(g));
        self.program = Some(program);
        self.revision += 1;
        self.history.push(format!("{description} [{diff}]"));
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
