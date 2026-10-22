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
use std::collections::{HashMap, HashSet};
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
        // `Air Fuel Ratio(Measured)(:1)`
        ":1" => "AFR",
        // `Commanded Equivalence Ratio(lambda)`
        "lambda" => "λ",
        other => other,
    }
}

/// How many data rows `choose_time_source` looks at.
const TIME_SOURCE_SAMPLE_ROWS: usize = 50;

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

/// Pick the time source from the first data rows, as `(GPS Time, Device
/// Time)` pairs. One unreadable first row must not put the whole file on the
/// 1 s GPS clock.
fn choose_time_source(rows: &[(&str, &str)]) -> TimeSource {
    if rows
        .iter()
        .any(|(_, device)| parse_device_datetime(device).is_some())
    {
        return TimeSource::DeviceDateTime;
    }
    if let Some((_, period)) = rows
        .iter()
        .find_map(|(_, device)| parse_device_clock(device))
    {
        return TimeSource::DeviceClock { period };
    }
    TimeSource::GpsClock
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

/// Builds a sorted time axis from readings that can wrap or step back.
#[derive(Default)]
struct TimeAxis {
    offset: f64,
    last: Option<f64>,
}

impl TimeAxis {
    /// Place one reading on the axis, or return `None` to drop the row.
    ///
    /// - A clock (`period` is `Some`) that steps back by more than half its
    ///   period has wrapped (`mm:ss` each hour, `HH:MM:SS` at midnight), so
    ///   one period is added.
    /// - Any other step back of more than 1 s is a clock change on the phone
    ///   (a time-zone crossing, the DST fall-back). The offset absorbs it and
    ///   the axis continues from the last row, as the MSL parser does.
    /// - A step back of 1 s or less is jitter, and the row is dropped.
    fn place(&mut self, reading: f64, period: Option<f64>) -> Option<f64> {
        if let Some(last) = self.last
            && reading < last
        {
            let back = last - reading;
            match period {
                Some(period) if back > period / 2.0 => self.offset += period,
                _ if back > 1.0 => self.offset += back,
                _ => return None,
            }
        }
        self.last = Some(reading);
        Some(reading + self.offset)
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

/// A display name not yet in `used`: the plain name, then the name with its
/// unit, then a counter.
fn unique_name(name: &str, unit: &str, used: &HashSet<String>) -> String {
    if !used.contains(name) {
        return name.to_string();
    }
    if !unit.is_empty() {
        let with_unit = format!("{name} ({unit})");
        if !used.contains(&with_unit) {
            return with_unit;
        }
    }
    let mut n = 2;
    loop {
        let candidate = format!("{name} ({n})");
        if !used.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
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
    /// The header is normally the first line. The first few lines are
    /// searched so a file cut out of a longer log still detects when a
    /// restart header follows soon; the rows above that header cannot be
    /// mapped to channels and are skipped. The `GPS Time, Device Time` pair
    /// is specific enough that no other supported format starts a row with it.
    pub fn detect(contents: &str) -> bool {
        contents
            .lines()
            .take(DETECT_LINE_LIMIT)
            .any(|line| Self::is_header(line.trim()))
    }

    /// Lines that carry no data: blank, or only commas.
    fn is_blank(line: &str) -> bool {
        line.chars().all(|c| c == ',')
    }

    /// `(GPS Time, Device Time)` of the first data rows after the first header.
    fn sample_time_fields(contents: &str) -> Vec<(&str, &str)> {
        contents
            .lines()
            .map(str::trim)
            .skip_while(|line| !Self::is_header(line))
            .filter(|line| !Self::is_blank(line) && !Self::is_header(line))
            .take(TIME_SOURCE_SAMPLE_ROWS)
            .map(|line| {
                let mut fields = line.split(',');
                (fields.next().unwrap_or(""), fields.next().unwrap_or(""))
            })
            .collect()
    }
}

/// Column layout of one header section.
struct Section {
    /// Union channel index for each column after the two time columns
    columns: Vec<Option<usize>>,
}

impl Parseable for Torque {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let source = choose_time_source(&Self::sample_time_fields(file_contents));

        // Channels are the union of every section's columns, in first-seen
        // order. Rows are stored sparse, as `(channel, reading)` pairs: with
        // "log all PIDs" about 90 % of fields are `-`, and a channel that
        // first appears in a later section needs no reshaping.
        let mut channels: Vec<TorqueChannel> = Vec::new();
        let mut channel_index: HashMap<String, usize> = HashMap::new();
        let mut has_reading: Vec<bool> = Vec::new();
        let mut section: Option<Section> = None;
        let mut sessions = 0usize;

        let mut start_time = String::new();
        let mut raw_times: Vec<f64> = Vec::new();
        let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
        let mut data_rows = 0usize;
        let mut rows_before_header = 0usize;
        let mut unreadable_time_rows = 0usize;
        let mut out_of_order_rows = 0usize;

        let mut axis = TimeAxis::default();
        let mut datetime_base: Option<i64> = None;

        for line in file_contents.lines() {
            let line = line.trim();
            if Self::is_blank(line) {
                continue;
            }

            if Self::is_header(line) {
                sessions += 1;
                // A header that names one column twice keeps both columns:
                // the second occurrence gets its own key. Without that, the
                // later column's `-` would overwrite the earlier reading.
                let mut occurrences: HashMap<&str, usize> = HashMap::new();
                let mut columns = Vec::new();
                for header in line.split(',').skip(2) {
                    let header = header.trim();
                    if header.is_empty() {
                        // Trailing padding from a spreadsheet round-trip.
                        columns.push(None);
                        continue;
                    }
                    let occurrence = occurrences.entry(header).or_insert(0);
                    let key = match *occurrence {
                        0 => header.to_string(),
                        n => format!("{header}\u{0}{n}"),
                    };
                    *occurrence += 1;
                    let idx = *channel_index.entry(key).or_insert_with(|| {
                        channels.push(TorqueChannel::from_header(header));
                        has_reading.push(false);
                        channels.len() - 1
                    });
                    columns.push(Some(idx));
                }
                section = Some(Section { columns });
                continue;
            }

            data_rows += 1;
            let Some(section) = section.as_ref() else {
                // Data with no header above it cannot be mapped to channels.
                rows_before_header += 1;
                continue;
            };

            let mut fields = line.split(',');
            let gps_time = fields.next().unwrap_or("");
            let device_time = fields.next().unwrap_or("");

            let reading = match source {
                TimeSource::DeviceDateTime => parse_device_datetime(device_time).map(|ms| {
                    let ms = ms - *datetime_base.get_or_insert(ms);
                    (ms as f64 / 1000.0, None)
                }),
                TimeSource::DeviceClock { period } => parse_device_clock(device_time)
                    .filter(|(_, p)| *p == period)
                    .map(|(clock, _)| (clock, Some(period))),
                TimeSource::GpsClock => {
                    parse_gps_clock(gps_time).map(|clock| (clock, Some(86_400.0)))
                }
            };
            let Some((reading, period)) = reading else {
                unreadable_time_rows += 1;
                continue;
            };
            // `times` must stay sorted: computed-channel time-shift lookups
            // binary-search it.
            let Some(time) = axis.place(reading, period) else {
                out_of_order_rows += 1;
                continue;
            };

            let mut row = Vec::new();
            for (field, column) in fields.zip(&section.columns) {
                if let (Some(idx), Some(value)) = (*column, parse_value(field)) {
                    has_reading[idx] = true;
                    row.push((idx, value));
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
            if unreadable_time_rows > 0 {
                return Err(format!(
                    "Torque Pro log has {} data rows, but none has a readable Device Time or GPS Time.",
                    unreadable_time_rows
                )
                .into());
            }
            return Err("Torque Pro log has a header row but no data rows. \
                        Torque recorded no samples in this logging session."
                .into());
        }
        if rows_before_header > 0 {
            tracing::warn!(
                "Torque Pro log had {} of {} data rows before the first header row; those rows were skipped",
                rows_before_header,
                data_rows
            );
        }
        if unreadable_time_rows > 0 {
            tracing::warn!(
                "Torque Pro log had {} rows with an unreadable time ({:?}); those rows were skipped",
                unreadable_time_rows,
                source
            );
        }
        if out_of_order_rows > 0 {
            tracing::warn!(
                "Torque Pro log contained {} out-of-order timestamps; those rows were dropped",
                out_of_order_rows
            );
        }

        // Keep only the channels that hold at least one reading.
        let mut kept_position: Vec<Option<usize>> = vec![None; channels.len()];
        let mut kept: Vec<usize> = Vec::new();
        for (idx, _) in has_reading.iter().enumerate().filter(|(_, seen)| **seen) {
            kept_position[idx] = Some(kept.len());
            kept.push(idx);
        }
        let empty_columns_dropped = channels.len() - kept.len();

        // Torque polls PIDs round-robin and writes `-` until a PID answers,
        // so missing readings carry the last known value forward (0.0 before
        // the first one), the same as the Haltech, MSL and RaceChrono parsers.
        let mut last_values = vec![0.0; kept.len()];
        let data: Vec<Vec<Value>> = rows
            .iter()
            .map(|row| {
                for &(idx, value) in row {
                    if let Some(position) = kept_position[idx] {
                        last_values[position] = value;
                    }
                }
                last_values.iter().copied().map(Value::Float).collect()
            })
            .collect();

        let base = raw_times[0];
        let times: Vec<f64> = raw_times.iter().map(|t| t - base).collect();

        // Unique display names: a channel name can repeat once the unit is
        // stripped (`Fuel Rate(L/m)` and a custom `Fuel Rate(l/hr)`), or when
        // a header names one column twice.
        let mut used: HashSet<String> = HashSet::new();
        let channels: Vec<Channel> = kept
            .iter()
            .map(|&idx| {
                let mut channel = channels[idx].clone();
                channel.name = unique_name(&channel.name, &channel.unit, &used);
                used.insert(channel.name.clone());
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
            ("Air Fuel Ratio(Commanded)", "AFR")
        );
        let c = TorqueChannel::from_header("Commanded Equivalence Ratio(lambda)");
        assert_eq!(c.unit, "λ");
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

    #[test]
    fn duplicate_header_column_keeps_both_readings() {
        // Review finding: the second column's `-` overwrote the first
        // column's reading, and the channel was dropped as empty.
        let header = "GPS Time, Device Time, Longitude, Latitude,GPS Speed (Meters/second), \
            Horizontal Dilution of Precision, Altitude, Bearing, G(x), G(y), G(z), G(calibrated),\
            Engine RPM(rpm),Engine RPM(rpm)";
        let contents = [
            header.to_string(),
            row("x", "05-Jan-2013 14:32:13.000", "800,-"),
            row("x", "05-Jan-2013 14:32:14.000", "810,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(column(&log, "Engine RPM"), vec![800.0, 810.0]);
        assert!(log.find_channel_index("Engine RPM (rpm)").is_none());
    }

    #[test]
    fn device_clock_stepping_back_an_hour_keeps_the_rows() {
        // A time-zone crossing or the DST fall-back moves the phone clock back.
        let contents = [
            HEADER.to_string(),
            row("x", "05-Jan-2013 14:59:58.000", "800,-,-,-,-"),
            row("x", "05-Jan-2013 14:59:59.000", "810,-,-,-,-"),
            row("x", "05-Jan-2013 14:00:00.000", "820,-,-,-,-"),
            row("x", "05-Jan-2013 14:00:01.000", "830,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(column(&log, "Engine RPM"), vec![800.0, 810.0, 820.0, 830.0]);
        assert_eq!(log.times, vec![0.0, 1.0, 1.0, 2.0]);
    }

    #[test]
    fn gps_clock_zone_change_is_not_a_midnight_wrap() {
        let contents = [
            HEADER.to_string(),
            row("Sun Nov 03 01:59:58 MDT 2013", "garbage", "800,-,-,-,-"),
            row("Sun Nov 03 01:00:00 MST 2013", "garbage", "810,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert!(log.times[1] < 2.0, "{:?}", log.times);
    }

    #[test]
    fn unreadable_first_device_time_does_not_pick_the_gps_clock() {
        let contents = [
            HEADER.to_string(),
            row("-", "05-Ja", "800,-,-,-,-"),
            row("-", "05-Jan-2013 14:32:13.250", "810,-,-,-,-"),
            row("-", "05-Jan-2013 14:32:13.750", "820,-,-,-,-"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        assert_eq!(log.times, vec![0.0, 0.5]);
        assert_eq!(column(&log, "Engine RPM"), vec![810.0, 820.0]);
    }

    #[test]
    fn rows_without_any_readable_time_are_reported() {
        let contents = [HEADER.to_string(), row("-", "-", "800,-,-,-,-")].join("\n");
        let err = Torque.parse(&contents).unwrap_err().to_string();
        assert!(err.contains("none has a readable"), "{err}");
    }

    #[test]
    fn display_names_stay_unique() {
        let header = "GPS Time, Device Time, Longitude, Latitude,GPS Speed (Meters/second), \
            Horizontal Dilution of Precision, Altitude, Bearing, G(x), G(y), G(z), G(calibrated),\
            Fuel Rate(L/m),Fuel Rate(l/hr),Fuel Rate (l/hr)(x)";
        let contents = [
            header.to_string(),
            row("x", "05-Jan-2013 14:32:13.000", "1,2,3"),
        ]
        .join("\n");

        let log = Torque.parse(&contents).expect("parse");
        let names = names(&log);
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{names:?}");
        assert_eq!(column(&log, "Fuel Rate"), vec![1.0]);
        assert_eq!(column(&log, "Fuel Rate (l/hr)"), vec![2.0]);
    }
}
