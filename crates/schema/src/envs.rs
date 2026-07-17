//! Hidden-rule demo environments standing in for ARC-AGI-3 games.
//!
//! Color legend (env-internal truth, NOT told to agents):
//! 0 floor · 1 player · 3 goal (win on entry) · 4 cart · 5 wall · 6 dock
//! 7 mystery cell (ToggleMaze: secretly passable)

use crate::executor::{Environment, Observation};
use crate::timeline::Grid;

pub const ACTIONS: [&str; 4] = ["up", "down", "left", "right"];

pub fn delta(action: &str) -> (i32, i32) {
    match action {
        "up" => (0, -1),
        "down" => (0, 1),
        "left" => (-1, 0),
        "right" => (1, 0),
        _ => (0, 0),
    }
}

pub fn find_color(grid: &Grid, color: u8) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for (y, row) in grid.iter().enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if v == color {
                out.push((x, y));
            }
        }
    }
    out
}

fn player(grid: &Grid) -> (usize, usize) {
    find_color(grid, 1)[0]
}

fn target_of(grid: &Grid, action: &str) -> (usize, usize, usize, usize) {
    let (px, py) = player(grid);
    let (dx, dy) = delta(action);
    let tx = (px as i32 + dx) as usize;
    let ty = (py as i32 + dy) as usize;
    (px, py, tx, ty)
}

macro_rules! base_env_impl {
    () => {
        fn reset(&mut self) -> Observation {
            self.grid = Self::LAYOUT.iter().map(|r| r.to_vec()).collect();
            Observation::of(self.grid.clone())
        }

        fn actions_taken(&self) -> usize {
            self.actions_taken
        }

        fn legal_actions(&self) -> &[&'static str] {
            &ACTIONS
        }
    };
}

/// A corridor blocked by a mystery color 7. Hidden truth: 7 is passable
/// (cf. Evidence 2A: a search proved the target unreachable on an
/// incomplete model — the graph was missing an edge).
pub struct ToggleMaze {
    grid: Grid,
    actions_taken: usize,
}

impl ToggleMaze {
    const LAYOUT: [[u8; 7]; 3] =
        [[5, 5, 5, 5, 5, 5, 5], [5, 1, 0, 7, 0, 3, 5], [5, 5, 5, 5, 5, 5, 5]];

    pub fn new() -> Self {
        let mut env = Self { grid: vec![], actions_taken: 0 };
        env.reset();
        env
    }
}

impl Default for ToggleMaze {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment for ToggleMaze {
    base_env_impl!();

    fn step(&mut self, action: &str) -> Observation {
        self.actions_taken += 1;
        let (px, py, tx, ty) = target_of(&self.grid, action);
        match self.grid[ty][tx] {
            0 | 7 => {
                self.grid[py][px] = 0;
                self.grid[ty][tx] = 1;
                Observation::of(self.grid.clone())
            }
            3 => {
                self.grid[py][px] = 0;
                self.grid[ty][tx] = 1;
                Observation::win(self.grid.clone())
            }
            _ => Observation::of(self.grid.clone()),
        }
    }
}

/// Sokoban-style cart on a track. Hidden rule: when the cart is pushed onto
/// the dock (6), a gate wall elsewhere opens — "the board reflows when the
/// cart docks" (cf. Evidence 2B, LF52).
pub struct CartDock {
    grid: Grid,
    actions_taken: usize,
}

impl CartDock {
    const LAYOUT: [[u8; 7]; 5] = [
        [5, 5, 5, 5, 5, 5, 5],
        [5, 1, 0, 4, 0, 6, 5],
        [5, 5, 5, 5, 5, 5, 5], // gate at (3, 2) — opens on dock
        [5, 5, 5, 3, 5, 5, 5],
        [5, 5, 5, 5, 5, 5, 5],
    ];
    pub const GATE: (usize, usize) = (3, 2);

    pub fn new() -> Self {
        let mut env = Self { grid: vec![], actions_taken: 0 };
        env.reset();
        env
    }
}

impl Default for CartDock {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment for CartDock {
    base_env_impl!();

    fn step(&mut self, action: &str) -> Observation {
        self.actions_taken += 1;
        let (px, py, tx, ty) = target_of(&self.grid, action);
        let (dx, dy) = delta(action);
        match self.grid[ty][tx] {
            0 => {
                self.grid[py][px] = 0;
                self.grid[ty][tx] = 1;
                Observation::of(self.grid.clone())
            }
            3 => {
                self.grid[py][px] = 0;
                self.grid[ty][tx] = 1;
                Observation::win(self.grid.clone())
            }
            4 => {
                let bx = (tx as i32 + dx) as usize;
                let by = (ty as i32 + dy) as usize;
                let beyond = self.grid[by][bx];
                if beyond == 0 || beyond == 6 {
                    self.grid[by][bx] = 4;
                    self.grid[ty][tx] = 1;
                    self.grid[py][px] = 0;
                    if beyond == 6 {
                        // hidden board reflow
                        let (gx, gy) = Self::GATE;
                        self.grid[gy][gx] = 0;
                    }
                }
                Observation::of(self.grid.clone())
            }
            _ => Observation::of(self.grid.clone()), // wall / empty dock block
        }
    }
}
