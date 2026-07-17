//! run_bfs — breadth-first search entirely inside the verified model.
//!
//! Observables (Evidence 1C, M0R0): "breadth-first search explores 10^3-10^4
//! modeled states without spending a single environment action"; "BFS: goal
//! in 19 step(s) via level_up; expanded 3300 nodes, 891 distinct states."
//! The planner takes only the model — it cannot touch an environment.

use std::collections::{HashSet, VecDeque};

use crate::model::WorldModel;
use crate::timeline::Grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalVia {
    IsGoal,
    LevelUp,
    Win,
}

impl std::fmt::Display for GoalVia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoalVia::IsGoal => write!(f, "is_goal"),
            GoalVia::LevelUp => write!(f, "level_up"),
            GoalVia::Win => write!(f, "win"),
        }
    }
}

#[derive(Debug)]
pub struct BfsReport {
    pub plan: Option<Vec<String>>,
    pub expanded: usize,
    pub distinct: usize,
    pub goal_via: Option<GoalVia>,
}

impl BfsReport {
    pub fn found(&self) -> bool {
        self.plan.is_some()
    }

    pub fn summary(&self) -> String {
        match (&self.plan, self.goal_via) {
            (Some(p), Some(via)) => format!(
                "BFS: goal in {} step(s) via {}; expanded {} nodes, {} distinct states",
                p.len(),
                via,
                self.expanded,
                self.distinct
            ),
            _ => format!(
                "BFS: no goal found; expanded {} nodes, {} distinct states",
                self.expanded, self.distinct
            ),
        }
    }
}

pub fn run_bfs(
    model: &WorldModel,
    start: &Grid,
    actions: &[&str],
    max_nodes: usize,
) -> BfsReport {
    if model.is_goal(start) {
        return BfsReport { plan: Some(vec![]), expanded: 0, distinct: 1, goal_via: Some(GoalVia::IsGoal) };
    }

    let mut frontier: VecDeque<(Grid, Vec<String>)> = VecDeque::new();
    let mut seen: HashSet<Grid> = HashSet::new();
    frontier.push_back((start.clone(), vec![]));
    seen.insert(start.clone());
    let mut expanded = 0usize;

    while let Some((state, path)) = frontier.pop_front() {
        if expanded >= max_nodes {
            break;
        }
        expanded += 1;
        for &a in actions {
            let pred = model.step(&state, a);
            let extend = |via: GoalVia, seen: &HashSet<Grid>| BfsReport {
                plan: Some(path.iter().cloned().chain([a.to_string()]).collect()),
                expanded,
                distinct: seen.len(),
                goal_via: Some(via),
            };
            if pred.dead {
                continue;
            }
            if pred.win {
                return extend(GoalVia::Win, &seen);
            }
            if pred.level_up {
                return extend(GoalVia::LevelUp, &seen);
            }
            if model.is_goal(&pred.grid) {
                return extend(GoalVia::IsGoal, &seen);
            }
            if !seen.contains(&pred.grid) {
                seen.insert(pred.grid.clone());
                let mut next_path = path.clone();
                next_path.push(a.to_string());
                frontier.push_back((pred.grid, next_path));
            }
        }
    }

    BfsReport { plan: None, expanded, distinct: seen.len(), goal_via: None }
}
