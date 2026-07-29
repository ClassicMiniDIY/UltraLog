//! Lap detection from a GPS track.
//!
//! Strategy: a single proximity gate around the first finite GPS point. We
//! detect a lap completion when the car re-enters the gate (after having
//! left it for at least `min_lap_seconds`). Distances are computed in local
//! meters using an equirectangular projection scaled by `cos(mid_latitude)`,
//! which is more than accurate enough for sub-100 m gate radii.
//!
//! This isn't a full Motec-style start/finish line algorithm - it's the
//! pragmatic v1 that works without any user-placed reference. Sectors and
//! crossing-line detection are explicit non-goals here.

use std::f64::consts::PI;

/// Tunable parameters for lap detection.
#[derive(Debug, Clone, Copy)]
pub struct GateParams {
    /// Inner radius (m): once the car is inside this distance from the gate
    /// origin, it counts as "armed" and the next exit/re-entry can close a lap.
    pub inner_radius_m: f64,
    /// Outer radius (m): the car must travel at least this far from the gate
    /// origin before re-entry counts. Acts as hysteresis.
    pub outer_radius_m: f64,
    /// Minimum lap duration (s). Anything shorter is treated as gate dwell
    /// (e.g. car stopped near the start).
    pub min_lap_seconds: f64,
}

impl Default for GateParams {
    fn default() -> Self {
        Self {
            inner_radius_m: 10.0,
            outer_radius_m: 30.0,
            min_lap_seconds: 20.0,
        }
    }
}

/// One detected lap.
#[derive(Debug, Clone, PartialEq)]
pub struct LapInfo {
    pub index: usize,
    pub start_record: usize,
    pub end_record: usize,
    pub duration_s: f64,
}

const EARTH_RADIUS_M: f64 = 6_378_137.0;
const MAX_GPS_SPEED_MPS: f64 = 300.0;
const GPS_JUMP_ALLOWANCE_M: f64 = 500.0;
const MAX_SPEED_CHECK_SECONDS: f64 = 5.0;

/// Format a duration in seconds as `m:ss.fff` (no leading zero on minutes).
pub fn fmt_mmssms(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "–".to_string();
    }
    let total_ms = (seconds * 1000.0).round() as u64;
    let minutes = total_ms / 60_000;
    let secs = (total_ms % 60_000) / 1000;
    let millis = total_ms % 1000;
    format!("{minutes}:{secs:02}.{millis:03}")
}

/// Detect laps from latitude/longitude/time triples.
///
/// `lats`, `lons`, and `times` must be the same length. Non-finite samples
/// are skipped (they don't break the state machine - they just don't
/// advance it).
pub fn detect_laps(lats: &[f64], lons: &[f64], times: &[f64], params: GateParams) -> Vec<LapInfo> {
    let n = lats.len().min(lons.len()).min(times.len());
    if n < 2 {
        return Vec::new();
    }

    // Find the first valid point. This becomes the gate origin.
    let mut gate_idx = None;
    for i in 0..n {
        if is_valid_gps_point(lats[i], lons[i]) && times[i].is_finite() {
            gate_idx = Some(i);
            break;
        }
    }
    let gate_idx = match gate_idx {
        Some(i) => i,
        None => return Vec::new(),
    };

    let gate_lat = lats[gate_idx];
    let gate_lon = lons[gate_idx];
    let cos_mid = (gate_lat * PI / 180.0).cos();
    let m_per_deg_lat = EARTH_RADIUS_M * PI / 180.0;
    let m_per_deg_lon = m_per_deg_lat * cos_mid;

    let dist_m = |lat: f64, lon: f64| -> f64 {
        let dy = (lat - gate_lat) * m_per_deg_lat;
        let dx = longitude_delta_degrees(gate_lon, lon) * m_per_deg_lon;
        (dx * dx + dy * dy).sqrt()
    };

    let mut laps = Vec::new();
    let mut lap_start = gate_idx;
    let mut lap_start_time = times[gate_idx];
    // State: have we left the outer radius since `lap_start`?
    let mut went_outside = false;
    let mut lap_index = 0usize;

    for i in (gate_idx + 1)..n {
        let lat = lats[i];
        let lon = lons[i];
        let t = times[i];
        if !is_valid_gps_point(lat, lon) || !t.is_finite() {
            continue;
        }

        let d = dist_m(lat, lon);

        if !went_outside {
            if d > params.outer_radius_m {
                went_outside = true;
            }
            continue;
        }

        // Already left the gate area; watch for re-entry.
        if d <= params.inner_radius_m {
            let duration = t - lap_start_time;
            if duration >= params.min_lap_seconds {
                laps.push(LapInfo {
                    index: lap_index,
                    start_record: lap_start,
                    end_record: i,
                    duration_s: duration,
                });
                lap_index += 1;
                lap_start = i;
                lap_start_time = t;
                went_outside = false;
            } else {
                // Treat an early crossing as a new start candidate. Requiring
                // another exit prevents a vehicle that stops inside the gate
                // from eventually being counted as a completed lap.
                lap_start = i;
                lap_start_time = t;
                went_outside = false;
            }
        }
    }

    laps
}

/// Return whether a latitude/longitude pair can represent a GPS fix.
pub fn is_valid_gps_point(latitude: f64, longitude: f64) -> bool {
    latitude.is_finite()
        && longitude.is_finite()
        && (-90.0..=90.0).contains(&latitude)
        && (-180.0..=180.0).contains(&longitude)
        && !(latitude.abs() <= 1e-9 && longitude.abs() <= 1e-9)
}

/// Replace invalid fixes and isolated implausible jumps with NaN sentinels.
/// A new cluster of consecutive plausible samples starts a new track segment.
pub fn sanitize_gps_track(lats: &mut [f64], lons: &mut [f64], times: &[f64]) {
    let n = lats.len().min(lons.len()).min(times.len());
    let candidates: Vec<(usize, f64, f64, f64)> = (0..n)
        .filter_map(|index| {
            let latitude = lats[index];
            let longitude = lons[index];
            is_valid_gps_point(latitude, longitude).then_some((
                index,
                latitude,
                longitude,
                times[index],
            ))
        })
        .collect();

    for index in 0..lats.len().min(lons.len()) {
        lats[index] = f64::NAN;
        lons[index] = f64::NAN;
    }
    if candidates.is_empty() {
        return;
    }

    let anchor_position = candidates
        .windows(2)
        .position(|pair| plausible_gps_step(pair[0], pair[1]))
        .unwrap_or(0);
    let mut last_accepted = None;

    for position in anchor_position..candidates.len() {
        let candidate = candidates[position];
        let accepted = last_accepted
            .map(|previous| plausible_gps_step(previous, candidate))
            .unwrap_or(true);
        if accepted {
            lats[candidate.0] = candidate.1;
            lons[candidate.0] = candidate.2;
            last_accepted = Some(candidate);
            continue;
        }

        if candidates
            .get(position + 1)
            .is_some_and(|next| plausible_gps_step(candidate, *next))
        {
            last_accepted = None;
        }
    }
}

/// Return the signed shortest angular distance from one longitude to another.
pub fn longitude_delta_degrees(from: f64, to: f64) -> f64 {
    let delta = (to - from).rem_euclid(360.0);
    if delta > 180.0 { delta - 360.0 } else { delta }
}

fn plausible_gps_step(previous: (usize, f64, f64, f64), current: (usize, f64, f64, f64)) -> bool {
    let delta_seconds = if previous.3.is_finite() && current.3.is_finite() {
        (current.3 - previous.3).clamp(0.0, MAX_SPEED_CHECK_SECONDS)
    } else {
        0.0
    };
    let maximum_distance = GPS_JUMP_ALLOWANCE_M + MAX_GPS_SPEED_MPS * delta_seconds;
    gps_distance_m(previous.1, previous.2, current.1, current.2) <= maximum_distance
}

fn gps_distance_m(lat_a: f64, lon_a: f64, lat_b: f64, lon_b: f64) -> f64 {
    let mid_latitude = ((lat_a + lat_b) * 0.5).to_radians();
    let dy = (lat_b - lat_a).to_radians() * EARTH_RADIUS_M;
    let dx =
        longitude_delta_degrees(lon_a, lon_b).to_radians() * EARTH_RADIUS_M * mid_latitude.cos();
    dx.hypot(dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_track(
        center_lat: f64,
        center_lon: f64,
        radius_m: f64,
        laps: usize,
        samples_per_lap: usize,
        lap_seconds: f64,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut lats = Vec::new();
        let mut lons = Vec::new();
        let mut times = Vec::new();

        let m_per_deg_lat = EARTH_RADIUS_M * PI / 180.0;
        let m_per_deg_lon = m_per_deg_lat * (center_lat * PI / 180.0).cos();

        let total = laps * samples_per_lap;
        for i in 0..=total {
            let angle = 2.0 * PI * (i as f64 / samples_per_lap as f64);
            let dx = radius_m * angle.cos();
            let dy = radius_m * angle.sin();
            let lat = center_lat + dy / m_per_deg_lat;
            let lon = center_lon + dx / m_per_deg_lon;
            let t = lap_seconds * (i as f64 / samples_per_lap as f64);
            lats.push(lat);
            lons.push(lon);
            times.push(t);
        }
        (lats, lons, times)
    }

    #[test]
    fn detects_three_full_laps_on_circle() {
        let (lats, lons, times) = circle_track(45.0, 9.0, 200.0, 3, 200, 90.0);
        let laps = detect_laps(&lats, &lons, &times, GateParams::default());
        assert_eq!(
            laps.len(),
            3,
            "expected 3 laps, got {}: {:?}",
            laps.len(),
            laps
        );
        for lap in &laps {
            assert!(
                (lap.duration_s - 90.0).abs() < 1.0,
                "lap duration {} should be ~90s",
                lap.duration_s
            );
        }
    }

    #[test]
    fn ignores_partial_trailing_lap() {
        let (mut lats, mut lons, mut times) = circle_track(45.0, 9.0, 200.0, 2, 200, 90.0);
        // Append a partial 3rd lap (only 1/4 of the way around).
        let extra = 50;
        let m_per_deg_lat = EARTH_RADIUS_M * PI / 180.0;
        let m_per_deg_lon = m_per_deg_lat * (45.0_f64 * PI / 180.0).cos();
        let last_t = *times.last().unwrap();
        for i in 1..=extra {
            let angle = 2.0 * PI * (i as f64 / 200.0);
            let dx = 200.0 * angle.cos();
            let dy = 200.0 * angle.sin();
            lats.push(45.0 + dy / m_per_deg_lat);
            lons.push(9.0 + dx / m_per_deg_lon);
            times.push(last_t + 90.0 * (i as f64 / 200.0));
        }
        let laps = detect_laps(&lats, &lons, &times, GateParams::default());
        assert_eq!(laps.len(), 2);
    }

    #[test]
    fn empty_input_yields_empty_laps() {
        let laps = detect_laps(&[], &[], &[], GateParams::default());
        assert!(laps.is_empty());
    }

    #[test]
    fn all_non_finite_yields_empty() {
        let laps = detect_laps(
            &[f64::NAN, f64::NAN],
            &[f64::NAN, f64::NAN],
            &[0.0, 1.0],
            GateParams::default(),
        );
        assert!(laps.is_empty());
    }

    #[test]
    fn dwelling_at_start_does_not_close_lap() {
        // 30 samples within 5 m of the start (car stopped), no laps should
        // close even though we're "near the gate".
        let lats = vec![45.0_f64; 30];
        let lons = vec![9.0_f64; 30];
        let times: Vec<f64> = (0..30).map(|i| i as f64 * 0.1).collect();
        let laps = detect_laps(&lats, &lons, &times, GateParams::default());
        assert!(laps.is_empty());
    }

    #[test]
    fn early_reentry_then_dwell_does_not_close_lap() {
        let latitude = 45.0_f64;
        let longitude = 9.0_f64;
        let meters_per_degree_lon = EARTH_RADIUS_M * PI / 180.0 * latitude.to_radians().cos();
        let outside_longitude = longitude + 40.0 / meters_per_degree_lon;

        let mut lats = vec![latitude, latitude];
        let mut lons = vec![longitude, outside_longitude];
        let mut times = vec![0.0, 1.0];
        for second in 2..=30 {
            lats.push(latitude);
            lons.push(longitude);
            times.push(second as f64);
        }

        let laps = detect_laps(&lats, &lons, &times, GateParams::default());
        assert!(laps.is_empty());
    }

    #[test]
    fn fmt_mmssms_basic() {
        assert_eq!(fmt_mmssms(0.0), "0:00.000");
        assert_eq!(fmt_mmssms(1.234), "0:01.234");
        assert_eq!(fmt_mmssms(65.5), "1:05.500");
        assert_eq!(fmt_mmssms(125.0), "2:05.000");
        assert_eq!(fmt_mmssms(f64::NAN), "–");
        assert_eq!(fmt_mmssms(-1.0), "–");
    }

    #[test]
    fn gps_validation_rejects_sentinels_and_out_of_range_values() {
        assert!(!is_valid_gps_point(0.0, 0.0));
        assert!(!is_valid_gps_point(91.0, 10.0));
        assert!(!is_valid_gps_point(45.0, 181.0));
        assert!(is_valid_gps_point(45.0, 9.0));
    }

    #[test]
    fn sanitization_removes_isolated_jump_and_recovers_track() {
        let mut lats = vec![10.0, 45.0, 45.0001, 20.0, 45.0002, 45.0003];
        let mut lons = vec![10.0, 9.0, 9.0001, 20.0, 9.0002, 9.0003];
        let times = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];

        sanitize_gps_track(&mut lats, &mut lons, &times);

        assert!(lats[0].is_nan());
        assert!(lats[3].is_nan());
        assert_eq!(lats[1], 45.0);
        assert_eq!(lats[5], 45.0003);
    }

    #[test]
    fn sanitization_accepts_antimeridian_crossing() {
        let mut lats = vec![0.1, 0.1, 0.1];
        let mut lons = vec![179.999, -179.999, -179.998];
        let times = vec![0.0, 1.0, 2.0];

        sanitize_gps_track(&mut lats, &mut lons, &times);

        assert!(lats.iter().all(|value| value.is_finite()));
        assert!((longitude_delta_degrees(179.999, -179.999) - 0.002).abs() < 1e-9);
    }

    #[test]
    fn lap_gate_distance_uses_shortest_longitude_delta() {
        let lats = vec![0.1, 0.1, 0.1];
        let lons = vec![179.9999, -179.9999, 179.9999];
        let times = vec![0.0, 20.0, 40.0];

        let laps = detect_laps(&lats, &lons, &times, GateParams::default());

        assert!(laps.is_empty());
    }
}
