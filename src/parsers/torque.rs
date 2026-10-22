//! Torque Pro (Android OBD-II app) trackLog CSV parser.
//!
//! Torque Pro writes one `trackLog-<date>.csv` per logging session.
//! Format:
//! - Header row: `GPS Time, Device Time, Longitude, Latitude, GPS Speed
//!   (Meters/second), Horizontal Dilution of Precision, Altitude, Bearing,
//!   G(x), G(y), G(z), G(calibrated)`, then one column per logged PID named
//!   `Name(unit)`, e.g. `Engine RPM(rpm)`, `Air Fuel Ratio(Commanded)(:1)`
//! - Data rows: `GPS Time` is a Java `Date.toString()` with 1 s resolution
//!   (`Sat Jan 05 14:34:31 MST 2013`); `Device Time` is
//!   `05-Jan-2013 14:32:13.526`
//! - A PID with no reading for that row is written as `-`
//! - When logging restarts into the same file, Torque writes the header row
//!   again, and the PID set can differ from the previous section
//!
//! With "log all PIDs" enabled the header lists every PID Torque knows
//! (~220), most of which the car never answers. Columns that never hold a
//! number are dropped, so the channel list shows what the car reported.
//!
//! Reference: <https://torque-bhp.com/>

use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// How many lines `detect` looks through for the header row.
const DETECT_LINE_LIMIT: usize = 64;

/// Fixed leading columns Torque writes ahead of the PIDs, with the units
/// Torque uses for them. These cannot go through the `Name(unit)` split:
/// `G(x)` would become a channel named `G`, three times over.
const FIXED_COLUMNS: &[(&str, &str, &str)] = &[
    ("Longitude", "Longitude", "°"),
    ("Latitude", "Latitude", "°"),
    ("GPS Speed (Meters/second)", "GPS Speed", "m/s"),
    (
        "Horizontal Dilution of Precision",
        "Horizontal Dilution of Precision",
        "",
    ),
    ("Altitude", "Altitude", "m"),
    ("Bearing", "Bearing", "°"),
    ("G(x)", "G(x)", "m/s²"),
    ("G(y)", "G(y)", "m/s²"),
    ("G(z)", "G(z)", "m/s²"),
    ("G(calibrated)", "G(calibrated)", ""),
];

/// Torque Pro log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct TorqueMeta {
    /// `Device Time` of the first data row, as written
    pub start_time: String,
    /// Number of header rows found (one per logging session in the file)
    pub sessions: usize,
    /// Columns dropped because they never held a reading
    pub empty_columns_dropped: usize,
    /// Number of channels in the log
    pub channel_count: usize,
    /// Number of data points
    pub data_points: usize,
}

/// Torque Pro channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct TorqueChannel {
    /// Channel name with the unit removed (e.g. "Engine RPM")
    pub name: String,
    /// Unit from the trailing parentheses (e.g. "rpm")
    pub unit: String,
}

impl TorqueChannel {
    /// Split a header column into name and unit.
    ///
    /// PID columns end with the unit in parentheses. Only the last group is
    /// the unit: `Air Fuel Ratio(Commanded)(:1)` is `Air Fuel Ratio(Commanded)`
    /// in `:1`. A column without a trailing group has no unit.
    pub fn from_header(header: &str) -> Self {
        let header = header.trim();
        if let Some(&(_, name, unit)) = FIXED_COLUMNS.iter().find(|(h, _, _)| *h == header) {
            return Self {
                name: name.to_string(),
                unit: unit.to_string(),
            };
        }

        if header.ends_with(')')
            && let Some(open) = header.rfind('(')
            && open > 0
        {
            let name = header[..open].trim();
            let unit = header[open + 1..header.len() - 1].trim();
            if !name.is_empty() {
                return Self {
                    name: name.to_string(),
                    unit: canonical_unit(unit).to_string(),
                };
            }
        }

        Self {
            name: header.to_string(),
            unit: String::new(),
        }
    }
}

/// Map Torque's unit spellings to the ones `UnitPreferences` converts.
fn canonical_unit(unit: &str) -> &str {
    match unit {
        "l/100km" => "L/100km",
        "l" => "L",
        other => other,
    }
}

/// Which column, and which reading of it, gives the time axis.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TimeSource {
    /// `Device Time` as `05-Jan-2013 14:32:13.526`
    DeviceDateTime,
    /// `Device Time` as a clock only. Excel re-saves the column as
    /// `mm:ss.0`, and a non-English month name defeats the full parse.
    /// The clock wraps every `period` seconds.
    DeviceClock { period: f64 },
    /// `GPS Time` clock (`Sat Jan 05 14:34:31 MST 2013`), 1 s resolution.
    /// Only used when `Device Time` is unreadable.
    GpsClock,
}

/// Parse `[HH:]MM:SS[.fff]` into seconds, plus the period the clock wraps at.
fn parse_clock(token: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = token.split(':').collect();
    let (hours, minutes, seconds, period) = match parts.as_slice() {
        [h, m, s] => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?, *s, 86_400.0),
        [m, s] => (0, m.parse::<u32>().ok()?, *s, 3_600.0),
        _ => return None,
    };
    let seconds: f64 = seconds.parse().ok()?;
    if hours > 23 || minutes > 59 || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some((
        f64::from(hours) * 3_600.0 + f64::from(minutes) * 60.0 + seconds,
        period,
    ))
}

/// Turns a wrapping clock into a monotonic time axis.
#[derive(Default)]
struct ClockAxis {
    offset: f64,
    last: Option<f64>,
}

impl ClockAxis {
    /// A clock wraps each hour (`mm:ss`) or at midnight (`HH:MM:SS`). A step
    /// back of more than 1 s adds one period. The 1 s guard ignores jitter,
    /// the same as the Haltech midnight rollover.
    fn unwrap(&mut self, clock: f64, period: f64) -> f64 {
        if self.last.is_some_and(|last| clock < last - 1.0) {
            self.offset += period;
        }
        self.last = Some(clock);
        clock + self.offset
    }
}

/// Milliseconds since the unix epoch for a full `Device Time` value.
///
/// Kept as an integer until the first row is subtracted: epoch seconds as
/// an `f64` would turn a 977 ms step into 0.97699999 s.
fn parse_device_datetime(field: &str) -> Option<i64> {
    let dt = chrono::NaiveDateTime::parse_from_str(field.trim(), "%d-%b-%Y %H:%M:%S%.f").ok()?;
    Some(dt.and_utc().timestamp_millis())
}

/// The clock in the last whitespace-separated token of `Device Time`.
fn parse_device_clock(field: &str) -> Option<(f64, f64)> {
    parse_clock(field.split_whitespace().last()?)
}

/// The `HH:MM:SS` token of a Java `Date.toString()` value.
fn parse_gps_clock(field: &str) -> Option<f64> {
    field
        .split_whitespace()
        .find(|t| t.matches(':').count() == 2)
        .and_then(parse_clock)
        .map(|(seconds, _)| seconds)
}

/// Read one numeric field. `None` means "no reading": `-`, blank, text, or a
/// non-finite value.
fn parse_value(field: &str) -> Option<f64> {
    field.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Torque Pro trackLog parser
pub struct Torque;

impl Torque {
    /// A Torque header row starts with the `GPS Time, Device Time` pair.
    fn is_header(line: &str) -> bool {
        let mut fields = line.split(',').map(str::trim);
        fields
            .next()
            .is_some_and(|f| f.trim_start_matches('\u{FEFF}') == "GPS Time")
            && fields.next() == Some("Device Time")
    }

    /// Detect a Torque Pro trackLog.
    ///
    /// The header is normally the first line, but a file a user cut out of
    /// a longer log can start mid-session, so the first few lines are
    /// searched. The `GPS Time, Device Time` pair is specific enough that
    /// no other supported format starts a row with it.
    pub fn detect(contents: &str) -> bool {
        contents
            .lines()
            .take(DETECT_LINE_LIMIT)
            .any(|line| Self::is_header(line.trim()))
    }
}

/// Column layout of one header section.
struct Section {
    /// Union channel index for each column after the two time columns
    columns: Vec<usize>,
}

impl Parseable for Torque {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        // Channels are the union of every section's columns, in first-seen
        // order. Raw readings are kept as `Option` per row so a channel that
        // only appears in a later section can be added without reshaping.
        let mut channels: Vec<TorqueChannel> = Vec::new();
        let mut channel_index: HashMap<String, usize> = HashMap::new();
        let mut section: Option<Section> = None;
        let mut sessions = 0usize;

        let mut time_source: Option<TimeSource> = None;
        let mut start_time = String::new();
        let mut raw_times: Vec<f64> = Vec::new();
        let mut rows: Vec<Vec<Option<f64>>> = Vec::new();
        let mut rows_before_header = 0usize;
        let mut unreadable_time_rows = 0usize;
        let mut out_of_order_rows = 0usize;

        let mut clock_axis = ClockAxis::default();
        let mut datetime_base: Option<i64> = None;

        for line in file_contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.chars().all(|c| c == ',') {
                continue;
            }

            if Self::is_header(line) {
                sessions += 1;
                let mut columns = Vec::new();
                for header in line.split(',').skip(2) {
                    let header = header.trim();
                    if header.is_empty() {
                        // Trailing padding from a spreadsheet round-trip.
                        columns.push(usize::MAX);
                        continue;
                    }
                    let channel = TorqueChannel::from_header(header);
                    let idx = *channel_index.entry(header.to_string()).or_insert_with(|| {
                        channels.push(channel);
                        channels.len() - 1
                    });
                    columns.push(idx);
                }
                section = Some(Section { columns });
                continue;
            }

            let Some(section) = section.as_ref() else {
                // Data with no header above it cannot be mapped to channels.
                rows_before_header += 1;
                continue;
            };

            let mut fields = line.split(',');
            let gps_time = fields.next().unwrap_or("");
            let device_time = fields.next().unwrap_or("");

            let source = *time_source.get_or_insert_with(|| {
                if parse_device_datetime(device_time).is_some() {
                    TimeSource::DeviceDateTime
                } else if let Some((_, period)) = parse_device_clock(device_time) {
                    TimeSource::DeviceClock { period }
                } else {
                    TimeSource::GpsClock
                }
            });

            let time = match source {
                TimeSource::DeviceDateTime => parse_device_datetime(device_time)
                    .map(|ms| (ms - *datetime_base.get_or_insert(ms)) as f64 / 1000.0),
                TimeSource::DeviceClock { period } => parse_device_clock(device_time)
                    .filter(|(_, p)| *p == period)
                    .map(|(clock, _)| clock_axis.unwrap(clock, period)),
                TimeSource::GpsClock => {
                    parse_gps_clock(gps_time).map(|clock| clock_axis.unwrap(clock, 86_400.0))
                }
            };
            let Some(time) = time else {
                unreadable_time_rows += 1;
                continue;
            };

            // `times` must stay sorted: computed-channel time-shift lookups
            // binary-search it. Drop any row that steps backwards.
            if raw_times.last().is_some_and(|last| time < *last) {
                out_of_order_rows += 1;
                continue;
            }

            let mut row = vec![None; channels.len()];
            for (field, &idx) in fields.zip(&section.columns) {
                if idx != usize::MAX {
                    row[idx] = parse_value(field);
                }
            }

            if raw_times.is_empty() {
                start_time = device_time.trim().to_string();
            }
            raw_times.push(time);
            rows.push(row);
        }

        if sessions == 0 {
            return Err(
                "Invalid Torque Pro format: no 'GPS Time, Device Time' header row found".into(),
            );
        }
        if raw_times.is_empty() {
            return Err("Torque Pro log has a header row but no data rows. \
                        Torque recorded no samples in this logging session."
                .into());
        }
        if rows_before_header > 0 {
            tracing::warn!(
                "Torque Pro log had {} rows before the first header row; those rows were skipped",
                rows_before_header
            );
        }
        if unreadable_time_rows > 0 {
            tracing::warn!(
                "Torque Pro log had {} rows with an unreadable time ({:?}); those rows were skipped",
                unreadable_time_rows,
                time_source
            );
        }
        if out_of_order_rows > 0 {
            tracing::warn!(
                "Torque Pro log contained {} out-of-order timestamps; those rows were dropped",
                out_of_order_rows
            );
        }

        // Keep only the channels that hold at least one reading.
        let total_channels = channels.len();
        let keep: Vec<usize> = (0..total_channels)
            .filter(|&c| {
                rows.iter()
                    .any(|row| row.get(c).copied().flatten().is_some())
            })
            .collect();
        let empty_columns_dropped = total_channels - keep.len();

        // Torque polls PIDs round-robin and writes `-` until a PID answers,
        // so missing readings carry the last known value forward (0.0 before
        // the first one), the same as the Haltech, MSL and RaceChrono parsers.
        let mut last_values = vec![0.0; keep.len()];
        let data: Vec<Vec<Value>> = rows
            .iter()
            .map(|row| {
                keep.iter()
                    .zip(last_values.iter_mut())
                    .map(|(&c, last)| {
                        if let Some(v) = row.get(c).copied().flatten() {
                            *last = v;
                        }
                        Value::Float(*last)
                    })
                    .collect()
            })
            .collect();

        let base = raw_times[0];
        let times: Vec<f64> = raw_times.iter().map(|t| t - base).collect();

        // Unique display names: a channel name can repeat once the unit is
        // stripped (`Fuel Rate(L/m)` and a custom `Fuel Rate(l/hr)`).
        let mut seen: HashMap<String, usize> = HashMap::new();
        let channels: Vec<Channel> = keep
            .iter()
            .map(|&c| {
                let mut channel = channels[c].clone();
                let count = seen.entry(channel.name.clone()).or_insert(0);
                *count += 1;
                if *count > 1 && !channel.unit.is_empty() {
                    channel.name = format!("{} ({})", channel.name, channel.unit);
                } else if *count > 1 {
                    channel.name = format!("{} ({})", channel.name, count);
                }
                Channel::Torque(channel)
            })
            .collect();

        let channel_count = channels.len();
        let data_points = times.len();
        tracing::info!(
            "Torque Pro parse complete: {} channels ({} empty dropped), {} data points, {} session(s)",
            channel_count,
            empty_columns_dropped,
            data_points,
            sessions
        );

        Ok(Log {
            meta: Meta::Torque(TorqueMeta {
                start_time,
                sessions,
                empty_columns_dropped,
                channel_count,
                data_points,
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

    const HEADER: &str = "GPS Time, Device Time, Longitude, Latitude,GPS Speed (Meters/second), \
        Horizontal Dilution of Precision, Altitude, Bearing, G(x), G(y), G(z), G(calibrated),\
        Engine RPM(rpm),Air Fuel Ratio(Commanded)(:1),Engine Coolant Temperature(°C),\
        Hybrid Battery Charge (%)(%),Trip average Litres/100 KM(l/100km)";

    fn row(gps: &str, device: &str, rest: &str) -> String {
        format!(
            "{gps},{device},-104.86746,39.07588,0.0,2.8,2087.4,213.3,0.19,0.95,9.57,0.0001,{rest}"
        )
    }

    fn names(log: &Log) -> Vec<String> {
        log.channels.iter().map(|c| c.name()).collect()
    }

    fn column(log: &Log, name: &str) -> Vec<f64> {
        let idx = log.find_channel_index(name).expect(name);
        log.get_channel_data(idx)
    }

    #[test]
    fn detects_header_row() {
        assert!(Torque::detect(&format!("{HEADER}\r\n")));
        assert!(Torque::detect(&format!("\u{FEFF}{HEADER}\r\n")));
        assert!(!Torque::detect("Time,RPM\n0,1000\n"));
        assert!(!Torque::detect("GPS Time,RPM\n0,1000\n"));
    }

    #[test]
    fn detects_header_after_leading_rows() {
        let contents = format!(
            "{}\n{HEADER}\n",
            row(
                "Mon May 08 09:30:25 EDT 2017",
                "08-May-2017 09:31:54.506",
                "800,-,-,-,-"
            )
        );
        assert!(Torque::detect(&contents));
    }

    #[test]
    fn splits_name_and_unit() {
        let c = TorqueChannel::from_header("Engine RPM(rpm)");
        assert_eq!((c.name.as_str(), c.unit.as_str()), ("Engine RPM", "rpm"));
        let c = TorqueChannel::from_header("Air Fuel Ratio(Commanded)(:1)");
        assert_eq!(
            (c.name.as_str(), c.unit.as_str()),
            ("Air Fuel Ratio(Commanded)", ":1")
        );
        let c = TorqueChannel::from_header("Hybrid Battery Charge (%)(%)");
        assert_eq!(
            (c.name.as_str(), c.unit.as_str()),
            ("Hybrid Battery Charge (%)", "%")
        );
        let c = TorqueChannel::from_header("GPS Satellites");
        assert_eq!((c.name.as_str(), c.unit.as_str()), ("GPS Satellites", ""));
        let c = TorqueChannel::from_header("Trip average Litres/100 KM(l/100km)");
        assert_eq!(c.unit, "L/100km");
    }

    #[test]
    fn fixed_columns_keep_their_names() {
        let c = TorqueChannel::from_header(" G(x)");
        assert_eq!((c.name.as_str(), c.unit.as_str()), ("G(x)", "m/s²"));
        let c = TorqueChannel::from_header("GPS Speed (Meters/second)");
        assert_eq!((c.name.as_str(), c.unit.as_str()), ("GPS Speed", "m/s"));
        // Track Map matches these names exactly.
        assert_eq!(TorqueChannel::from_header(" Latitude").name, "Latitude");
        assert_eq!(TorqueChannel::from_header(" Longitude").name, "Longitude");
    }

    #[test]
    fn parses_device_time_and_drops_empty_columns() {
        let contents = [
            HEADER.to_string(),
            row(
                "Sat Jan 05 14:34:31 MST 2013",
                "05-Jan-2013 14:32:13.526",
                "-,-,80,-,-",
            ),
            row(
                "Sat Jan 05 14:34:32 MST 2013",
                "05-Jan-2013 14:32:14.026",
                "850.5,14.7,-,-,-",
            ),
            row(
                "Sat Jan 05 14:34:32 MST 2013",
                "05-Jan-2013 14:32:14.526",
                "900,-,81,-,∞",
            ),
        ]
        .join("\r\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(log.times.len(), 3);
        assert!((log.times[1] - 0.5).abs() < 1e-9);
        assert!((log.times[2] - 1.0).abs() < 1e-9);

        let names = names(&log);
        assert!(names.contains(&"Engine RPM".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("Hybrid")));
        assert!(!names.iter().any(|n| n.starts_with("Trip average")));

        // `-` carries the last reading forward, 0.0 before the first one.
        assert_eq!(column(&log, "Engine RPM"), vec![0.0, 850.5, 900.0]);
        assert_eq!(
            column(&log, "Air Fuel Ratio(Commanded)"),
            vec![0.0, 14.7, 14.7]
        );
        assert_eq!(
            column(&log, "Engine Coolant Temperature"),
            vec![80.0, 80.0, 81.0]
        );

        if let Meta::Torque(meta) = &log.meta {
            assert_eq!(meta.start_time, "05-Jan-2013 14:32:13.526");
            assert_eq!(meta.sessions, 1);
            assert_eq!(meta.empty_columns_dropped, 2);
        } else {
            panic!("expected Torque metadata");
        }
    }

    #[test]
    fn device_time_steps_are_exact_milliseconds() {
        let contents = [
            HEADER.to_string(),
            row("x", "22-Oct-2026 16:23:12.345", "800,-,-,-,-"),
            row("x", "22-Oct-2026 16:23:13.322", "810,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(log.times, vec![0.0, 0.977]);
    }

    #[test]
    fn header_only_file_is_an_error() {
        // Issue #97: the reporter's first file had a header row and nothing else.
        let err = Torque.parse(&format!("{HEADER}\r\n")).unwrap_err();
        assert!(err.to_string().contains("no data rows"), "{err}");
    }

    #[test]
    fn repeated_header_starts_a_new_section() {
        let second = "GPS Time, Device Time, Longitude, Latitude,GPS Speed (Meters/second), \
            Horizontal Dilution of Precision, Altitude, Bearing, G(x), G(y), G(z), G(calibrated),\
            Intake Air Temperature(°C),Engine RPM(rpm)";
        let contents = [
            HEADER.to_string(),
            row("x", "05-Jan-2013 14:32:13.000", "800,-,80,-,-"),
            second.to_string(),
            row("x", "05-Jan-2013 14:40:00.000", "25,1200"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(column(&log, "Engine RPM"), vec![800.0, 1200.0]);
        assert_eq!(column(&log, "Intake Air Temperature"), vec![0.0, 25.0]);
        // Not in the second section: carried forward.
        assert_eq!(column(&log, "Engine Coolant Temperature"), vec![80.0, 80.0]);
        assert!((log.times[1] - 467.0).abs() < 1e-9);
        if let Meta::Torque(meta) = &log.meta {
            assert_eq!(meta.sessions, 2);
        }
    }

    #[test]
    fn excel_minute_clock_wraps_each_hour() {
        // Excel re-saves Device Time as `mm:ss.0`.
        let contents = [
            HEADER.to_string(),
            row("x", "59:58.7", "800,-,-,-,-"),
            row("x", "59:59.7", "810,-,-,-,-"),
            row("x", "00:00.7", "820,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        let deltas: Vec<f64> = log.times.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(deltas.iter().all(|d| (d - 1.0).abs() < 1e-6), "{deltas:?}");
    }

    #[test]
    fn falls_back_to_gps_time() {
        let contents = [
            HEADER.to_string(),
            row("Sat Jan 05 23:59:59 MST 2013", "garbage", "800,-,-,-,-"),
            row("Sat Jan 06 00:00:01 MST 2013", "garbage", "810,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert!((log.times[1] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn drops_rows_that_step_backwards() {
        let contents = [
            HEADER.to_string(),
            row("x", "05-Jan-2013 14:32:13.000", "800,-,-,-,-"),
            row("x", "05-Jan-2013 14:32:12.000", "810,-,-,-,-"),
            row("x", "05-Jan-2013 14:32:14.000", "820,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(column(&log, "Engine RPM"), vec![800.0, 820.0]);
    }

    #[test]
    fn short_rows_keep_column_alignment() {
        let contents = [
            HEADER.to_string(),
            row("x", "05-Jan-2013 14:32:13.000", "800,14.7,80"),
            row("x", "05-Jan-2013 14:32:14.000", "810"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(column(&log, "Engine RPM"), vec![800.0, 810.0]);
        assert_eq!(column(&log, "Engine Coolant Temperature"), vec![80.0, 80.0]);
    }
}
