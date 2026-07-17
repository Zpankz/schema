//! RuleProgram — the world model as an interpretable, serializable program.
//!
//! The publication's core claim is that the world model is "an interpretable
//! program … readable as text, diffable across versions … searchable" — not
//! an opaque function. The original port faked this with a `Box<dyn Fn>`
//! closure: executable, but not serializable, not diffable, and impossible
//! for a model to *author*. This module makes the claim literally true.
//!
//! A `RuleProgram` is an ordered list of [`Rule`]s over a grid where the
//! player is color 1. Each step: the player attempts to move one cell in the
//! action's direction; the interpreter finds the first rule whose `on_target`
//! matches the color of the cell being entered (and whose optional `beyond`
//! condition holds), and applies that rule's [`Effect`]. If no rule matches,
//! the move is blocked (a no-op). Because it is plain data, the program
//! serializes to JSON, diffs structurally, and can be authored by an LLM as
//! text — which is exactly what the deliberation protocol relies on.

use serde::{Deserialize, Serialize};

use crate::model::Prediction;
use crate::timeline::Grid;

const DELTAS: [(&str, i32, i32); 4] =
    [("up", 0, -1), ("down", 0, 1), ("left", -1, 0), ("right", 1, 0)];

fn delta(action: &str) -> (i32, i32) {
    DELTAS
        .iter()
        .find(|(a, _, _)| *a == action)
        .map(|&(_, dx, dy)| (dx, dy))
        .unwrap_or((0, 0))
}

fn player(grid: &Grid) -> Option<(usize, usize)> {
    for (y, row) in grid.iter().enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if v == 1 {
                return Some((x, y));
            }
        }
    }
    None
}

/// A relative cell rewrite applied by an effect (used for board reflows like
/// CartDock's dock-triggered gate opening). Coordinates are absolute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellWrite {
    pub x: usize,
    pub y: usize,
    pub color: u8,
}

/// What happens when a rule fires.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Effect {
    /// The player moves onto the target cell (leaving floor `0` behind).
    pub player_advances: bool,
    /// The target cell's occupant is pushed one further cell (Sokoban-style);
    /// the player then advances into the vacated target. Used for carts.
    pub push: bool,
    /// Sets the `win` terminal flag.
    pub win: bool,
    /// Sets the `dead` terminal flag.
    pub dead: bool,
    /// Absolute cell rewrites applied after movement — the "board reflow".
    pub writes: Vec<CellWrite>,
    /// If set, `writes` apply only when the pushed occupant landed on a cell
    /// of this color (e.g. the cart docking on color 6). None = always apply.
    pub writes_when_beyond: Option<u8>,
}

impl Effect {
    /// Convenience: the common "walk onto this cell" effect.
    pub fn advance() -> Self {
        Self { player_advances: true, ..Self::default() }
    }
    /// Convenience: "walk onto this cell and win".
    pub fn advance_win() -> Self {
        Self { player_advances: true, win: true, ..Self::default() }
    }
}

/// One transition rule, keyed on the color of the cell the player enters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Fires when the player attempts to enter a cell of this color.
    pub on_target: u8,
    /// Optional guard: the cell one further in the move direction must be one
    /// of these colors (used by `push` effects — the cart's destination).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beyond_any_of: Option<Vec<u8>>,
    /// A short human note describing the rule's intent (survives diffs).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub effect: Effect,
}

impl Rule {
    pub fn new(on_target: u8, effect: Effect) -> Self {
        Self { on_target, beyond_any_of: None, note: String::new(), effect }
    }
}

/// The world model as data: an ordered list of rules plus a `goal` predicate
/// expressed as "no cell has any of these colors" (used by in-model search
/// for auxiliary goals; the primary goal is the `win` flag).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuleProgram {
    pub rules: Vec<Rule>,
    /// Goal predicate: satisfied when the grid contains none of these colors.
    /// Empty = never satisfied by predicate (rely on the `win` flag instead).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goal_absent_colors: Vec<u8>,
}

impl RuleProgram {
    /// The honest zero hypothesis: everything is blocked, nothing wins.
    pub fn vacuous() -> Self {
        Self::default()
    }

    /// Execute one step: the world model's `step(state, action)`.
    pub fn step(&self, state: &Grid, action: &str) -> Prediction {
        let (px, py) = match player(state) {
            Some(p) => p,
            None => return Prediction::of(state.clone()),
        };
        let (dx, dy) = delta(action);
        let tx = px as i32 + dx;
        let ty = py as i32 + dy;
        if ty < 0 || tx < 0 || ty as usize >= state.len() || tx as usize >= state[0].len() {
            return Prediction::of(state.clone());
        }
        let (tx, ty) = (tx as usize, ty as usize);
        let target = state[ty][tx];

        for rule in &self.rules {
            if rule.on_target != target {
                continue;
            }
            // beyond guard
            let beyond_pos = (tx as i32 + dx, ty as i32 + dy);
            let beyond = if beyond_pos.0 >= 0
                && beyond_pos.1 >= 0
                && (beyond_pos.1 as usize) < state.len()
                && (beyond_pos.0 as usize) < state[0].len()
            {
                Some(state[beyond_pos.1 as usize][beyond_pos.0 as usize])
            } else {
                None
            };
            if let Some(allowed) = &rule.beyond_any_of {
                match beyond {
                    Some(b) if allowed.contains(&b) => {}
                    _ => continue, // guard failed; try next rule
                }
            }
            return self.apply(state, px, py, tx, ty, dx, dy, beyond, &rule.effect);
        }
        // No rule matched: blocked.
        Prediction::of(state.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        &self,
        state: &Grid,
        px: usize,
        py: usize,
        tx: usize,
        ty: usize,
        dx: i32,
        dy: i32,
        beyond: Option<u8>,
        effect: &Effect,
    ) -> Prediction {
        let mut g = state.clone();
        let mut docked_color = None;

        if effect.push {
            let bx = tx as i32 + dx;
            let by = ty as i32 + dy;
            if by < 0 || bx < 0 || (by as usize) >= g.len() || (bx as usize) >= g[0].len() {
                return Prediction::of(state.clone()); // nowhere to push: blocked
            }
            let (bx, by) = (bx as usize, by as usize);
            docked_color = beyond;
            g[by][bx] = g[ty][tx]; // occupant moves one further
            g[ty][tx] = 1; // player takes the vacated cell
            g[py][px] = 0;
        } else if effect.player_advances {
            g[py][px] = 0;
            g[ty][tx] = 1;
        }

        let writes_apply = match effect.writes_when_beyond {
            None => true,
            Some(c) => docked_color == Some(c),
        };
        if writes_apply {
            for w in &effect.writes {
                if w.y < g.len() && w.x < g[0].len() {
                    g[w.y][w.x] = w.color;
                }
            }
        }

        Prediction { grid: g, level_up: false, dead: effect.dead, win: effect.win }
    }

    pub fn is_goal(&self, state: &Grid) -> bool {
        if self.goal_absent_colors.is_empty() {
            return false;
        }
        !state
            .iter()
            .flatten()
            .any(|v| self.goal_absent_colors.contains(v))
    }

    // -- serialization ------------------------------------------------------
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("RuleProgram serializes")
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// A compact human-readable form, one line per rule — the "readable as
    /// text" property the publication describes.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        for r in &self.rules {
            let mut parts = vec![format!("on color {}", r.on_target)];
            if let Some(b) = &r.beyond_any_of {
                parts.push(format!("beyond∈{b:?}"));
            }
            let mut effs = vec![];
            if r.effect.push {
                effs.push("push".to_string());
            }
            if r.effect.player_advances {
                effs.push("advance".to_string());
            }
            if r.effect.win {
                effs.push("win".to_string());
            }
            if r.effect.dead {
                effs.push("dead".to_string());
            }
            if !r.effect.writes.is_empty() {
                let cond = r
                    .effect
                    .writes_when_beyond
                    .map(|c| format!(" when beyond={c}"))
                    .unwrap_or_default();
                effs.push(format!(
                    "writes {}{}",
                    r.effect
                        .writes
                        .iter()
                        .map(|w| format!("({},{})={}", w.x, w.y, w.color))
                        .collect::<Vec<_>>()
                        .join(","),
                    cond
                ));
            }
            if effs.is_empty() {
                effs.push("block".to_string());
            }
            out.push_str(&format!("{} -> {}\n", parts.join(" "), effs.join("+")));
        }
        if out.is_empty() {
            out.push_str("(no rules: everything blocked)\n");
        }
        out
    }

    /// A structural diff against a previous program, as human-readable lines.
    /// Realizes "diffable across versions".
    pub fn diff(&self, prev: &RuleProgram) -> Vec<String> {
        let mut lines = vec![];
        let key = |r: &Rule| (r.on_target, r.beyond_any_of.clone());
        for r in &self.rules {
            match prev.rules.iter().find(|p| key(p) == key(r)) {
                None => lines.push(format!("+ rule on color {}", r.on_target)),
                Some(p) if p.effect != r.effect => {
                    lines.push(format!("~ rule on color {} effect changed", r.on_target))
                }
                _ => {}
            }
        }
        for p in &prev.rules {
            if !self.rules.iter().any(|r| key(r) == key(p)) {
                lines.push(format!("- rule on color {}", p.on_target));
            }
        }
        if self.goal_absent_colors != prev.goal_absent_colors {
            lines.push(format!(
                "~ goal predicate: absent{:?} -> absent{:?}",
                prev.goal_absent_colors, self.goal_absent_colors
            ));
        }
        lines
    }
}
