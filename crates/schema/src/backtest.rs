//! run_backtest — exact retrodictive verification against the full Timeline.
//!
//! Observable (Evidence 1A, RE86): "393 / 393 exact after 394 steps",
//! "grid on non-terminal steps + level_up/dead/win flags on EVERY step;
//! 0 mismatch(es), 1 skipped." Flags compare on every transition; the grid
//! comparison is skipped on terminal transitions; skips are counted.

use crate::model::WorldModel;
use crate::timeline::Timeline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MismatchKind {
    Grid,
    Flags,
}

#[derive(Debug, Clone)]
pub struct Mismatch {
    pub index: usize,
    pub kind: MismatchKind,
    pub detail: String,
}

#[derive(Debug, Default)]
pub struct BacktestReport {
    pub total: usize,
    pub exact: usize,
    pub skipped: usize,
    pub mismatches: Vec<Mismatch>,
}

impl BacktestReport {
    pub fn green(&self) -> bool {
        self.mismatches.is_empty() && self.total > 0
    }

    pub fn first_mismatch(&self) -> Option<&Mismatch> {
        self.mismatches.first()
    }

    /// Reproduces the trace format:
    /// "393 / 393 exact after 394 steps; 0 mismatch(es), 1 skipped"
    pub fn summary(&self) -> String {
        format!(
            "{} / {} exact after {} steps; {} mismatch(es), {} skipped",
            self.exact,
            self.total - self.skipped,
            self.total,
            self.mismatches.len(),
            self.skipped
        )
    }
}

pub fn run_backtest(model: &WorldModel, timeline: &Timeline) -> BacktestReport {
    let mut report = BacktestReport { total: timeline.len(), ..Default::default() };

    for t in timeline {
        let pred = model.step(&t.state_before, &t.action);
        let flags_ok =
            pred.level_up == t.level_up && pred.dead == t.dead && pred.win == t.win;
        if !flags_ok {
            report.mismatches.push(Mismatch {
                index: t.index,
                kind: MismatchKind::Flags,
                detail: format!(
                    "predicted (level_up={}, dead={}, win={}) but observed \
                     (level_up={}, dead={}, win={}) at step {}",
                    pred.level_up, pred.dead, pred.win, t.level_up, t.dead, t.win, t.index
                ),
            });
            continue;
        }
        if t.terminal() {
            report.skipped += 1; // flags verified; terminal grid not required
            continue;
        }
        if pred.grid != t.state_after {
            report.mismatches.push(Mismatch {
                index: t.index,
                kind: MismatchKind::Grid,
                detail: format!(
                    "grid mismatch at step {}: {}",
                    t.index,
                    first_cell_diff(&pred.grid, &t.state_after)
                ),
            });
            continue;
        }
        report.exact += 1;
    }
    report
}

fn first_cell_diff(pred: &crate::timeline::Grid, real: &crate::timeline::Grid) -> String {
    if pred.len() != real.len() {
        return format!("row count {} != {}", pred.len(), real.len());
    }
    for (y, (pr, rr)) in pred.iter().zip(real).enumerate() {
        if pr.len() != rr.len() {
            return format!("row {} width {} != {}", y, pr.len(), rr.len());
        }
        for (x, (pv, rv)) in pr.iter().zip(rr).enumerate() {
            if pv != rv {
                return format!("cell ({x},{y}) predicted {pv}, observed {rv}");
            }
        }
    }
    "grids differ (no cell-level diff found)".to_string()
}
