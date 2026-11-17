//! 2-D binning for the Scatter Plot tool (issue #114).
//!
//! [`bin_scatter`] is the one place the scatter plot's cells are computed. The on-screen view
//! (`ui/scatter_plot.rs`, 512 bins per axis) and the export (`ui/export.rs`, a coarser grid)
//! both call it, so they agree on bounds, cell boundaries ([`uniform_bin`]) and which samples
//! count.
//!
//! Every cell counts its hits. With a Z channel, a cell also keeps the mean, minimum and
//! maximum of the Z samples that landed in it, so the view can switch statistic without
//! rebinning.

use super::tables::binning::{finite_bounds, uniform_bin};
use super::tables::events::is_invalid_sample;

/// Which statistic of the Z samples in a cell colours it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZStat {
    /// Mean of the cell's Z samples.
    #[default]
    Mean,
    /// Smallest Z sample in the cell (for AFR / λ: the richest reading).
    Min,
    /// Largest Z sample in the cell (for AFR / λ: the leanest reading).
    Max,
}

impl ZStat {
    /// All statistics, in the order the UI offers them.
    pub const ALL: [ZStat; 3] = [ZStat::Mean, ZStat::Min, ZStat::Max];
}

/// Why a scatter grid could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScatterError {
    /// No samples, or no finite X / Y values to bound the axes.
    NoData,
    /// The X, Y (and Z) series are not the same length, so zipping them would pair samples
    /// from different records (same contract as `require_aligned` in the IPC handler).
    MismatchedLengths,
}

/// Per-cell Z statistics, row-major like [`ScatterGrid::hits`]. A cell with no valid Z sample
/// has `count == 0` and `NaN` statistics.
#[derive(Clone, Debug)]
pub struct ZPlanes {
    pub mean: Vec<f32>,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    /// Valid Z samples in the cell. Can be lower than the cell's hits when some Z samples are
    /// missing (NaN) or ECU "no reading" sentinels.
    pub count: Vec<u32>,
}

impl ZPlanes {
    /// The statistic plane for `stat`.
    pub fn plane(&self, stat: ZStat) -> &[f32] {
        match stat {
            ZStat::Mean => &self.mean,
            ZStat::Min => &self.min,
            ZStat::Max => &self.max,
        }
    }
}

/// A binned scatter plot: `bins × bins` cells, row-major with row 0 at the lowest Y.
#[derive(Clone, Debug)]
pub struct ScatterGrid {
    pub bins: usize,
    /// Samples per cell (`hits[y * bins + x]`).
    pub hits: Vec<u32>,
    pub max_hits: u32,
    /// Z statistics, when a Z channel was given.
    pub z: Option<ZPlanes>,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl ScatterGrid {
    /// Row-major index of cell (`x_bin`, `y_bin`).
    #[inline]
    pub fn index(&self, x_bin: usize, y_bin: usize) -> usize {
        y_bin * self.bins + x_bin
    }

    /// Width of the X axis, never zero (a constant channel spans 1.0).
    pub fn x_range(&self) -> f64 {
        nonzero(self.x_max - self.x_min)
    }

    /// Height of the Y axis, never zero (a constant channel spans 1.0).
    pub fn y_range(&self) -> f64 {
        nonzero(self.y_max - self.y_min)
    }

    /// Cell of a value pair, by the same rule the samples were binned with.
    pub fn cell_of(&self, x: f64, y: f64) -> (usize, usize) {
        (
            uniform_bin(x, self.x_min, self.x_range(), self.bins),
            uniform_bin(y, self.y_min, self.y_range(), self.bins),
        )
    }

    /// The `stat` value of the cell at `idx` and how many Z samples it summarises, or `None`
    /// when the grid has no Z channel or the cell holds no valid Z sample.
    pub fn z_value(&self, idx: usize, stat: ZStat) -> Option<(f32, u32)> {
        let z = self.z.as_ref()?;
        let count = *z.count.get(idx)?;
        (count > 0).then(|| (z.plane(stat)[idx], count))
    }

    /// Smallest and largest `stat` value over the cells that hold Z samples, or `None` when
    /// none do.
    pub fn z_bounds(&self, stat: ZStat) -> Option<(f64, f64)> {
        let z = self.z.as_ref()?;
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for (&v, &n) in z.plane(stat).iter().zip(&z.count) {
            if n > 0 {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo <= hi).then_some((lo as f64, hi as f64))
    }
}

/// Bin `x` against `y` into `bins × bins` cells, with optional per-cell statistics of `z`.
///
/// - Axis bounds are the finite min/max of each series; samples whose X or Y is not finite
///   are skipped entirely.
/// - A Z sample that is not finite or is an ECU sentinel (`is_invalid_sample`) still counts as
///   a hit (the X/Y pair is real) but does not enter the cell's Z statistics.
/// - All series must be the same length ([`ScatterError::MismatchedLengths`]).
pub fn bin_scatter(
    x: &[f64],
    y: &[f64],
    z: Option<&[f64]>,
    bins: usize,
) -> Result<ScatterGrid, ScatterError> {
    if x.len() != y.len() || z.is_some_and(|z| z.len() != x.len()) {
        return Err(ScatterError::MismatchedLengths);
    }
    if x.is_empty() || bins == 0 {
        return Err(ScatterError::NoData);
    }
    let (x_min, x_max) = finite_bounds(x).ok_or(ScatterError::NoData)?;
    let (y_min, y_max) = finite_bounds(y).ok_or(ScatterError::NoData)?;
    let (x_range, y_range) = (nonzero(x_max - x_min), nonzero(y_max - y_min));

    let cells = bins * bins;
    let mut hits = vec![0u32; cells];
    let mut z_acc = z.map(|_| {
        (
            vec![0.0f64; cells],            // sum
            vec![f64::INFINITY; cells],     // min
            vec![f64::NEG_INFINITY; cells], // max
            vec![0u32; cells],              // count
        )
    });

    for i in 0..x.len() {
        let (xv, yv) = (x[i], y[i]);
        if !xv.is_finite() || !yv.is_finite() {
            continue;
        }
        let idx =
            uniform_bin(yv, y_min, y_range, bins) * bins + uniform_bin(xv, x_min, x_range, bins);
        hits[idx] += 1;
        if let (Some(z), Some((sum, min, max, count))) = (z, z_acc.as_mut()) {
            let zv = z[i];
            if !is_invalid_sample(zv) {
                sum[idx] += zv;
                min[idx] = min[idx].min(zv);
                max[idx] = max[idx].max(zv);
                count[idx] += 1;
            }
        }
    }

    let max_hits = hits.iter().copied().max().unwrap_or(0);
    let z = z_acc.map(|(sum, min, max, count)| {
        let finish = |v: &[f64]| -> Vec<f32> {
            v.iter()
                .zip(&count)
                .map(|(&v, &n)| if n > 0 { v as f32 } else { f32::NAN })
                .collect()
        };
        let mean: Vec<f64> = sum
            .iter()
            .zip(&count)
            .map(|(&s, &n)| if n > 0 { s / n as f64 } else { f64::NAN })
            .collect();
        ZPlanes {
            mean: finish(&mean),
            min: finish(&min),
            max: finish(&max),
            count,
        }
    });

    Ok(ScatterGrid {
        bins,
        hits,
        max_hits,
        z,
        x_min,
        x_max,
        y_min,
        y_max,
    })
}

fn nonzero(range: f64) -> f64 {
    if range.abs() < f64::EPSILON {
        1.0
    } else {
        range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_hits_per_cell() {
        // 0..=1 on both axes in a 2×2 grid: (0,0) and (0.2,0.1) share the bottom-left cell.
        let x = [0.0, 0.2, 1.0, 1.0];
        let y = [0.0, 0.1, 1.0, 0.0];
        let g = bin_scatter(&x, &y, None, 2).unwrap();
        assert_eq!(g.hits[g.index(0, 0)], 2);
        assert_eq!(g.hits[g.index(1, 1)], 1);
        assert_eq!(g.hits[g.index(1, 0)], 1);
        assert_eq!(g.hits[g.index(0, 1)], 0);
        assert_eq!(g.max_hits, 2);
        assert!(g.z.is_none());
    }

    #[test]
    fn z_statistics_per_cell() {
        let x = [0.0, 0.1, 0.2, 1.0];
        let y = [0.0, 0.1, 0.2, 1.0];
        let z = [12.0, 14.0, 13.0, 11.0];
        let g = bin_scatter(&x, &y, Some(&z), 2).unwrap();
        let low = g.index(0, 0);
        assert_eq!(g.z_value(low, ZStat::Mean), Some((13.0, 3)));
        assert_eq!(g.z_value(low, ZStat::Min), Some((12.0, 3)));
        assert_eq!(g.z_value(low, ZStat::Max), Some((14.0, 3)));
        assert_eq!(g.z_value(g.index(1, 1), ZStat::Mean), Some((11.0, 1)));
        assert_eq!(g.z_value(g.index(0, 1), ZStat::Mean), None);
        assert_eq!(g.z_bounds(ZStat::Mean), Some((11.0, 13.0)));
        assert_eq!(g.z_bounds(ZStat::Max), Some((11.0, 14.0)));
    }

    #[test]
    fn non_finite_x_or_y_is_skipped() {
        let x = [0.0, f64::NAN, 1.0, 0.5];
        let y = [0.0, 0.5, 1.0, f64::INFINITY];
        let g = bin_scatter(&x, &y, None, 4).unwrap();
        assert_eq!(g.hits.iter().sum::<u32>(), 2);
        assert_eq!((g.x_min, g.x_max, g.y_min, g.y_max), (0.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn invalid_z_counts_as_hit_but_not_in_statistics() {
        let x = [0.0, 0.0, 0.0];
        let y = [0.0, 0.0, 0.0];
        let z = [14.7, f64::NAN, -2_147_483_617.0];
        let g = bin_scatter(&x, &y, Some(&z), 8).unwrap();
        let idx = g.index(0, 0);
        assert_eq!(g.hits[idx], 3);
        assert_eq!(g.z_value(idx, ZStat::Min), Some((14.7, 1)));
    }

    #[test]
    fn constant_channel_does_not_divide_by_zero() {
        let x = [5.0, 5.0, 5.0];
        let y = [1.0, 2.0, 3.0];
        let g = bin_scatter(&x, &y, None, 4).unwrap();
        assert_eq!(g.x_range(), 1.0);
        assert_eq!(g.hits.iter().sum::<u32>(), 3);
    }

    #[test]
    fn mismatched_lengths_are_refused() {
        assert_eq!(
            bin_scatter(&[1.0, 2.0], &[1.0], None, 4).unwrap_err(),
            ScatterError::MismatchedLengths
        );
        assert_eq!(
            bin_scatter(&[1.0, 2.0], &[1.0, 2.0], Some(&[1.0]), 4).unwrap_err(),
            ScatterError::MismatchedLengths
        );
    }

    #[test]
    fn no_data_is_refused() {
        assert_eq!(
            bin_scatter(&[], &[], None, 4).unwrap_err(),
            ScatterError::NoData
        );
        assert_eq!(
            bin_scatter(&[f64::NAN], &[1.0], None, 4).unwrap_err(),
            ScatterError::NoData
        );
    }

    #[test]
    fn cell_of_matches_binning() {
        let x = [0.0, 100.0];
        let y = [0.0, 10.0];
        let g = bin_scatter(&x, &y, None, 10).unwrap();
        assert_eq!(g.cell_of(0.0, 0.0), (0, 0));
        assert_eq!(g.cell_of(100.0, 10.0), (9, 9));
        assert_eq!(g.cell_of(55.0, 5.5), (5, 5));
    }
}
