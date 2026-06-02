//! Woolich Racing Tuned (WRT) ECU log file parser.
//!
//! Parses CSV log files exported from Woolich Racing Tuned, the flash-tuning
//! software used on Yamaha, Kawasaki, Suzuki, Honda and BMW sportbike ECUs.
//! These logs come from the bike's stock ECU read back through the WRT
//! datalogger rather than a standalone aftermarket ECU.
//!
//! Format characteristics:
//! - Comma-delimited CSV with a trailing comma (so each row carries an empty
//!   final field that must be ignored).
//! - First column is `Log Time`, formatted `HH:MM:SS.mmm` (wall-clock elapsed),
//!   which we normalize to relative seconds starting at zero.
//! - Channel names carry no inline units; units are inferred from the name.
//! - Boolean channels such as `Clutch In` are exported as `True`/`False` and
//!   are mapped to 1.0 / 0.0 so they can be charted.
//!
//! Example:
//! ```text
//! Log Time,RPM,TPS,IAP,AFR,Gear,Clutch In,Coolant Temp,IAT,
//! 00:00:00.040,0,0.00,100.00,0.0,0,False,24.0,25.0,
//! ```

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Woolich Racing Tuned log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoolichMeta {
    pub channel_count: usize,
    pub data_points: usize,
}

/// Woolich Racing Tuned channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoolichChannel {
    /// Full column name from the header (e.g. "Coolant Temp")
    pub name: String,
    /// Unit inferred from the name (WRT does not embed units)
    pub unit: String,
}

impl WoolichChannel {
    /// Create a channel from a raw header column name.
    pub fn from_header(header: &str) -> Self {
        let name = header.trim().to_string();
        let unit = Self::infer_unit(&name);
        Self { name, unit }
    }

    /// Infer a display unit from the channel name. WRT exports strip units
    /// from the header, so map known Woolich channel names back to
    /// conventional units.
    fn infer_unit(name: &str) -> String {
        let n = name.to_lowercase();

        if n == "rpm" || n.ends_with(" rpm") {
            return "RPM".to_string();
        }

        // Throttle / accelerator position
        if n == "tps" || n.contains("throttle") || n.contains("aps") {
            return "%".to_string();
        }

        // Intake air pressure / manifold pressure (kPa on WRT logs)
        if n == "iap" || n == "map" || n.contains("pressure") {
            return "kPa".to_string();
        }

        // Air/fuel ratio and lambda
        if n.contains("afr") || n.contains("lambda") {
            return "AFR".to_string();
        }

        // Temperatures (coolant, intake air, engine, oil)
        if n.contains("temp") || n == "iat" || n == "ect" {
            return "°C".to_string();
        }

        // Speed
        if n.contains("speed") {
            return "km/h".to_string();
        }

        // Voltages
        if n.contains("voltage") || n.contains("volt") {
            return "V".to_string();
        }

        // Ignition advance / timing angles
        if n.contains("advance") || n.contains("ignition") || n.contains("timing") {
            return "°".to_string();
        }

        // Gear, clutch and other unitless / boolean channels
        String::new()
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Parse a `HH:MM:SS.mmm` Woolich log timestamp into seconds.
fn parse_log_time(value: &str) -> Option<f64> {
    let value = value.trim();
    let mut parts = value.split(':');
    let hours: f64 = parts.next()?.trim().parse().ok()?;
    let minutes: f64 = parts.next()?.trim().parse().ok()?;
    let seconds: f64 = parts.next()?.trim().parse().ok()?;
    // A well-formed timestamp has exactly three components.
    if parts.next().is_some() {
        return None;
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Parse a single Woolich field into an f64, mapping booleans to 1.0 / 0.0.
fn parse_field(value: &str) -> f64 {
    let v = value.trim();
    if v.eq_ignore_ascii_case("true") {
        1.0
    } else if v.eq_ignore_ascii_case("false") {
        0.0
    } else {
        v.parse::<f64>().unwrap_or(0.0)
    }
}

/// Woolich Racing Tuned log parser
pub struct Woolich;

impl Woolich {
    /// Detect if the file contents look like a Woolich Racing Tuned CSV export.
    ///
    /// Requires the first column to be exactly `Log Time` and the first data
    /// row to begin with an `HH:MM:SS.mmm` timestamp. No other supported
    /// format uses a `Log Time` header, so this is specific enough on its own.
    pub fn detect(contents: &str) -> bool {
        let mut lines = contents.lines();

        let Some(header) = lines.next() else {
            return false;
        };
        if !header.contains(',') {
            return false;
        }
        let first_col = header.split(',').next().unwrap_or("").trim();
        if !first_col.eq_ignore_ascii_case("Log Time") {
            return false;
        }

        // Confirm the first non-empty data row starts with a valid timestamp.
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let first_field = line.split(',').next().unwrap_or("");
            return parse_log_time(first_field).is_some();
        }

        // Header matched but there were no data rows; accept on the header alone.
        true
    }
}

impl Parseable for Woolich {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let line_count = file_contents.lines().count();
        let estimated_rows = line_count.saturating_sub(1);

        let mut lines = file_contents.lines();
        let header = lines.next().ok_or("Empty file: no header found")?;

        let column_names: Vec<&str> = header.split(',').collect();
        if column_names.is_empty() {
            return Err("Invalid Woolich log: no columns found".into());
        }
        if !column_names[0].trim().eq_ignore_ascii_case("Log Time") {
            return Err("Invalid Woolich log: first column must be Log Time".into());
        }

        // The header carries a trailing comma, so the final field is empty.
        // Build channels from every non-empty column after "Log Time".
        let mut channels: Vec<Channel> = Vec::with_capacity(column_names.len());
        // Column index (into the raw row split) for each channel.
        let mut channel_columns: Vec<usize> = Vec::with_capacity(column_names.len());
        for (idx, name) in column_names.iter().enumerate().skip(1) {
            if name.trim().is_empty() {
                continue;
            }
            channels.push(Channel::Woolich(WoolichChannel::from_header(name)));
            channel_columns.push(idx);
        }

        if channels.is_empty() {
            return Err("Invalid Woolich log: no data channels found".into());
        }

        let mut times: Vec<f64> = Vec::with_capacity(estimated_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_rows);
        let mut first_time: Option<f64> = None;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            let Some(time_val) = fields.first().and_then(|f| parse_log_time(f)) else {
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
            for &col in &channel_columns {
                let value = fields.get(col).map(|f| parse_field(f)).unwrap_or(0.0);
                row.push(Value::Float(value));
            }
            data.push(row);
        }

        tracing::info!(
            "Parsed Woolich log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::Woolich(WoolichMeta {
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

    const SAMPLE: &str = "Log Time,RPM,TPS,IAP,AFR,Gear,Clutch In,Coolant Temp,IAT,\n\
        00:00:00.040,0,0.00,100.00,0.0,0,False,24.0,25.0,\n\
        00:00:00.100,1224,12.50,59.00,13.2,0,True,92.0,26.0,\n\
        00:00:00.160,3065,7.84,91.00,14.0,2,False,27.0,26.0,\n";

    #[test]
    fn detects_woolich_header() {
        assert!(Woolich::detect(SAMPLE));
    }

    #[test]
    fn rejects_other_formats() {
        // RomRaider-style leading "Time" column
        assert!(!Woolich::detect("Time (msec),RPM,Load\n0,1000,0.5\n"));
        // ECUMaster
        assert!(!Woolich::detect("TIME;engine/rpm\n0.0;1000\n"));
        // ME221 generic Time CSV
        assert!(!Woolich::detect("Time,RPM,Load\n0,1000,50\n"));
        // Locomotive
        assert!(!Woolich::detect("TimeStamp: Sat Nov 15 19:00:03 2025\n"));
    }

    #[test]
    fn rejects_log_time_header_without_timestamp_rows() {
        // Header matches but data row is not a HH:MM:SS timestamp.
        assert!(!Woolich::detect("Log Time,RPM\n1.23,1000\n"));
    }

    #[test]
    fn parses_sample_log() {
        let log = Woolich.parse(SAMPLE).expect("should parse");

        // 8 channels (trailing empty column dropped, Log Time excluded).
        assert_eq!(log.channels.len(), 8);
        assert_eq!(log.data.len(), 3);
        assert_eq!(log.times.len(), 3);

        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[5].name(), "Clutch In");
        assert_eq!(log.channels[6].name(), "Coolant Temp");

        // Times are normalized to start at zero (60 ms steps).
        assert!((log.times[0] - 0.0).abs() < 1e-9);
        assert!((log.times[1] - 0.06).abs() < 1e-3);
        assert!((log.times[2] - 0.12).abs() < 1e-3);

        // RPM column alignment.
        assert!((log.data[1][0].as_f64() - 1224.0).abs() < 1e-9);
        assert!((log.data[2][0].as_f64() - 3065.0).abs() < 1e-9);

        // Boolean Clutch In maps to 1.0 / 0.0.
        assert!((log.data[0][5].as_f64() - 0.0).abs() < 1e-9);
        assert!((log.data[1][5].as_f64() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_log_time_formats() {
        assert!((parse_log_time("00:00:00.040").unwrap() - 0.040).abs() < 1e-9);
        assert!((parse_log_time("00:01:30.500").unwrap() - 90.5).abs() < 1e-9);
        assert!((parse_log_time("01:00:00.000").unwrap() - 3600.0).abs() < 1e-9);
        assert!(parse_log_time("not a time").is_none());
        assert!(parse_log_time("1.23").is_none());
        assert!(parse_log_time("00:00").is_none());
    }

    #[test]
    fn infers_units() {
        assert_eq!(WoolichChannel::from_header("RPM").unit, "RPM");
        assert_eq!(WoolichChannel::from_header("TPS").unit, "%");
        assert_eq!(WoolichChannel::from_header("IAP").unit, "kPa");
        assert_eq!(WoolichChannel::from_header("AFR").unit, "AFR");
        assert_eq!(WoolichChannel::from_header("Coolant Temp").unit, "°C");
        assert_eq!(WoolichChannel::from_header("IAT").unit, "°C");
        assert_eq!(WoolichChannel::from_header("Gear").unit, "");
        assert_eq!(WoolichChannel::from_header("Clutch In").unit, "");
    }

    #[test]
    fn empty_file_fails_gracefully() {
        assert!(Woolich.parse("").is_err());
    }

    #[test]
    fn header_only_yields_no_rows() {
        let log = Woolich
            .parse("Log Time,RPM,TPS,IAP,AFR,Gear,Clutch In,Coolant Temp,IAT,")
            .expect("header only parse");
        assert_eq!(log.channels.len(), 8);
        assert_eq!(log.data.len(), 0);
        assert_eq!(log.times.len(), 0);
    }
}
