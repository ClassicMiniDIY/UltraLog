//! Motorsport Electronics (ME221/ME442) ECU log file parser.
//!
//! Parses CSV log files exported from the ME Tuner software used by
//! Motorsport Electronics ECUs (ME221, ME442, etc.). These ECUs are
//! popular on turbo/supercharged MX-5s, Ford Focus ST150, Honda S2000,
//! Toyota Starlet, Mitsubishi Evo builds and similar platforms.
//!
//! Format characteristics:
//! - Comma-delimited CSV
//! - First column is "Time" (seconds since logging start)
//! - Channel names carry no inline units; units are inferred from the name
//! - Headers include ME-specific markers such as "Sync status",
//!   "Lost sync count", "MAP Raw", "TPS Raw", etc.
//!
//! Note: The RomRaider parser also detects files starting with "Time" but
//! treats that column as milliseconds, which would scale ME221 timestamps
//! 1000x smaller than reality. Detection for this parser therefore runs
//! before RomRaider detection in the dispatch chain.

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Distinctive column names used to fingerprint an ME Tuner CSV export.
/// Requiring multiple matches keeps detection specific enough to avoid
/// collisions with other "Time"-prefixed CSV formats (e.g. RomRaider).
const ME221_MARKER_COLUMNS: &[&str] = &[
    "Sync status",
    "Lost sync count",
    "Injector duty",
    "MAP Raw",
    "TPS Raw",
    "Ign. Adv. Base Table Output",
    "WBO2 Curr. AFR",
    "DBW Status",
];

/// Motorsport Electronics log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct MotorsportElectronicsMeta {
    pub channel_count: usize,
    pub data_points: usize,
}

/// Motorsport Electronics channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct MotorsportElectronicsChannel {
    /// Full column name from the header (e.g. "Ign. Adv. Base Table Output")
    pub name: String,
    /// Unit inferred from the name (ME Tuner does not embed units)
    pub unit: String,
}

impl MotorsportElectronicsChannel {
    /// Create a channel from a raw header column name.
    pub fn from_header(header: &str) -> Self {
        let name = header.trim().to_string();
        let unit = Self::infer_unit(&name);
        Self { name, unit }
    }

    /// Infer display unit from the channel name. ME Tuner exports strip
    /// units from the header, so we map known ME221 channel names back to
    /// conventional units.
    fn infer_unit(name: &str) -> String {
        let n = name.to_lowercase();

        // RPM
        if n == "rpm"
            || n.ends_with(" rpm")
            || n.contains("rpm error")
            || n.contains("target rpm")
            || n.contains("rpm limit")
            || n.contains("rpm clamp")
        {
            return "RPM".to_string();
        }

        // Temperatures
        if n.contains("coolant temperature")
            || n.contains("intake air temp")
            || n.contains("fuel temperature")
            || n.contains("internal iat")
        {
            return "°C".to_string();
        }

        // Pressures
        if n == "map"
            || n.starts_with("map ")
            || n.ends_with(" map")
            || n.contains("map delta")
            || n.contains("boost target")
            || n.contains("target map")
            || n.contains("boost open loop")
        {
            return "kPa".to_string();
        }

        // Voltages
        if n.contains("battery voltage") || n.contains("batt v") {
            return "V".to_string();
        }

        // Throttle and pedal positions
        if n == "tps"
            || n.starts_with("tps ")
            || n.ends_with(" tps")
            || n.contains("throttle plate pos")
            || n.contains("throttle pos.")
            || n.contains("pedal pos")
            || n.contains("calc. pedal pos")
            || n.contains("target tpps")
        {
            return "%".to_string();
        }

        // Duty cycle / percentage-ish signals
        if n.contains("duty")
            || n.contains("ve")
            || n.contains("trim")
            || n.contains("mod")
            || n.contains("correction")
            || n.contains("comp.")
            || n.contains("clt add")
            || n.contains("iat add")
        {
            // "ve" substring also matches unrelated words; fall through for
            // these before returning "%".
            if n.contains("valve") || n.contains("overrun") {
                // handled below
            } else {
                return "%".to_string();
            }
        }

        // Ignition advance / angles
        if n.contains("ign. adv")
            || n.contains("ignition adv")
            || n.contains("ignition timing")
            || n.contains("advance")
            || n.contains("angle")
            || n.contains("cam curr")
            || n.contains("cam target")
            || n.contains("spark scatter")
        {
            return "°".to_string();
        }

        // Dwell / pulse width / time-based
        if n.contains("dwell")
            || n.contains("pulse width")
            || n.contains("dead time")
            || n.contains("pulse")
            || n.contains("decay time")
        {
            return "ms".to_string();
        }

        // AFR / lambda
        if n.contains("afr") || n.contains("lambda") {
            return "AFR".to_string();
        }

        // O2 sensor raw/value
        if n == "o2 raw" || n == "o2 val" {
            return "".to_string();
        }

        // Speed
        if n.contains("veh. speed") || n.contains("vehicle speed") || n == "speed" {
            return "km/h".to_string();
        }

        // Airflow / fuel flow estimates
        if n.contains("airflow") {
            return "g/s".to_string();
        }
        if n.contains("fuelfow") || n.contains("fuel flow") {
            return "g/s".to_string();
        }

        // Ethanol / flex fuel
        if n.contains("ethanol content") {
            return "%".to_string();
        }

        // Gear (no unit)
        if n.contains("gear") {
            return "".to_string();
        }

        // Knock level
        if n.contains("knock") {
            return "".to_string();
        }

        // Default: no unit
        String::new()
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Motorsport Electronics (ME221/ME442) log parser
pub struct MotorsportElectronics;

impl MotorsportElectronics {
    /// Detect if the file contents look like an ME Tuner CSV export.
    ///
    /// Requires the first column to be exactly `Time` (not `Time (msec)` or
    /// `TIME;`) plus at least two ME221-specific channel names in the
    /// header. This keeps us from colliding with the RomRaider parser.
    pub fn detect(contents: &str) -> bool {
        let Some(first_line) = contents.lines().next() else {
            return false;
        };

        // ME Tuner always uses comma delimiter.
        if !first_line.contains(',') {
            return false;
        }

        // First column must be exactly "Time" (no unit suffix).
        let first_col = first_line.split(',').next().unwrap_or("").trim();
        if first_col != "Time" {
            return false;
        }

        // Require a couple of ME221-specific column names to fingerprint
        // the format and reject generic "Time,..." CSVs.
        let match_count = ME221_MARKER_COLUMNS
            .iter()
            .filter(|marker| first_line.contains(*marker))
            .count();

        match_count >= 2
    }
}

impl Parseable for MotorsportElectronics {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let line_count = file_contents.lines().count();
        let estimated_rows = line_count.saturating_sub(1);

        let mut channels: Vec<Channel> = Vec::with_capacity(64);
        let mut times: Vec<f64> = Vec::with_capacity(estimated_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_rows);

        let mut lines = file_contents.lines();
        let header = lines.next().ok_or("Empty file: no header found")?;

        let column_names: Vec<&str> = header.split(',').collect();
        if column_names.is_empty() {
            return Err("Invalid ME221 log: no columns found".into());
        }
        if !column_names[0].trim().eq_ignore_ascii_case("Time") {
            return Err("Invalid ME221 log: first column must be Time".into());
        }

        // Skip trailing "Dummy" column if present (ME Tuner sometimes emits one).
        let mut effective_count = column_names.len();
        if effective_count > 1
            && column_names[effective_count - 1]
                .trim()
                .eq_ignore_ascii_case("Dummy")
        {
            effective_count -= 1;
        }

        for name in column_names.iter().take(effective_count).skip(1) {
            channels.push(Channel::MotorsportElectronics(
                MotorsportElectronicsChannel::from_header(name),
            ));
        }

        // Track first timestamp so logs always start at t = 0.
        let mut first_time: Option<f64> = None;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.split(',');
            let Some(time_str) = parts.next() else {
                continue;
            };

            let Ok(time_val) = time_str.trim().parse::<f64>() else {
                continue;
            };

            let relative_time = match first_time {
                Some(first) => time_val - first,
                None => {
                    first_time = Some(time_val);
                    0.0
                }
            };
            times.push(relative_time);

            let mut row: Vec<Value> = Vec::with_capacity(channels.len());
            for part in parts.take(effective_count.saturating_sub(1)) {
                let value = part.trim().parse::<f64>().unwrap_or(0.0);
                row.push(Value::Float(value));
            }

            while row.len() < channels.len() {
                row.push(Value::Float(0.0));
            }

            data.push(row);
        }

        tracing::info!(
            "Parsed Motorsport Electronics log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::MotorsportElectronics(MotorsportElectronicsMeta {
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

    const SAMPLE_HEADER: &str = "Time, Marker, RPM, Sync status, Lost sync count, Injector duty, MAP Raw, MAP, TPS Raw, TPS, Battery Voltage, Coolant temperature, Intake Air Temp.";

    fn sample_log() -> String {
        format!(
            "{}\n0.06,0,1104.00,2.00,0.00,0.64,5908.00,27.05,5870.00,0.00,13.79,76.00,28.00\n\
             0.11,0,1105.00,2.00,0.00,0.64,5924.00,27.09,5881.00,0.00,13.86,76.00,28.00\n\
             0.16,0,1104.00,2.00,0.00,0.65,5931.00,27.14,5853.00,0.00,13.75,76.00,28.00\n",
            SAMPLE_HEADER
        )
    }

    #[test]
    fn detects_me221_header() {
        assert!(MotorsportElectronics::detect(&sample_log()));
    }

    #[test]
    fn rejects_romraider_header() {
        let romraider = "Time (msec),Engine Speed (rpm),Load (g/rev)\n0,1000,0.5\n";
        assert!(!MotorsportElectronics::detect(romraider));
    }

    #[test]
    fn rejects_ecumaster_header() {
        let ecumaster = "TIME;engine/rpm;sensors/tps1\n0.0;1000;50\n";
        assert!(!MotorsportElectronics::detect(ecumaster));
    }

    #[test]
    fn rejects_generic_time_csv() {
        // Starts with "Time," but no ME221 fingerprint columns.
        let generic = "Time,RPM,Load\n0,1000,50\n";
        assert!(!MotorsportElectronics::detect(generic));
    }

    #[test]
    fn parses_sample_log() {
        let parser = MotorsportElectronics;
        let log = parser.parse(&sample_log()).expect("should parse");

        assert_eq!(log.channels.len(), 12);
        assert_eq!(log.data.len(), 3);
        assert_eq!(log.times.len(), 3);

        // Time is in seconds, normalized to start at zero.
        assert!((log.times[0] - 0.0).abs() < 1e-9);
        assert!((log.times[1] - 0.05).abs() < 1e-3);
        assert!((log.times[2] - 0.10).abs() < 1e-3);

        // Channel data alignment: RPM is column index 1 (after Time, Marker)
        let rpm_idx = log
            .channels
            .iter()
            .position(|c| c.name() == "RPM")
            .expect("RPM channel present");
        assert!((log.data[0][rpm_idx].as_f64() - 1104.0).abs() < 1e-9);
    }

    #[test]
    fn unit_inference() {
        assert_eq!(MotorsportElectronicsChannel::from_header("RPM").unit, "RPM");
        assert_eq!(MotorsportElectronicsChannel::from_header("MAP").unit, "kPa");
        assert_eq!(MotorsportElectronicsChannel::from_header("TPS").unit, "%");
        assert_eq!(
            MotorsportElectronicsChannel::from_header("Battery Voltage").unit,
            "V"
        );
        assert_eq!(
            MotorsportElectronicsChannel::from_header("Coolant temperature").unit,
            "°C"
        );
        assert_eq!(
            MotorsportElectronicsChannel::from_header("Injector duty").unit,
            "%"
        );
        assert_eq!(
            MotorsportElectronicsChannel::from_header("Ignition Dwell").unit,
            "ms"
        );
        assert_eq!(
            MotorsportElectronicsChannel::from_header("Ign. Adv. Base Table Output").unit,
            "°"
        );
    }

    #[test]
    fn strips_trailing_dummy_column() {
        let header = format!("{}, Dummy", SAMPLE_HEADER);
        let body =
            "0.06,0,1104.00,2.00,0.00,0.64,5908.00,27.05,5870.00,0.00,13.79,76.00,28.00,0.00\n";
        let contents = format!("{}\n{}", header, body);

        let log = MotorsportElectronics.parse(&contents).expect("parses");
        // Without the Dummy column, 12 real channels remain.
        assert_eq!(log.channels.len(), 12);
        assert_eq!(log.data[0].len(), 12);
    }

    #[test]
    fn empty_file_fails_gracefully() {
        assert!(MotorsportElectronics.parse("").is_err());
    }

    #[test]
    fn header_only_yields_no_rows() {
        let log = MotorsportElectronics
            .parse(SAMPLE_HEADER)
            .expect("header only parse");
        assert_eq!(log.data.len(), 0);
        assert_eq!(log.times.len(), 0);
    }
}
