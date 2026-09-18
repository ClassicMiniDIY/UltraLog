//! Axis specifications and 2-D binning for generated tuning tables.
//!
//! An [`AxisSpec`] is a list of cell *edges* (N+1 edges give N bins), so
//! uneven spacing like a real ECU table axis is natural. Binning is
//! lower-edge inclusive: a value exactly on an edge lands in the cell that
//! starts there, and a value equal to the last edge is out of range.

use serde::{Deserialize, Serialize};

use super::stats::{mad, median, percentile};

/// Maximum bins per axis. Keeps a grid (values + counts + spreads) well
/// under the 512 KiB MCP response guard and keeps the heatmap legible.
pub const MAX_BINS_PER_AXIS: usize = 64;

/// Bin index of a value in `[min, min + range)` split into `n` equal cells,
/// clamped into range. Shared with the histogram view so both tools agree on
/// cell boundaries.
#[inline]
pub fn uniform_bin(value: f64, min: f64, range: f64, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let normalized = if range > 0.0 {
        ((value - min) / range).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    ((normalized * n as f32).floor() as usize).min(n - 1)
}

/// One table axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AxisSpec {
    /// Display label, e.g. `RPM`, `MAP`, `TPS rate`.
    pub label: String,
    /// Unit suffix for headers, e.g. `kPa`, `%/s`. Empty when unitless.
    pub unit: String,
    /// Strictly increasing cell edges. `len() - 1` bins.
    pub edges: Vec<f64>,
}

impl AxisSpec {
    /// Build an axis, sorting and de-duplicating the edges and capping the
    /// bin count at [`MAX_BINS_PER_AXIS`].
    pub fn new(label: impl Into<String>, unit: impl Into<String>, edges: Vec<f64>) -> Self {
        let mut edges: Vec<f64> = edges.into_iter().filter(|e| e.is_finite()).collect();
        edges.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        edges.dedup();
        edges.truncate(MAX_BINS_PER_AXIS + 1);
        Self {
            label: label.into(),
            unit: unit.into(),
            edges,
        }
    }

    /// Number of bins.
    pub fn bins(&self) -> usize {
        self.edges.len().saturating_sub(1)
    }

    /// Whether the axis has at least one bin.
    pub fn is_valid(&self) -> bool {
        self.bins() >= 1
    }

    /// Lower-edge-inclusive bin lookup. `None` when out of range or the value
    /// is not finite.
    pub fn bin_index(&self, value: f64) -> Option<usize> {
        if !value.is_finite() || self.edges.len() < 2 {
            return None;
        }
        let last = *self.edges.last()?;
        if value < self.edges[0] || value >= last {
            return None;
        }
        // partition_point gives the number of edges <= value; subtract one for
        // the bin whose lower edge that is.
        Some(self.edges.partition_point(|&e| e <= value) - 1)
    }

    /// Lower edge of a bin, used as the row/column header value.
    pub fn lower_edge(&self, bin: usize) -> f64 {
        self.edges[bin]
    }

    /// Centre of a bin.
    pub fn center(&self, bin: usize) -> f64 {
        (self.edges[bin] + self.edges[bin + 1]) / 2.0
    }

    /// Axis label with unit, e.g. `MAP (kPa)`.
    pub fn header(&self) -> String {
        if self.unit.is_empty() {
            self.label.clone()
        } else {
            format!("{} ({})", self.label, self.unit)
        }
    }

    /// Edges as a comma-separated list, the editable form shown in the UI.
    pub fn edges_text(&self) -> String {
        self.edges
            .iter()
            .map(|e| format_edge(*e))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Parse a comma / whitespace separated edge list. Returns `None` when
    /// fewer than two distinct finite numbers are given.
    pub fn parse_edges(text: &str) -> Option<Vec<f64>> {
        let mut edges: Vec<f64> = text
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .collect();
        edges.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        edges.dedup();
        if edges.len() < 2 { None } else { Some(edges) }
    }

    /// Uniform edges from the 1st to the 99th percentile of the data, snapped
    /// outward to multiples of `step`. Falls back to `fallback` when the data
    /// has no usable spread.
    pub fn from_data(
        label: impl Into<String>,
        unit: impl Into<String>,
        values: &[f64],
        step: f64,
        fallback: &[f64],
    ) -> Self {
        let (lo, hi) = match (percentile(values, 1.0), percentile(values, 99.0)) {
            (Some(lo), Some(hi)) if hi > lo && step > 0.0 => (lo, hi),
            _ => return Self::new(label, unit, fallback.to_vec()),
        };
        let lo = (lo / step).floor() * step;
        let hi = (hi / step).ceil() * step;
        let mut n = ((hi - lo) / step).round() as usize;
        let mut step = step;
        // Coarsen rather than exceed the bin cap.
        while n > MAX_BINS_PER_AXIS {
            step *= 2.0;
            n = ((hi - lo) / step).ceil() as usize;
        }
        if n == 0 {
            return Self::new(label, unit, fallback.to_vec());
        }
        let edges: Vec<f64> = (0..=n).map(|i| lo + i as f64 * step).collect();
        Self::new(label, unit, edges)
    }
}

fn format_edge(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// How much a cell's value can be trusted, from its sample count and spread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Confidence {
    /// No samples.
    #[default]
    Empty,
    /// Fewer than `min_samples`, or MAD/|median| above 0.5.
    Low,
    /// Enough samples with acceptable spread.
    Medium,
    /// At least `good_samples` with MAD/|median| at or below 0.25.
    High,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Derive the tier from a sample list.
    pub fn classify(
        count: usize,
        median: f64,
        mad: f64,
        min_samples: usize,
        good_samples: usize,
    ) -> Self {
        if count == 0 {
            return Self::Empty;
        }
        // Relative spread; a near-zero median with any spread counts as noisy.
        let rel = if median.abs() > 1e-9 {
            mad / median.abs()
        } else if mad > 1e-9 {
            f64::INFINITY
        } else {
            0.0
        };
        if count < min_samples || rel > 0.5 {
            Self::Low
        } else if count >= good_samples && rel <= 0.25 {
            Self::High
        } else {
            Self::Medium
        }
    }
}

/// Thresholds for [`Confidence::classify`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceRule {
    pub min_samples: usize,
    pub good_samples: usize,
}

impl Default for ConfidenceRule {
    fn default() -> Self {
        Self {
            min_samples: 3,
            good_samples: 8,
        }
    }
}

/// Statistics for one table cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellStats {
    /// Raw per-event values. Medians and MADs cannot be merged incrementally,
    /// so the samples are kept and re-derived when a log is removed.
    pub samples: Vec<f64>,
    pub median: f64,
    pub mad: f64,
    pub count: usize,
    pub confidence: Confidence,
}

impl CellStats {
    pub fn from_samples(samples: Vec<f64>, rule: ConfidenceRule) -> Self {
        let med = median(&samples).unwrap_or(0.0);
        let spread = mad(&samples).unwrap_or(0.0);
        let count = samples.iter().filter(|v| v.is_finite()).count();
        Self {
            confidence: Confidence::classify(
                count,
                med,
                spread,
                rule.min_samples,
                rule.good_samples,
            ),
            samples,
            median: med,
            mad: spread,
            count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// A binned 2-D table: `cells[row][col]` where rows follow the Y axis and
/// columns the X axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableGrid {
    pub x_axis: AxisSpec,
    pub y_axis: AxisSpec,
    pub cells: Vec<Vec<CellStats>>,
    pub rule: ConfidenceRule,
}

impl TableGrid {
    /// Bin `(x, y, value)` triples. Out-of-axis points are skipped (callers
    /// that need to count them use [`AxisSpec::bin_index`] first).
    pub fn build<I>(x_axis: AxisSpec, y_axis: AxisSpec, points: I, rule: ConfidenceRule) -> Self
    where
        I: IntoIterator<Item = (f64, f64, f64)>,
    {
        let cols = x_axis.bins();
        let rows = y_axis.bins();
        let mut buckets: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); cols]; rows];
        for (x, y, v) in points {
            if let (Some(c), Some(r)) = (x_axis.bin_index(x), y_axis.bin_index(y))
                && v.is_finite()
            {
                buckets[r][c].push(v);
            }
        }
        let cells = buckets
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|s| CellStats::from_samples(s, rule))
                    .collect()
            })
            .collect();
        Self {
            x_axis,
            y_axis,
            cells,
            rule,
        }
    }

    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    pub fn cols(&self) -> usize {
        self.cells.first().map_or(0, Vec::len)
    }

    pub fn cell(&self, row: usize, col: usize) -> Option<&CellStats> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Number of non-empty cells and number at `High` confidence.
    pub fn coverage(&self) -> (usize, usize) {
        let mut filled = 0;
        let mut high = 0;
        for c in self.cells.iter().flatten() {
            if !c.is_empty() {
                filled += 1;
            }
            if c.confidence == Confidence::High {
                high += 1;
            }
        }
        (filled, high)
    }

    /// Min and max of the non-empty cell medians, for colour scaling.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for c in self.cells.iter().flatten().filter(|c| !c.is_empty()) {
            lo = lo.min(c.median);
            hi = hi.max(c.median);
        }
        if lo.is_finite() && hi.is_finite() {
            Some((lo, hi))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_bin_matches_floor_semantics() {
        assert_eq!(uniform_bin(0.0, 0.0, 10.0, 10), 0);
        assert_eq!(uniform_bin(9.99, 0.0, 10.0, 10), 9);
        assert_eq!(uniform_bin(10.0, 0.0, 10.0, 10), 9);
        assert_eq!(uniform_bin(-5.0, 0.0, 10.0, 10), 0);
        assert_eq!(uniform_bin(5.0, 0.0, 0.0, 10), 0);
        assert_eq!(uniform_bin(5.0, 0.0, 10.0, 0), 0);
    }

    #[test]
    fn bin_index_is_lower_edge_inclusive() {
        let axis = AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0]);
        assert_eq!(axis.bins(), 2);
        assert_eq!(axis.bin_index(1000.0), Some(0));
        assert_eq!(axis.bin_index(1999.9), Some(0));
        assert_eq!(axis.bin_index(2000.0), Some(1));
        assert_eq!(axis.bin_index(3000.0), None);
        assert_eq!(axis.bin_index(999.0), None);
        assert_eq!(axis.bin_index(f64::NAN), None);
    }

    #[test]
    fn edges_are_sorted_deduped_and_capped() {
        let axis = AxisSpec::new("x", "", vec![3.0, 1.0, 2.0, 2.0, f64::NAN]);
        assert_eq!(axis.edges, vec![1.0, 2.0, 3.0]);
        let big: Vec<f64> = (0..200).map(|i| i as f64).collect();
        let axis = AxisSpec::new("x", "", big);
        assert_eq!(axis.bins(), MAX_BINS_PER_AXIS);
    }

    #[test]
    fn edges_round_trip_text() {
        let axis = AxisSpec::new("MAP", "kPa", vec![20.0, 30.5, 40.0]);
        assert_eq!(axis.edges_text(), "20, 30.5, 40");
        assert_eq!(
            AxisSpec::parse_edges("40, 20 30.5;40"),
            Some(vec![20.0, 30.5, 40.0])
        );
        assert_eq!(AxisSpec::parse_edges("40"), None);
        assert_eq!(AxisSpec::parse_edges("abc"), None);
        assert_eq!(axis.header(), "MAP (kPa)");
    }

    #[test]
    fn from_data_snaps_to_step_and_caps_bins() {
        let rpm: Vec<f64> = (0..1000).map(|i| 1100.0 + i as f64 * 4.0).collect();
        let axis = AxisSpec::from_data("RPM", "", &rpm, 500.0, &[0.0, 8000.0]);
        assert_eq!(axis.edges[0], 1000.0);
        assert_eq!(*axis.edges.last().unwrap(), 5500.0);
        assert!(axis.bins() <= MAX_BINS_PER_AXIS);
        let flat = vec![50.0; 10];
        let axis = AxisSpec::from_data("MAP", "kPa", &flat, 10.0, &[0.0, 100.0]);
        assert_eq!(axis.edges, vec![0.0, 100.0]);
        let fine: Vec<f64> = (0..10000).map(|i| i as f64).collect();
        let axis = AxisSpec::from_data("x", "", &fine, 1.0, &[0.0, 1.0]);
        assert!(axis.bins() <= MAX_BINS_PER_AXIS);
    }

    #[test]
    fn confidence_tiers() {
        let rule = ConfidenceRule::default();
        assert_eq!(
            CellStats::from_samples(vec![], rule).confidence,
            Confidence::Empty
        );
        assert_eq!(
            CellStats::from_samples(vec![100.0, 110.0], rule).confidence,
            Confidence::Low
        );
        assert_eq!(
            CellStats::from_samples(vec![100.0, 110.0, 105.0], rule).confidence,
            Confidence::Medium
        );
        let tight: Vec<f64> = (0..8).map(|i| 100.0 + i as f64).collect();
        assert_eq!(
            CellStats::from_samples(tight, rule).confidence,
            Confidence::High
        );
        let wide: Vec<f64> = vec![10.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
        assert_eq!(
            CellStats::from_samples(wide, rule).confidence,
            Confidence::Low
        );
    }

    #[test]
    fn grid_build_and_coverage() {
        let x = AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0]);
        let y = AxisSpec::new("MAP", "kPa", vec![30.0, 50.0, 70.0]);
        let points = vec![
            (1500.0, 40.0, 100.0),
            (1500.0, 40.0, 120.0),
            (1500.0, 40.0, 110.0),
            (2500.0, 60.0, 80.0),
            (9999.0, 40.0, 1.0), // out of axis
            (1500.0, 40.0, f64::NAN),
        ];
        let grid = TableGrid::build(x, y, points, ConfidenceRule::default());
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.cols(), 2);
        let c = grid.cell(0, 0).unwrap();
        assert_eq!(c.count, 3);
        assert_eq!(c.median, 110.0);
        assert_eq!(grid.cell(1, 1).unwrap().count, 1);
        assert_eq!(grid.coverage(), (2, 0));
        assert_eq!(grid.value_range(), Some((80.0, 110.0)));
    }
}
