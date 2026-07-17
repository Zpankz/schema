//! Demo deliberators: bounded-hypothesis-space stands-in for the inference
//! plane. They see only observations, the Timeline, backtest reports, and
//! counterexamples — never the environments' hidden rules.
//!
//! `ToggleDeliberator` reproduces "Action for discovery": keep every
//! candidate mechanism consistent with recorded history; while several
//! survive, commit a discriminating probe.
//!
//! `CartDeliberator` reproduces Evidence 2B's joint revision: a cart-sprite
//! representation whose dock counterexample forces representational
//! unification, with the board reflow learned from the observed diff.

use std::collections::{BTreeMap, HashSet, VecDeque};

use crate::agent::{Deliberation, Deliberator, Intent};
use crate::backtest::BacktestReport;
use crate::envs::{delta, find_color, ACTIONS};
use crate::model::{Prediction, WorldModel};
use crate::planner::run_bfs;
use crate::timeline::{Grid, Timeline, Transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semantics {
    Blocking,
    Passable,
    Lethal,
}

const SEMANTICS: [Semantics; 3] = [Semantics::Blocking, Semantics::Passable, Semantics::Lethal];

type SemMap = BTreeMap<u8, Semantics>;

fn moved(grid: &Grid, px: usize, py: usize, tx: usize, ty: usize) -> Grid {
    let mut g = grid.clone();
    g[py][px] = 0;
    g[ty][tx] = 1;
    g
}

fn predict(sem: &SemMap, state: &Grid, action: &str) -> Prediction {
    let (px, py) = find_color(state, 1)[0];
    let (dx, dy) = delta(action);
    let (tx, ty) = ((px as i32 + dx) as usize, (py as i32 + dy) as usize);
    let v = state[ty][tx];
    let s = match v {
        0 => Semantics::Passable,
        3 => return Prediction::win(moved(state, px, py, tx, ty)),
        c => *sem.get(&c).unwrap_or(&Semantics::Blocking),
    };
    match s {
        Semantics::Passable => Prediction::of(moved(state, px, py, tx, ty)),
        Semantics::Lethal => Prediction::dead(state.clone()),
        Semantics::Blocking => Prediction::of(state.clone()),
    }
}

/// Priors: color 1 is the agent, 0 floor, 3 the goal, actions translate one
/// cell. Unknown mechanism: the semantics of every other color present.
pub struct ToggleDeliberator {
    unknown_colors: Vec<u8>,
    candidates: Vec<SemMap>,
}

impl ToggleDeliberator {
    pub fn new(initial_grid: &Grid) -> Self {
        let mut colors: Vec<u8> = initial_grid
            .iter()
            .flatten()
            .copied()
            .filter(|c| ![0u8, 1, 3].contains(c))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        colors.sort_unstable();
        let mut candidates = vec![SemMap::new()];
        for &c in &colors {
            candidates = candidates
                .into_iter()
                .flat_map(|m| {
                    SEMANTICS.iter().map(move |&s| {
                        let mut m2 = m.clone();
                        m2.insert(c, s);
                        m2
                    })
                })
                .collect();
        }
        Self { unknown_colors: colors, candidates }
    }

    fn consistent(sem: &SemMap, timeline: &Timeline) -> bool {
        for t in timeline {
            let p = predict(sem, &t.state_before, &t.action);
            if (p.level_up, p.dead, p.win) != (t.level_up, t.dead, t.win) {
                return false;
            }
            if !t.terminal() && p.grid != t.state_after {
                return false;
            }
        }
        true
    }

    fn ambiguous_colors(&self) -> Vec<u8> {
        self.unknown_colors
            .iter()
            .copied()
            .filter(|c| {
                self.candidates.iter().map(|m| m[c]).collect::<HashSet<_>>().len() > 1
            })
            .collect()
    }

    /// Shortest action sequence through consensus-floor cells ending with a
    /// step INTO a cell of an ambiguous color — the discriminating probe.
    fn probe_path(state: &Grid, targets: &[u8]) -> Option<Vec<String>> {
        let (px, py) = find_color(state, 1)[0];
        let mut frontier = VecDeque::from([((px, py), Vec::<String>::new())]);
        let mut seen = HashSet::from([(px, py)]);
        while let Some(((x, y), path)) = frontier.pop_front() {
            for a in ACTIONS {
                let (dx, dy) = delta(a);
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if ny < 0 || nx < 0 || ny as usize >= state.len() || nx as usize >= state[0].len()
                {
                    continue;
                }
                let (nxu, nyu) = (nx as usize, ny as usize);
                let v = state[nyu][nxu];
                let mut next = path.clone();
                next.push(a.to_string());
                if targets.contains(&v) {
                    return Some(next);
                }
                if v == 0 && !seen.contains(&(nxu, nyu)) {
                    seen.insert((nxu, nyu));
                    frontier.push_back(((nxu, nyu), next));
                }
            }
        }
        None
    }
}

impl Deliberator for ToggleDeliberator {
    fn deliberate(
        &mut self,
        current: &Grid,
        model: &mut WorldModel,
        timeline: &Timeline,
        backtest: &BacktestReport,
        _counterexample: Option<&Transition>,
    ) -> Deliberation {
        self.candidates.retain(|c| Self::consistent(c, timeline));
        let best = self.candidates[0].clone();
        let desc = format!(
            "mechanism hypothesis {:?} ({} candidate(s) consistent with {}-step history)",
            best,
            self.candidates.len(),
            timeline.len()
        );
        let sem = best.clone();
        model.revise(Some(Box::new(move |s: &Grid, a: &str| predict(&sem, s, a))), None, &desc);

        let ambiguous = self.ambiguous_colors();
        if !ambiguous.is_empty() {
            if let Some(probe) = Self::probe_path(current, &ambiguous) {
                return Deliberation {
                    actions: probe.clone(),
                    intent: Intent::Experiment,
                    note: format!(
                        "Ambiguity over colors {:?}: committing discriminating probe {:?}; \
                         outcomes differ across {} surviving candidates.",
                        ambiguous,
                        probe,
                        self.candidates.len()
                    ),
                };
            }
        }
        Deliberation {
            actions: vec![],
            intent: Intent::Plan,
            note: format!(
                "Unique-enough mechanism {best:?}; backtest {}; handing plan to in-model BFS.",
                backtest.summary()
            ),
        }
    }
}

/// Priors: movement and push mechanics. Initial representation: static
/// board with a cart sprite. The dock counterexample forces unification —
/// the cart is a board cell and docking rewrites cells learned from the
/// observed diff — after which backtest certifies the revised program.
pub struct CartDeliberator {
    dock_cells: Vec<(usize, usize)>,
    pub reflow: BTreeMap<(usize, usize), u8>,
    pub unified: bool,
}

impl CartDeliberator {
    pub fn new(initial_grid: &Grid) -> Self {
        Self { dock_cells: find_color(initial_grid, 6), reflow: BTreeMap::new(), unified: false }
    }

    fn cart_step(
        dock_cells: &[(usize, usize)],
        reflow: &BTreeMap<(usize, usize), u8>,
        unified: bool,
        state: &Grid,
        action: &str,
    ) -> Prediction {
        let (px, py) = find_color(state, 1)[0];
        let (dx, dy) = delta(action);
        let (tx, ty) = ((px as i32 + dx) as usize, (py as i32 + dy) as usize);
        match state[ty][tx] {
            0 => Prediction::of(moved(state, px, py, tx, ty)),
            3 => Prediction::win(moved(state, px, py, tx, ty)),
            4 => {
                let (bx, by) = ((tx as i32 + dx) as usize, (ty as i32 + dy) as usize);
                let beyond = state[by][bx];
                if beyond == 0 || beyond == 6 {
                    let mut g = state.clone();
                    g[by][bx] = 4;
                    g[ty][tx] = 1;
                    g[py][px] = 0;
                    if unified && dock_cells.contains(&(bx, by)) {
                        for (&(rx, ry), &val) in reflow {
                            g[ry][rx] = val; // board reflow (learned)
                        }
                    }
                    Prediction::of(g)
                } else {
                    Prediction::of(state.clone())
                }
            }
            _ => Prediction::of(state.clone()),
        }
    }

    fn install(&self, model: &mut WorldModel, description: &str) {
        let dock = self.dock_cells.clone();
        let reflow = self.reflow.clone();
        let unified = self.unified;
        model.revise(
            Some(Box::new(move |s: &Grid, a: &str| {
                Self::cart_step(&dock, &reflow, unified, s, a)
            })),
            None,
            description,
        );
    }

    fn cart_docked(&self, state: &Grid) -> bool {
        self.dock_cells.iter().any(|&(x, y)| state[y][x] == 4)
    }
}

impl Deliberator for CartDeliberator {
    fn deliberate(
        &mut self,
        current: &Grid,
        model: &mut WorldModel,
        _timeline: &Timeline,
        backtest: &BacktestReport,
        counterexample: Option<&Transition>,
    ) -> Deliberation {
        if model.revision == 0 {
            self.install(model, "initial representation: static board + cart sprite; walls immutable");
        }

        if let Some(ce) = counterexample {
            if !self.unified {
                // Learn the reflow from the observed diff the sprite model missed.
                let pred = Self::cart_step(
                    &self.dock_cells,
                    &self.reflow,
                    false,
                    &ce.state_before,
                    &ce.action,
                );
                for (y, (prow, orow)) in pred.grid.iter().zip(&ce.state_after).enumerate() {
                    for (x, (&pv, &ov)) in prow.iter().zip(orow).enumerate() {
                        if pv != ov {
                            self.reflow.insert((x, y), ov);
                        }
                    }
                }
                self.unified = true;
                let cells: Vec<_> = self.reflow.keys().copied().collect();
                self.install(
                    model,
                    &format!(
                        "representation revision: cart unified as a board cell; dock event \
                         rewrites cells {cells:?} to observed values (learned from \
                         counterexample diff)"
                    ),
                );
                return Deliberation {
                    actions: vec![],
                    intent: Intent::Plan,
                    note: format!(
                        "Counterexample at dock: sprite-overlay representation cannot express \
                         the board reflow. Revised representation jointly with rules; learned \
                         reflow {:?}.",
                        self.reflow
                    ),
                };
            }
        }

        if !self.unified && !self.cart_docked(current) {
            // Drive the cart to the dock as an observation point (LF52).
            let dock = self.dock_cells.clone();
            let reflow = self.reflow.clone();
            let dock_goal = self.dock_cells.clone();
            let aux = WorldModel::new(
                Box::new(move |s: &Grid, a: &str| Self::cart_step(&dock, &reflow, false, s, a)),
                Box::new(move |s: &Grid| dock_goal.iter().any(|&(x, y)| s[y][x] == 4)),
                "aux: cart-on-dock probe goal",
            );
            let bfs = run_bfs(&aux, current, &ACTIONS, 100_000);
            let bfs_summary = bfs.summary();
            if let Some(plan) = bfs.plan {
                return Deliberation {
                    actions: plan,
                    intent: Intent::Experiment,
                    note: format!(
                        "Goal unreachable under current model; driving cart to dock as \
                         observation point ({bfs_summary}).",
                    ),
                };
            }
        }

        Deliberation {
            actions: vec![],
            intent: Intent::Plan,
            note: format!("Backtest {}; planning.", backtest.summary()),
        }
    }
}
