//! schema — the Schema harness control plane as a jcode workspace crate.
//!
//! Ported from the countersigned Python reference (reverse-engineered from
//! the methodology and reasoning traces published at
//! <https://schema-harness.github.io/>). The architecture splits into a code
//! plane (this crate: append-only Timeline, exact backtesting, in-model BFS
//! planning, gated commit execution) and an inference plane (the
//! [`agent::Deliberator`] seat, where a jcode provider session plugs in —
//! see `docs/SCHEMA_HARNESS.md`).
//!
//! Contract summary, each enforced in exactly one place:
//! - Timeline: ground truth is appended, never rewritten (type-enforced —
//!   no `&mut` access to recorded transitions exists).
//! - run_backtest: a model is certified only by exact replay of the FULL
//!   recorded history (grid on non-terminal steps, flags on every step).
//! - run_bfs: planning spends zero environment actions; it touches only the
//!   model.
//! - commit_actions: the sole channel to the environment; the prediction is
//!   fixed before each real action, the transition is recorded before any
//!   halt, and the first mismatch halts, discards the rest of the plan, and
//!   surfaces the counterexample.

pub mod agent;
pub mod backtest;
pub mod deliberators;
pub mod demo_provider;
pub mod envs;
pub mod executor;
pub mod model;
pub mod planner;
pub mod program;
pub mod protocol;
pub mod session;
pub mod timeline;

pub use agent::{Deliberation, Deliberator, EpisodeLog, SchemaAgent};
pub use backtest::{run_backtest, BacktestReport, Mismatch, MismatchKind};
pub use executor::{commit_actions, Environment, ExecutionReport, Observation};
pub use model::{Prediction, WorldModel};
pub use planner::{run_bfs, BfsReport, GoalVia};
pub use program::{CellWrite, Effect, Rule, RuleProgram};
pub use protocol::{DeliberationRequest, DeliberationResponse, ProtocolDeliberator};
pub use session::Session;
pub use timeline::{Grid, Timeline, Transition};
