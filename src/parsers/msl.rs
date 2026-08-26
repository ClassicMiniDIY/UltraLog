//! TunerStudio MSL (legacy ASCII datalog) parser.
//!
//! `.msl` is the tab-delimited "MegaSquirt log" text format written by
//! TunerStudio's legacy ASCII data logger and by third-party dashes that
//! speak the same dialect — RealDash, for one, logs a Speeduino over
//! Bluetooth straight into this format.
//!
//! Format:
//! - Optional preamble lines (firmware banner, `"Capture Date: ..."`)
//! - Tab-delimited header row whose first column is `Time`
//! - A second row carrying the unit for each column (`sec`, `RPM`, `kPa`, ...)
//! - Tab-delimited data rows
//! - Optional `MARK` lines written when the user tags a point in the log
//!
//! Two behaviors here are load-bearing:
//!
//! - **Non-numeric fields fall back to the last known value.** MSL columns
//!   are not all numeric — RealDash writes `GPS Date` as `18.8.2026` and
//!   leaves fields blank before the GPS gets a fix. Substituting the last
//!   value keeps column alignment, matching `parsers/haltech.rs` and
//!   `parsers/ecumaster.rs`. Clock-style fields (`HH:MM`, `HH:MM:SS`) are
//!   the exception: they convert to seconds since midnight, which plots.
//! - **Times are forced monotonic.** A `.msl` can concatenate sessions, so
//!   the `Time` column can jump backwards. Backwards jumps accumulate an
//!   offset, because computed-channel time-shift lookups binary-search the
//!   `times` vector.
//!
//! Reference: <https://www.tunerstudio.com>

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// How many leading lines may precede the header row before detection
/// gives up. Real exports carry at most a banner and a capture date.
const MAX_PREAMBLE_LINES: usize = 8;

/// Unit strings accepted in the units row under the `Time` column.
const TIME_UNITS: &[&str] = &[
    "sec",
    "secs",
    "s",
    "seconds",
    "ms",
    "msec",
    "msecs",
    "milliseconds",
];

/// MSL log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct MslMeta {
    /// Preamble lines found ahead of the header row (banner, capture date)
    pub header_lines: Vec<String>,
    /// Number of channels in the log (excludes the Time column)
    pub channel_count: usize,
    /// Number of parsed data points
    pub data_points: usize,
}

/// MSL channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct MslChannel {
    /// Column name exactly as exported (e.g. "SPK: Spark Advance")
    pub name: String,
    /// Unit from the units row (e.g. "kPa", "deg", "AFR")
    pub unit: String,
}

impl MslChannel {
    /// Create a channel from a header column and its units-row entry.
    pub fn new(name: &str, unit: &str) -> Self {
        Self {
            name: name.trim().to_string(),
            unit: unit.trim().to_string(),
        }
    }

    /// Get the display unit
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Locate the header/units row pair, skipping any preamble.
///
/// Returns `(header_line, units_line, remaining_lines_offset_in_bytes)` —
/// the offset lets `parse` resume at the first data row without re-splitting.
fn locate_header(contents: &str) -> Option<(&str, &str, usize)> {
    let mut offset = 0usize;
    let mut pending: Option<&str> = None;

    for raw in contents.split_inclusive('\n').take(MAX_PREAMBLE_LINES + 1) {
        let line = raw.trim_end_matches(['\r', '\n']);
        let line_end = offset + raw.len();
        offset = line_end;

        if let Some(header) = pending.take() {
            // The line after a header candidate must be the units row.
            if is_units_row(header, line) {
                return Some((header, line, line_end));
            }
        }

        if is_header_row(line) {
            pending = Some(line);
        }
    }

    None
}

/// A header row is tab-delimited with `Time` as its first column.
fn is_header_row(line: &str) -> bool {
    let mut fields = line.split('\t');
    let first = fields.next().unwrap_or("").trim();
    first.eq_ignore_ascii_case("time") && fields.next().is_some()
}

/// A units row has the same field count as its header and names a time
/// unit under the `Time` column.
fn is_units_row(header: &str, line: &str) -> bool {
    if !line.contains('\t') {
        return false;
    }
    if line.split('\t').count() != header.split('\t').count() {
        return false;
    }
    let first = line.split('\t').next().unwrap_or("").trim().to_lowercase();
    TIME_UNITS.contains(&first.as_str())
}

/// Seconds-per-unit for the time column, read from the units row.
fn time_scale(unit: &str) -> f64 {
    match unit.trim().to_lowercase().as_str() {
        "ms" | "msec" | "msecs" | "milliseconds" => 0.001,
        _ => 1.0,
    }
}

/// TunerStudio marker line written when the user tags a point in the log.
fn is_mark_line(line: &str) -> bool {
    line.trim_start().to_uppercase().starts_with("MARK")
}

/// MSL log file parser
pub struct Msl;

impl Msl {
    /// Detect if file contents look like a TunerStudio MSL datalog.
    ///
    /// Requires the header/units row pair *and* a first data row whose time
    /// field parses as a number, which keeps this from claiming other
    /// tab-delimited `Time`-first exports.
    pub fn detect(contents: &str) -> bool {
        let contents = contents.trim_start_matches('\u{FEFF}');
        let Some((header, _units, data_offset)) = locate_header(contents) else {
            return false;
        };

        for line in contents[data_offset..].lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || is_mark_line(line) {
                continue;
            }
            let first = line.split('\t').next().unwrap_or("").trim();
            return first.parse::<f64>().is_ok() && line.split('\t').count() > 1;
        }

        // Header and units row present but no data — still an MSL file, and
        // `parse` gives a clearer error than a fallthrough parser would.
        let _ = header;
        true
    }

    /// Parse one field. `None` means "carry the previous value forward".
    fn parse_value(s: &str) -> Option<f64> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(v) = trimmed.parse::<f64>() {
            return Some(v);
        }
        match trimmed.to_uppercase().as_str() {
            "ON" | "YES" | "TRUE" | "ACTIVE" => return Some(1.0),
            "OFF" | "NO" | "FALSE" | "INACTIVE" => return Some(0.0),
            _ => {}
        }
        Self::parse_clock(trimmed)
    }

    /// Convert `HH:MM`, `HH:MM:SS` or `HH:MM:SS.mmm` to seconds since
    /// midnight so clock columns such as `GPS Time` plot as a channel.
    fn parse_clock(s: &str) -> Option<f64> {
        let mut parts = s.split(':');
        let hours: f64 = parts.next()?.trim().parse().ok()?;
        let minutes: f64 = parts.next()?.trim().parse().ok()?;
        let seconds: f64 = match parts.next() {
            Some(sec) => sec.trim().parse().ok()?,
            None => 0.0,
        };
        if parts.next().is_some() {
            return None;
        }
        if !(0.0..24.0).contains(&hours)
            || !(0.0..60.0).contains(&minutes)
            || !(0.0..61.0).contains(&seconds)
        {
            return None;
        }
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }
}

impl Parseable for Msl {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let file_contents = file_contents.trim_start_matches('\u{FEFF}');

        let (header_line, units_line, data_offset) =
            locate_header(file_contents).ok_or("Not a TunerStudio MSL log: no Time header row")?;

        let preamble: Vec<String> = file_contents[..data_offset]
            .lines()
            .map(|l| l.trim_end_matches('\r').to_string())
            .filter(|l| !l.is_empty())
            .take_while(|l| l.as_str() != header_line)
            .collect();

        let column_names: Vec<&str> = header_line.split('\t').collect();
        let units: Vec<&str> = units_line.split('\t').collect();

        if column_names.len() < 2 {
            return Err("Invalid MSL log: too few columns".into());
        }

        let scale = time_scale(units.first().copied().unwrap_or("sec"));

        // Column 0 is Time; every other column becomes a channel.
        let mut channels: Vec<Channel> = Vec::with_capacity(column_names.len() - 1);
        for (i, name) in column_names.iter().enumerate().skip(1) {
            let unit = units.get(i).copied().unwrap_or("");
            channels.push(Channel::Msl(MslChannel::new(name, unit)));
        }

        let estimated_rows = file_contents[data_offset..].len() / 128;
        let mut times: Vec<f64> = Vec::with_capacity(estimated_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_rows);

        // Last successfully parsed value per channel, for substitution.
        let mut last_values: Vec<f64> = vec![0.0; channels.len()];

        // Monotonic-time bookkeeping: `.msl` files can concatenate sessions.
        let mut time_base: Option<f64> = None;
        let mut time_offset = 0.0f64;
        let mut prev_raw = f64::NEG_INFINITY;

        for line in file_contents[data_offset..].lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || is_mark_line(line) {
                continue;
            }

            let mut fields = line.split('\t');
            let Some(raw_time) = fields.next().and_then(|f| f.trim().parse::<f64>().ok()) else {
                continue;
            };
            if !raw_time.is_finite() {
                continue;
            }

            if prev_raw.is_finite() && raw_time < prev_raw {
                // Session restart: shift so the axis never goes backwards.
                time_offset += prev_raw - raw_time;
            }
            prev_raw = raw_time;

            let scaled = (raw_time + time_offset) * scale;
            let base = *time_base.get_or_insert(scaled);
            times.push(scaled - base);

            let mut row: Vec<Value> = Vec::with_capacity(channels.len());
            for (i, field) in fields.take(channels.len()).enumerate() {
                let value = match Self::parse_value(field) {
                    Some(v) => {
                        last_values[i] = v;
                        v
                    }
                    None => last_values[i],
                };
                row.push(Value::Float(value));
            }

            // Short rows carry the last known value forward too.
            while row.len() < channels.len() {
                row.push(Value::Float(last_values[row.len()]));
            }

            data.push(row);
        }

        if data.is_empty() {
            return Err("No data rows found in MSL log".into());
        }

        tracing::info!(
            "Parsed MSL log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::Msl(MslMeta {
                header_lines: preamble,
                channel_count: channels.len(),
                data_points: data.len(),
            }),
            channels,
            times,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Time\tSecL\tRPM\tMAP\tGPS Time\n\
                          sec\ts\tRPM\tkPa\ttime\n\
                          0.0\t202.0\t965.0\t33.0\t21:51\n\
                          0.5\t202.0\t970.0\t34.0\t21:51\n\
                          1.0\t203.0\t980.0\t35.0\t21:52\n";

    #[test]
    fn detects_msl_header() {
        assert!(Msl::detect(SAMPLE));
        // UTF-8 BOM from Windows exporters must not break detection.
        assert!(Msl::detect(&format!("\u{FEFF}{}", SAMPLE)));
    }

    #[test]
    fn detects_msl_with_preamble() {
        let sample = format!(
            "\"MS3 Format 0.1\"\n\"Capture Date: Fri Aug 18\"\n{}",
            SAMPLE
        );
        assert!(Msl::detect(&sample));
        let log = Msl.parse(&sample).unwrap();
        assert_eq!(log.channels.len(), 4);
        match &log.meta {
            Meta::Msl(m) => assert_eq!(m.header_lines.len(), 2),
            other => panic!("expected MSL metadata, got {:?}", other),
        }
    }

    #[test]
    fn rejects_other_formats() {
        // Comma-delimited RomRaider-style export
        assert!(!Msl::detect("Time (msec),RPM,Load\n0,1000,0.5\n"));
        // ECUMaster tab export: no units row under TIME
        assert!(!Msl::detect("TIME\tRPM\tMAP\n0.0\t1000\t33\n"));
        // Haltech marker file
        assert!(!Msl::detect("%DataLog%\nDataLogVersion : 1.1\n"));
        // Header present but the second row is data, not units
        assert!(!Msl::detect("Time\tRPM\n0.0\t1000\n"));
        assert!(!Msl::detect(""));
    }

    #[test]
    fn parses_channels_and_units() {
        let log = Msl.parse(SAMPLE).unwrap();
        assert_eq!(log.channels.len(), 4);
        assert_eq!(log.channels[0].name(), "SecL");
        assert_eq!(log.channels[1].name(), "RPM");
        assert_eq!(log.channels[1].unit(), "RPM");
        assert_eq!(log.channels[2].unit(), "kPa");
        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);
        assert_eq!(log.data[0][1].as_f64(), 965.0);
        assert_eq!(log.data[2][2].as_f64(), 35.0);
    }

    #[test]
    fn converts_clock_columns_to_seconds_since_midnight() {
        let log = Msl.parse(SAMPLE).unwrap();
        // 21:51 -> 21*3600 + 51*60
        assert_eq!(log.data[0][3].as_f64(), 78660.0);
        assert_eq!(log.data[2][3].as_f64(), 78720.0);
    }

    #[test]
    fn parses_clock_variants_and_rejects_bad_ones() {
        assert_eq!(Msl::parse_clock("00:00"), Some(0.0));
        assert_eq!(Msl::parse_clock("01:02:03"), Some(3723.0));
        assert_eq!(Msl::parse_clock("01:02:03.5"), Some(3723.5));
        assert_eq!(Msl::parse_clock("24:00"), None);
        assert_eq!(Msl::parse_clock("12:61"), None);
        assert_eq!(Msl::parse_clock("1:2:3:4"), None);
        // A date is not a clock — it carries the last known value instead.
        assert_eq!(Msl::parse_value("18.8.2026"), None);
    }

    #[test]
    fn substitutes_last_known_value_for_blank_and_text_fields() {
        let sample = "Time\tRPM\tGPS Date\n\
                      sec\tRPM\tdate\n\
                      0.0\t1000.0\t\n\
                      0.5\t\t18.8.2026\n\
                      1.0\t1200.0\t18.8.2026\n";
        let log = Msl.parse(sample).unwrap();
        // Blank before any valid sample -> 0.0
        assert_eq!(log.data[0][1].as_f64(), 0.0);
        // Blank after a valid sample -> previous value
        assert_eq!(log.data[1][0].as_f64(), 1000.0);
        assert_eq!(log.data[2][0].as_f64(), 1200.0);
    }

    #[test]
    fn parses_boolean_fields() {
        let sample = "Time\tMIL\n\
                      sec\t\n\
                      0.0\tOFF\n\
                      0.5\tON\n";
        let log = Msl.parse(sample).unwrap();
        assert_eq!(log.data[0][0].as_f64(), 0.0);
        assert_eq!(log.data[1][0].as_f64(), 1.0);
    }

    #[test]
    fn skips_mark_lines() {
        let sample = "Time\tRPM\n\
                      sec\tRPM\n\
                      0.0\t1000.0\n\
                      MARK 001\n\
                      0.5\t1100.0\n";
        let log = Msl.parse(sample).unwrap();
        assert_eq!(log.data.len(), 2);
        assert_eq!(log.data[1][0].as_f64(), 1100.0);
    }

    #[test]
    fn scales_millisecond_time_column() {
        let sample = "Time\tRPM\n\
                      ms\tRPM\n\
                      0\t1000.0\n\
                      500\t1100.0\n\
                      1500\t1200.0\n";
        let log = Msl.parse(sample).unwrap();
        assert!((log.times[1] - 0.5).abs() < 1e-9);
        assert!((log.times[2] - 1.5).abs() < 1e-9);
    }

    #[test]
    fn rebases_time_to_zero() {
        let sample = "Time\tRPM\n\
                      sec\tRPM\n\
                      100.0\t1000.0\n\
                      100.5\t1100.0\n";
        let log = Msl.parse(sample).unwrap();
        assert_eq!(log.times[0], 0.0);
        assert!((log.times[1] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn forces_monotonic_time_across_a_session_restart() {
        let sample = "Time\tRPM\n\
                      sec\tRPM\n\
                      0.0\t1000.0\n\
                      1.0\t1100.0\n\
                      0.0\t1200.0\n\
                      1.0\t1300.0\n";
        let log = Msl.parse(sample).unwrap();
        for i in 1..log.times.len() {
            assert!(
                log.times[i] >= log.times[i - 1],
                "time went backwards at index {}",
                i
            );
        }
        assert!((log.times[2] - 1.0).abs() < 1e-9);
        assert!((log.times[3] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn errors_on_header_without_data() {
        let err = Msl.parse("Time\tRPM\nsec\tRPM\n").unwrap_err();
        assert!(err.to_string().contains("No data rows"));
    }

    #[test]
    fn errors_when_not_an_msl_file() {
        let err = Msl.parse("Time (msec),RPM\n0,1000\n").unwrap_err();
        assert!(err.to_string().contains("no Time header row"));
    }

    #[test]
    fn parses_real_realdash_export() {
        let contents = std::fs::read_to_string("exampleLogs/msl/realdash_speeduino_excerpt.msl")
            .expect("example file");
        assert!(Msl::detect(&contents));

        let log = Msl.parse(&contents).unwrap();

        // 30 columns minus the Time column
        assert_eq!(log.channels.len(), 29);
        assert_eq!(log.data.len(), 2000);
        assert_eq!(log.times.len(), log.data.len());

        assert_eq!(log.channels[1].name(), "RPM");
        assert_eq!(log.channels[1].unit(), "RPM");
        assert_eq!(log.channels[2].name(), "MAP");
        assert_eq!(log.channels[2].unit(), "kPa");
        assert_eq!(log.channels[4].name(), "SPK: Spark Advance");
        assert_eq!(log.channels[4].unit(), "deg");

        // GPS columns keep their exported names so the Track Map widget
        // can match them (see find_gps_channels in ui/widgets/track_map.rs).
        assert_eq!(log.channels[23].name(), "GPS Latitude");
        assert_eq!(log.channels[24].name(), "GPS Longitude");

        assert_eq!(log.times[0], 0.0);
        for i in 1..log.times.len() {
            assert!(
                log.times[i] >= log.times[i - 1],
                "time should be monotonic at index {}",
                i
            );
        }

        // First row values straight from the file.
        assert_eq!(log.data[0][1].as_f64(), 957.0); // RPM
        assert_eq!(log.data[0][2].as_f64(), 35.0); // MAP

        // The first seven rows have a blank GPS Date/Time; the eighth is the
        // first fix. Blank-before-first-sample reads 0.
        assert_eq!(log.data[0][26].as_f64(), 0.0); // GPS Date
        assert_eq!(log.data[7][27].as_f64(), 15.0 * 3600.0 + 17.0 * 60.0); // GPS Time 15:17
    }
}
