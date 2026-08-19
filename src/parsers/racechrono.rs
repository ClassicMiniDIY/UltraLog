//! RaceChrono CSV v3 session export parser.
//!
//! Parses session exports from the RaceChrono / RaceChrono Pro lap-timing
//! app (Android/iOS), which merges GPS, phone-sensor, and OBD/CAN channels
//! into one table.
//!
//! Format characteristics (CSV v3, "Format,3"):
//! - A banner line: `This file is created using RaceChrono Pro vX.Y.Z ...`
//! - A `Key,Value` metadata preamble (`Format`, `Session title`, `Track name`,
//!   `Created`, ...) terminated by a blank line. Values may be double-quoted.
//! - A comma-delimited header row whose first column is `timestamp`
//! - A units row (`unix time,,,s,m,...`) — temperatures use `.C` for °C
//! - A sources row tagging each column with its producer
//!   (`100: gps`, `calc`, `300: canbus`, `200: acc`, `201: gyro`, `202: magn`)
//! - Data rows keyed by unix-time `timestamp`. Channels update at different
//!   rates, so fields are blank between updates of their source.
//!
//! Column names repeat across sources (`speed` from GPS and calc,
//! `device_update_rate` from every sensor), so duplicated names are
//! disambiguated with the source label: `speed (gps)`, `speed (calc)`.
//! Unique names — including `latitude`/`longitude`, which the GPS Track Map
//! matches on — are kept exactly as exported.
//!
//! `elapsed_time` resets to zero at every fragment boundary (a fragment is
//! one recording segment; sessions paused and resumed at the track have
//! several), so the unix `timestamp` column is the only valid time base.
//! Times are re-based to seconds since the first record.
//!
//! Reference: <https://racechrono.com/support/file-formats>

use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Number of leading lines inspected when fingerprinting a file.
/// The banner and `Format` marker sit inside the metadata preamble.
const DETECT_SCAN_LINES: usize = 12;

/// The only CSV export version this parser understands.
const SUPPORTED_FORMAT_VERSION: u32 = 3;

/// RaceChrono session metadata, taken from the `Key,Value` preamble.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RaceChronoMeta {
    /// The full banner line, e.g.
    /// "This file is created using RaceChrono Pro v10.1.3 ( http://racechrono.com/ )."
    pub creator: String,
    /// `Format` value (3 for CSV v3)
    pub format_version: u32,
    /// `Session title` value
    pub session_title: String,
    /// `Session type` value (e.g. "Lap timing")
    pub session_type: String,
    /// `Track name` value
    pub track_name: String,
    /// `Driver name` value — often blank
    pub driver_name: String,
    /// `Created` value, date and time as exported (e.g. "04/08/2026,13:14")
    pub created: String,
    /// `Note` value
    pub note: String,
    /// Number of channels in the log
    pub channel_count: usize,
    /// Number of data points
    pub data_points: usize,
}

/// RaceChrono channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct RaceChronoChannel {
    /// Display name; source-suffixed only when the raw column name repeats
    /// (e.g. "speed (gps)")
    pub name: String,
    /// Display unit from the units row, normalized (`.C` -> "°C")
    pub unit: String,
    /// Raw source tag from the sources row (e.g. "100: gps", "calc")
    pub source: String,
}

/// RaceChrono CSV v3 session export parser
pub struct RaceChrono;

impl RaceChrono {
    /// Detect a RaceChrono CSV session export.
    ///
    /// Requires both the "created using RaceChrono" banner and a `Format,N`
    /// metadata line, so a stray CSV that merely mentions RaceChrono in a
    /// comment is not claimed. Any format version is detected — `parse`
    /// reports unsupported versions with a clear error instead of letting
    /// the file fall through to an unrelated parser.
    pub fn detect(contents: &str) -> bool {
        let contents = contents.trim_start_matches('\u{FEFF}');
        let mut lines = contents.lines().take(DETECT_SCAN_LINES);

        let Some(banner) = lines.find(|line| !line.trim().is_empty()) else {
            return false;
        };
        if !banner.contains("RaceChrono") {
            return false;
        }

        lines.any(|line| line.trim_start().starts_with("Format,"))
    }

    /// Strip one pair of surrounding double quotes from a metadata value.
    fn unquote(value: &str) -> &str {
        let value = value.trim();
        value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value)
    }

    /// Translate RaceChrono unit notation into the symbols the rest of the
    /// app uses. The CSV export writes `.C` where the app shows °C.
    fn normalize_unit(unit: &str) -> String {
        match unit {
            ".C" => "°C".to_string(),
            ".F" => "°F".to_string(),
            _ => unit.to_string(),
        }
    }

    /// Reduce a sources-row tag to its label: "100: gps" -> "gps",
    /// "calc" -> "calc". The numeric prefix is RaceChrono's device id.
    fn source_label(source: &str) -> &str {
        match source.split_once(':') {
            Some((id, label)) if id.trim().chars().all(|c| c.is_ascii_digit()) => label.trim(),
            _ => source.trim(),
        }
    }
}

impl Parseable for RaceChrono {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let file_contents = file_contents.trim_start_matches('\u{FEFF}');
        let mut meta = RaceChronoMeta::default();
        let mut lines = file_contents.lines().peekable();

        // Phase 1: metadata preamble, terminated by the first blank line.
        for line in lines.by_ref() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }

            let Some((key, value)) = trimmed.split_once(',') else {
                // The banner line has no comma.
                if meta.creator.is_empty() {
                    meta.creator = trimmed.to_string();
                }
                continue;
            };

            let value = Self::unquote(value);
            match key.trim() {
                "Format" => {
                    meta.format_version = value.parse().map_err(|_| {
                        format!("Invalid RaceChrono format: unparseable version '{}'", value)
                    })?
                }
                "Session title" => meta.session_title = value.to_string(),
                "Session type" => meta.session_type = value.to_string(),
                "Track name" => meta.track_name = value.to_string(),
                "Driver name" => meta.driver_name = value.to_string(),
                "Created" => meta.created = value.to_string(),
                "Note" => meta.note = value.to_string(),
                _ => {}
            }
        }

        if meta.format_version != SUPPORTED_FORMAT_VERSION {
            return Err(format!(
                "RaceChrono CSV format version {} is not supported — re-export the session as CSV v3",
                meta.format_version
            )
            .into());
        }

        // Phase 2: header row.
        let header = loop {
            let line = lines
                .next()
                .ok_or("Invalid RaceChrono format: no header row found")?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                break trimmed;
            }
        };

        let header_fields: Vec<&str> = header.split(',').map(str::trim).collect();
        let time_header = header_fields.first().copied().unwrap_or("");
        if !time_header.eq_ignore_ascii_case("timestamp") {
            return Err(format!(
                "Invalid RaceChrono format: expected 'timestamp' as the first column, found '{}'",
                time_header
            )
            .into());
        }
        if header_fields.len() < 2 {
            return Err("Invalid RaceChrono format: header row has no channel columns".into());
        }

        // Phase 3: units and sources rows. Both sit between the header and
        // the data and are recognized by a non-numeric first field, so a
        // file missing either row still parses.
        let mut units_row: Option<Vec<String>> = None;
        let mut sources_row: Option<Vec<String>> = None;
        while let Some(&next) = lines.peek() {
            let trimmed = next.trim();
            if trimmed.is_empty() {
                lines.next();
                continue;
            }
            let first_field = trimmed.split(',').next().unwrap_or("").trim();
            if first_field.parse::<f64>().is_ok() {
                break;
            }

            let row: Vec<String> = next.split(',').map(|f| f.trim().to_string()).collect();
            if units_row.is_none() {
                units_row = Some(row);
            } else if sources_row.is_none() {
                sources_row = Some(row);
            } else {
                return Err(
                    "Invalid RaceChrono format: too many non-data rows after the header"
                        .to_string()
                        .into(),
                );
            }
            lines.next();
        }

        // Phase 4: build channels for every column after `timestamp`.
        // Column names repeat across sources, so duplicated names get the
        // source label appended; unique names (latitude, longitude, rpm, ...)
        // stay exactly as exported so downstream name matching — the GPS
        // Track Map, field normalization — keeps working.
        let field_at = |row: &Option<Vec<String>>, idx: usize| -> String {
            row.as_ref()
                .and_then(|r| r.get(idx))
                .cloned()
                .unwrap_or_default()
        };

        let mut name_counts: HashMap<String, usize> = HashMap::new();
        for field in &header_fields[1..] {
            *name_counts.entry(field.to_lowercase()).or_default() += 1;
        }

        let mut final_names: HashMap<String, usize> = HashMap::new();
        let channels: Vec<Channel> = (1..header_fields.len())
            .map(|idx| {
                let base = header_fields[idx];
                let source = field_at(&sources_row, idx);
                let label = Self::source_label(&source);

                let mut name = if name_counts[&base.to_lowercase()] > 1 && !label.is_empty() {
                    format!("{} ({})", base, label)
                } else {
                    base.to_string()
                };

                // Residual collisions (same name AND same source) get an
                // ordinal so every channel stays individually addressable.
                let seen = final_names.entry(name.to_lowercase()).or_insert(0);
                *seen += 1;
                if *seen > 1 {
                    name = format!("{} #{}", name, seen);
                }

                Channel::RaceChrono(RaceChronoChannel {
                    name,
                    unit: Self::normalize_unit(&field_at(&units_row, idx)),
                    source,
                })
            })
            .collect();

        let channel_count = channels.len();

        // Phase 5: parse data rows. Channels update at different rates, so
        // blank fields carry the last known value for that column (0.0
        // before the first valid sample), matching the Haltech, ECUMaster,
        // and MHD parsers. Times are re-based to the first record because
        // the raw timestamps are unix time.
        let mut times: Vec<f64> = Vec::new();
        let mut data: Vec<Vec<Value>> = Vec::new();
        let mut last_values: Vec<Value> = vec![Value::Float(0.0); channel_count];
        let mut first_time: Option<f64> = None;
        let mut out_of_order_rows = 0usize;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < header_fields.len() {
                // Short row - not enough columns to fill every channel.
                continue;
            }

            let time: f64 = match parts[0].trim().parse() {
                Ok(t) => t,
                Err(_) => continue, // Skip rows without a usable timestamp
            };

            let base = *first_time.get_or_insert(time);
            let relative_time = time - base;

            // `times` must stay sorted: computed-channel time-shift lookups
            // binary-search it. Drop any row that steps backwards.
            if times.last().is_some_and(|last| relative_time < *last) {
                out_of_order_rows += 1;
                continue;
            }

            let row: Vec<Value> = (0..channel_count)
                .map(|idx| match parts[idx + 1].trim().parse::<f64>() {
                    Ok(v) => {
                        let value = Value::Float(v);
                        last_values[idx] = value;
                        value
                    }
                    Err(_) => last_values[idx],
                })
                .collect();

            times.push(relative_time);
            data.push(row);
        }

        if out_of_order_rows > 0 {
            tracing::warn!(
                "RaceChrono log contained {} out-of-order timestamps; those rows were dropped",
                out_of_order_rows
            );
        }

        meta.channel_count = channel_count;
        meta.data_points = times.len();

        tracing::info!(
            "RaceChrono parse complete: {} channels, {} data points",
            channel_count,
            meta.data_points
        );

        Ok(Log {
            meta: Meta::RaceChrono(meta),
            channels,
            times,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "This file is created using RaceChrono Pro v10.1.3 ( http://racechrono.com/ ).\n\
        Format,3\n\
        Session title,\"Test Session\"\n\
        Session type,Lap timing\n\
        Track name,\"Test Track\"\n\
        Driver name,\n\
        Created,04/08/2026,13:14\n\
        Note,\n\
        \n\
        timestamp,fragment_id,lap_number,elapsed_time,latitude,longitude,speed,speed,engine_oil_temp,rpm\n\
        unix time,,,s,deg,deg,m/s,m/s,.C,rpm\n\
        ,,,,100: gps,100: gps,100: gps,calc,300: canbus,300: canbus\n\
        1785849363.002,0,,0.002,44.831825,11.224415,0.0,,90.0,850.0\n\
        1785849363.052,0,,0.052,44.831826,11.224416,1.5,1.4,,900.0\n\
        1785849363.102,0,1,0.102,44.831827,11.224417,3.0,2.9,91.0,\n";

    #[test]
    fn detects_racechrono_export() {
        assert!(RaceChrono::detect(SAMPLE));
    }

    #[test]
    fn detects_racechrono_export_with_bom() {
        assert!(RaceChrono::detect(&format!("\u{FEFF}{}", SAMPLE)));
    }

    #[test]
    fn detects_non_pro_banner() {
        let contents = "This file is created using RaceChrono v10.1.3 ( http://racechrono.com/ ).\n\
            Format,3\n";
        assert!(RaceChrono::detect(contents));
    }

    #[test]
    fn rejects_other_csv_formats() {
        // RomRaider-style
        assert!(!RaceChrono::detect("Time,RPM,Load\n0,1000,50\n"));
        // Haltech
        assert!(!RaceChrono::detect("%DataLog%\nDataLogVersion : 1.1\n"));
        // A CSV that merely mentions RaceChrono without the Format marker
        assert!(!RaceChrono::detect(
            "Exported from RaceChrono for analysis\nTime,RPM\n0,1000\n"
        ));
        // Format marker without the banner
        assert!(!RaceChrono::detect("Format,3\ntimestamp,speed\n0,1\n"));
        assert!(!RaceChrono::detect(""));
    }

    #[test]
    fn rejects_unsupported_format_versions() {
        let v2 = "This file is created using RaceChrono Pro v10.1.3 ( http://racechrono.com/ ).\n\
            Format,2\n\
            \n\
            timestamp,speed\n";
        // Detection still claims the file so the user gets a clear error
        // instead of a fall-through to an unrelated parser.
        assert!(RaceChrono::detect(v2));
        let err = RaceChrono.parse(v2).unwrap_err().to_string();
        assert!(err.contains("version 2"), "unexpected error: {}", err);
        assert!(err.contains("CSV v3"), "unexpected error: {}", err);
    }

    #[test]
    fn parses_sample_metadata() {
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");

        match log.meta {
            Meta::RaceChrono(meta) => {
                assert!(meta.creator.contains("RaceChrono Pro v10.1.3"));
                assert_eq!(meta.format_version, 3);
                assert_eq!(meta.session_title, "Test Session");
                assert_eq!(meta.session_type, "Lap timing");
                assert_eq!(meta.track_name, "Test Track");
                assert_eq!(meta.driver_name, "");
                assert_eq!(meta.created, "04/08/2026,13:14");
                assert_eq!(meta.channel_count, 9);
                assert_eq!(meta.data_points, 3);
            }
            other => panic!("expected RaceChrono metadata, got {:?}", other),
        }
    }

    #[test]
    fn disambiguates_duplicate_names_with_source_labels() {
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");

        let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
        assert_eq!(
            names,
            vec![
                "fragment_id",
                "lap_number",
                "elapsed_time",
                "latitude",
                "longitude",
                "speed (gps)",
                "speed (calc)",
                "engine_oil_temp",
                "rpm",
            ]
        );
    }

    #[test]
    fn keeps_gps_channel_names_unsuffixed() {
        // The GPS Track Map matches "latitude"/"longitude" exactly; unique
        // column names must never grow a source suffix.
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");
        assert!(log.channels.iter().any(|c| c.name() == "latitude"));
        assert!(log.channels.iter().any(|c| c.name() == "longitude"));
    }

    #[test]
    fn normalizes_units() {
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");

        let unit_of = |name: &str| {
            log.channels
                .iter()
                .find(|c| c.name() == name)
                .unwrap_or_else(|| panic!("Missing channel '{}'", name))
                .unit()
                .to_string()
        };

        assert_eq!(unit_of("latitude"), "deg");
        assert_eq!(unit_of("speed (gps)"), "m/s");
        // RaceChrono writes '.C' for °C
        assert_eq!(unit_of("engine_oil_temp"), "°C");
        assert_eq!(unit_of("rpm"), "rpm");
        assert_eq!(unit_of("fragment_id"), "");
    }

    #[test]
    fn rebases_unix_timestamps_to_session_start() {
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");

        assert_eq!(log.times.len(), 3);
        assert!(log.times[0].abs() < 1e-9);
        assert!((log.times[1] - 0.05).abs() < 1e-6);
        assert!((log.times[2] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn carries_last_known_value_over_blank_fields() {
        let log = RaceChrono.parse(SAMPLE).expect("parse failed");

        let index_of = |name: &str| {
            log.channels
                .iter()
                .position(|c| c.name() == name)
                .unwrap_or_else(|| panic!("Missing channel '{}'", name))
        };

        // speed (calc) is blank on the first row -> 0.0 before first sample
        assert_eq!(log.data[0][index_of("speed (calc)")].as_f64(), 0.0);
        // engine_oil_temp is blank on the second row -> carries 90.0
        assert_eq!(log.data[1][index_of("engine_oil_temp")].as_f64(), 90.0);
        // rpm is blank on the third row -> carries 900.0
        assert_eq!(log.data[2][index_of("rpm")].as_f64(), 900.0);
        // lap_number is blank until the first lap -> 0.0, then 1.0
        assert_eq!(log.data[0][index_of("lap_number")].as_f64(), 0.0);
        assert_eq!(log.data[2][index_of("lap_number")].as_f64(), 1.0);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let crlf = SAMPLE.replace('\n', "\r\n");
        assert!(RaceChrono::detect(&crlf));
        let log = RaceChrono.parse(&crlf).expect("parse failed");
        assert_eq!(log.channels.len(), 9);
        assert_eq!(log.data.len(), 3);
        assert_eq!(log.data[2][8].as_f64(), 900.0);
    }

    #[test]
    fn drops_out_of_order_and_short_rows() {
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,speed,rpm\n\
            unix time,m/s,rpm\n\
            ,calc,300: canbus\n\
            100.0,1.0,1000\n\
            100.2,2.0,1100\n\
            100.1,9.9,9999\n\
            100.3,3.0\n\
            100.4,4.0,1400\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        assert_eq!(log.times.len(), 3);
        assert!(log.times.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(log.data[2][0].as_f64(), 4.0);
        assert_eq!(log.data[2][1].as_f64(), 1400.0);
    }

    #[test]
    fn parses_without_units_and_sources_rows() {
        // Defensive: recognize the data start by its numeric first field
        // even if the units/sources rows are absent.
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,speed,rpm\n\
            100.0,1.0,1000\n\
            100.1,2.0,1100\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        assert_eq!(log.channels.len(), 2);
        assert_eq!(log.channels[0].name(), "speed");
        assert_eq!(log.channels[0].unit(), "");
        assert_eq!(log.data.len(), 2);
    }

    #[test]
    fn source_label_extraction() {
        assert_eq!(RaceChrono::source_label("100: gps"), "gps");
        assert_eq!(RaceChrono::source_label("300: canbus"), "canbus");
        assert_eq!(RaceChrono::source_label("calc"), "calc");
        assert_eq!(RaceChrono::source_label(""), "");
        // A colon without a numeric device id is not a source prefix
        assert_eq!(RaceChrono::source_label("a: b"), "a: b");
    }

    #[test]
    fn rejects_header_without_timestamp_column() {
        let contents = "RaceChrono banner\nFormat,3\n\nTime,RPM\n0,1000\n";
        assert!(RaceChrono.parse(contents).is_err());
    }
}
