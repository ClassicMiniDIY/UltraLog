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
//! When the short labels themselves collide (two location devices both
//! labeled `gps`), the first occurrence keeps its bare name — `latitude` /
//! `longitude` must survive exactly for the GPS Track Map — and later ones
//! carry the full source tag: `latitude (101: gps)`. Unique names are kept
//! exactly as exported.
//!
//! `elapsed_time` resets to zero at every fragment boundary (a fragment is
//! one recording segment; sessions paused and resumed at the track have
//! several), so the unix `timestamp` column is the only valid time base.
//! Times are re-based to seconds since the first record.
//!
//! The parser also survives a spreadsheet round-trip: Excel/Numbers/Sheets
//! pad every preamble line with trailing commas out to the widest row, so
//! metadata values are stripped of that padding before use.
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

/// Upper bound on continuation lines joined into one quoted metadata value
/// (e.g. a multi-line `Note`). Stops an unterminated quote from swallowing
/// the rest of the file.
const MAX_QUOTED_VALUE_LINES: usize = 100;

/// RaceChrono session metadata, taken from the `Key,Value` preamble.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RaceChronoMeta {
    /// The full banner line, e.g.
    /// `"This file is created using RaceChrono Pro v10.1.3 ( http://racechrono.com/ )."`
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

    /// Trim whitespace and the trailing-comma padding a spreadsheet
    /// round-trip adds to preamble lines (`Format,3,,,,` -> `3`).
    fn strip_padding(value: &str) -> &str {
        value.trim().trim_end_matches(',').trim_end()
    }

    /// Whether a line is blank for structural purposes. A spreadsheet
    /// round-trip pads the preamble-terminating blank line out to `,,,,,`,
    /// which must still read as blank.
    fn is_blank_line(line: &str) -> bool {
        line.chars().all(|c| c == ',' || c.is_whitespace())
    }

    /// Strip one pair of surrounding double quotes from a metadata value.
    fn unquote(value: &str) -> &str {
        let value = value.trim();
        value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value)
    }

    /// Whether a padded metadata value opens a quote it does not close, so
    /// the value continues on following lines (a multi-line `Note`).
    fn is_unterminated_quote(value: &str) -> bool {
        value.starts_with('"') && (value.len() == 1 || !value.ends_with('"'))
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

    /// Whether a field looks like a numbered device tag from the sources
    /// row, e.g. "100: gps" or "300: canbus".
    fn looks_like_device_tag(field: &str) -> bool {
        match field.split_once(':') {
            Some((id, label)) => {
                let id = id.trim();
                !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) && !label.trim().is_empty()
            }
            None => false,
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

    /// Build unique display names for columns `1..` of the header.
    ///
    /// - unique column names stay exactly as exported
    /// - duplicated names get the source label appended when the labels
    ///   differ: `speed (gps)`, `speed (calc)`
    /// - when the labels collide too (two location devices both labeled
    ///   `gps`), the first occurrence keeps the bare name — the GPS Track
    ///   Map matches `latitude`/`longitude` exactly — and later ones carry
    ///   the full source tag: `latitude (101: gps)`
    /// - anything still colliding after that gets an ordinal suffix
    fn build_channel_names(header_fields: &[&str], sources: &[String]) -> Vec<String> {
        let column_count = header_fields.len();

        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, field) in header_fields.iter().enumerate().skip(1) {
            groups.entry(field.to_lowercase()).or_default().push(idx);
        }

        let mut names: Vec<String> = vec![String::new(); column_count];
        for group in groups.values() {
            if group.len() == 1 {
                let idx = group[0];
                names[idx] = header_fields[idx].to_string();
                continue;
            }

            let labels: Vec<&str> = group
                .iter()
                .map(|&idx| Self::source_label(&sources[idx]))
                .collect();
            let mut seen_labels: HashMap<String, ()> = HashMap::new();
            let labels_distinct = labels
                .iter()
                .all(|l| !l.is_empty() && seen_labels.insert(l.to_lowercase(), ()).is_none());

            if labels_distinct {
                for (pos, &idx) in group.iter().enumerate() {
                    names[idx] = format!("{} ({})", header_fields[idx], labels[pos]);
                }
            } else {
                names[group[0]] = header_fields[group[0]].to_string();
                for &idx in &group[1..] {
                    let tag = sources[idx].trim();
                    names[idx] = if tag.is_empty() {
                        header_fields[idx].to_string()
                    } else {
                        format!("{} ({})", header_fields[idx], tag)
                    };
                }
            }
        }

        // Residual collisions get an ordinal so every channel stays
        // individually addressable.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for name in names.iter_mut().skip(1) {
            let count = seen.entry(name.to_lowercase()).or_insert(0);
            *count += 1;
            if *count > 1 {
                *name = format!("{} #{}", name, count);
            }
        }

        names
    }
}

impl Parseable for RaceChrono {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let file_contents = file_contents.trim_start_matches('\u{FEFF}');
        let mut meta = RaceChronoMeta::default();
        let mut lines = file_contents.lines().peekable();

        // Phase 1: metadata preamble, terminated by the first blank line
        // (possibly comma-padded by a spreadsheet round-trip).
        let mut saw_format_line = false;
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if Self::is_blank_line(trimmed) {
                break;
            }

            let Some((key, value)) = trimmed.split_once(',') else {
                // The banner line has no comma.
                if meta.creator.is_empty() {
                    meta.creator = trimmed.to_string();
                }
                continue;
            };

            let mut value = Self::strip_padding(value).to_string();

            // A quoted value can span lines (a multi-line Note). Join the
            // continuation lines until the quote closes, bounded so an
            // unterminated quote cannot swallow the rest of the file.
            if Self::is_unterminated_quote(&value) {
                for continuation in lines.by_ref().take(MAX_QUOTED_VALUE_LINES) {
                    value.push('\n');
                    value.push_str(Self::strip_padding(continuation));
                    if value.ends_with('"') {
                        break;
                    }
                }
            }

            let value = Self::unquote(&value);
            match key.trim() {
                "Format" => {
                    saw_format_line = true;
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

        // A missing Format line gets its own message — reporting it as
        // "version 0 is not supported" would mislead on corrupted or
        // truncated exports.
        if !saw_format_line {
            return Err(
                "Invalid RaceChrono format: no 'Format' metadata line found in the preamble".into(),
            );
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
            if !Self::is_blank_line(trimmed) {
                break trimmed;
            }
        };

        let mut header_fields: Vec<&str> = header.split(',').map(str::trim).collect();
        // Drop the trailing empty columns a spreadsheet round-trip appends
        // so padding never becomes ghost channels.
        while header_fields.len() > 1 && header_fields.last().is_some_and(|f| f.is_empty()) {
            header_fields.pop();
        }
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
        // the data (recognized by a non-numeric first field) and a file
        // missing either one still parses. The sources row is identified by
        // its numbered device tags ("100: gps"), so a lone sources row is
        // not mistaken for units; two untagged rows keep the exported
        // units-then-sources order. Any further annotation row is left for
        // the data loop, which skips rows without a usable timestamp.
        let mut units_row: Option<Vec<String>> = None;
        let mut sources_row: Option<Vec<String>> = None;
        while let Some(&next) = lines.peek() {
            let trimmed = next.trim();
            if Self::is_blank_line(trimmed) {
                lines.next();
                continue;
            }
            let first_field = trimmed.split(',').next().unwrap_or("").trim();
            if first_field.parse::<f64>().is_ok() {
                break;
            }

            let row: Vec<String> = next.split(',').map(|f| f.trim().to_string()).collect();
            let has_device_tags = row
                .iter()
                .skip(1)
                .any(|field| Self::looks_like_device_tag(field));
            match (units_row.is_some(), sources_row.is_some()) {
                (false, false) => {
                    if has_device_tags {
                        sources_row = Some(row);
                    } else {
                        units_row = Some(row);
                    }
                }
                (true, false) => sources_row = Some(row),
                (false, true) => units_row = Some(row),
                (true, true) => break,
            }
            lines.next();
        }

        // Phase 4: build channels for every column after `timestamp`.
        let field_at = |row: &Option<Vec<String>>, idx: usize| -> String {
            row.as_ref()
                .and_then(|r| r.get(idx))
                .cloned()
                .unwrap_or_default()
        };

        let sources: Vec<String> = (0..header_fields.len())
            .map(|idx| field_at(&sources_row, idx))
            .collect();
        let names = Self::build_channel_names(&header_fields, &sources);

        let channels: Vec<Channel> = (1..header_fields.len())
            .map(|idx| {
                Channel::RaceChrono(RaceChronoChannel {
                    name: names[idx].clone(),
                    unit: Self::normalize_unit(&field_at(&units_row, idx)),
                    source: sources[idx].clone(),
                })
            })
            .collect();

        let channel_count = channels.len();

        // Phase 5: parse data rows. Channels update at different rates, so
        // blank fields carry the last known value for that column (0.0
        // before the first valid sample), matching the Haltech, ECUMaster,
        // and MHD parsers. Times are re-based to the first record because
        // the raw timestamps are unix time.
        let estimated_rows = file_contents
            .as_bytes()
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        let mut times: Vec<f64> = Vec::with_capacity(estimated_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_rows);
        let mut last_values: Vec<Value> = vec![Value::Float(0.0); channel_count];
        let mut first_time: Option<f64> = None;
        let mut out_of_order_rows = 0usize;
        let mut malformed_rows = 0usize;

        for line in lines {
            let line = line.trim();
            if Self::is_blank_line(line) {
                continue;
            }

            // Cheap column count so a short row is rejected before any
            // last-known-value state is touched. Data rows are unquoted,
            // so every comma is a field separator.
            let field_count = 1 + line.as_bytes().iter().filter(|&&b| b == b',').count();
            if field_count < header_fields.len() {
                malformed_rows += 1;
                continue;
            }

            let mut fields = line.split(',');
            let time: f64 = match fields.next().unwrap_or("").trim().parse() {
                Ok(t) => t,
                Err(_) => {
                    // Row without a usable timestamp.
                    malformed_rows += 1;
                    continue;
                }
            };

            let base = *first_time.get_or_insert(time);
            let relative_time = time - base;

            // `times` must stay sorted: computed-channel time-shift lookups
            // binary-search it. Drop any row that steps backwards.
            if times.last().is_some_and(|last| relative_time < *last) {
                out_of_order_rows += 1;
                continue;
            }

            let mut row: Vec<Value> = Vec::with_capacity(channel_count);
            for (idx, field) in fields.take(channel_count).enumerate() {
                row.push(match field.trim().parse::<f64>() {
                    Ok(v) => {
                        let value = Value::Float(v);
                        last_values[idx] = value;
                        value
                    }
                    Err(_) => last_values[idx],
                });
            }

            times.push(relative_time);
            data.push(row);
        }

        if out_of_order_rows > 0 {
            tracing::warn!(
                "RaceChrono log contained {} out-of-order timestamps; those rows were dropped",
                out_of_order_rows
            );
        }
        if malformed_rows > 0 {
            tracing::warn!(
                "RaceChrono log contained {} malformed rows (short or missing timestamp); those rows were skipped",
                malformed_rows
            );
        }

        if times.is_empty() {
            return Err("No data rows found in RaceChrono log".into());
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
    fn errors_clearly_on_missing_format_line() {
        // A truncated preamble must not be reported as "version 0 is not
        // supported".
        let contents = "This file is created using RaceChrono Pro v10.1.3\n\
            Session title,\"Test\"\n\
            \n\
            timestamp,speed\n\
            100.0,1.0\n";

        let err = RaceChrono.parse(contents).unwrap_err().to_string();
        assert!(
            err.contains("no 'Format' metadata line"),
            "unexpected error: {}",
            err
        );
        assert!(!err.contains("version 0"), "unexpected error: {}", err);
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
    fn survives_spreadsheet_round_trip_padding() {
        // Excel/Numbers/Sheets pad every preamble line with trailing commas
        // out to the widest row when a user opens and re-saves the CSV.
        let contents = "This file is created using RaceChrono Pro v10.1.3,,,,,,\n\
            Format,3,,,,,\n\
            Session title,\"Padded Session\",,,,,\n\
            Created,04/08/2026,13:14,,,,\n\
            ,,,,,,\n\
            timestamp,speed,rpm,,,,\n\
            unix time,m/s,rpm,,,,\n\
            ,calc,300: canbus,,,,\n\
            100.0,1.0,1000,,,,\n\
            100.1,2.0,1100,,,,\n";

        // A padded blank line is no longer blank, so detection and the
        // preamble terminator both need the padding handled.
        let log = RaceChrono.parse(contents).expect("parse failed");
        match &log.meta {
            Meta::RaceChrono(meta) => {
                assert_eq!(meta.format_version, 3);
                assert_eq!(meta.session_title, "Padded Session");
                assert_eq!(meta.created, "04/08/2026,13:14");
            }
            other => panic!("expected RaceChrono metadata, got {:?}", other),
        }
        // Trailing padding columns must not become ghost channels
        let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
        assert_eq!(names, vec!["speed", "rpm"]);
        assert_eq!(log.data.len(), 2);
        assert_eq!(log.data[1][1].as_f64(), 1100.0);
    }

    #[test]
    fn joins_multiline_quoted_note() {
        let contents = "RaceChrono banner\n\
            Format,3\n\
            Note,\"first line\nsecond line\"\n\
            \n\
            timestamp,speed\n\
            100.0,1.0\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        match &log.meta {
            Meta::RaceChrono(meta) => {
                assert_eq!(meta.note, "first line\nsecond line");
            }
            other => panic!("expected RaceChrono metadata, got {:?}", other),
        }
        assert_eq!(log.data.len(), 1);
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
    fn dual_location_devices_keep_bare_latitude_longitude() {
        // Two location devices (phone GPS + external receiver) export the
        // same columns with the same short label. The first occurrence must
        // keep its bare name for the GPS Track Map; the second carries the
        // full source tag.
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,latitude,longitude,latitude,longitude\n\
            unix time,deg,deg,deg,deg\n\
            ,100: gps,100: gps,101: gps,101: gps\n\
            100.0,44.8,11.2,44.9,11.3\n\
            100.1,44.8,11.2,44.9,11.3\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
        assert_eq!(
            names,
            vec![
                "latitude",
                "longitude",
                "latitude (101: gps)",
                "longitude (101: gps)",
            ]
        );
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
    fn skips_rows_with_unusable_timestamps() {
        // A blank or garbage timestamp discards that row only, never the
        // whole file — including a garbage row sitting where the data
        // should start.
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,speed,rpm\n\
            unix time,m/s,rpm\n\
            ,calc,300: canbus\n\
            ,5.0,1500\n\
            100.0,1.0,1000\n\
            garbage,9.9,9999\n\
            100.1,2.0,1100\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        assert_eq!(log.times.len(), 2);
        assert_eq!(log.data[0][1].as_f64(), 1000.0);
        assert_eq!(log.data[1][1].as_f64(), 1100.0);
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
    fn sources_row_without_units_row_is_not_mistaken_for_units() {
        // The sources row is recognized by its numbered device tags, so a
        // file missing the units row still gets source-labeled duplicate
        // names and empty units — not source tags as units.
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,speed,speed,rpm\n\
            ,100: gps,calc,300: canbus\n\
            100.0,1.0,0.9,1000\n\
            100.1,2.0,1.9,1100\n";

        let log = RaceChrono.parse(contents).expect("parse failed");
        let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
        assert_eq!(names, vec!["speed (gps)", "speed (calc)", "rpm"]);
        assert!(log.channels.iter().all(|c| c.unit().is_empty()));
    }

    #[test]
    fn errors_on_zero_data_rows() {
        let contents = "RaceChrono banner\n\
            Format,3\n\
            \n\
            timestamp,speed\n\
            unix time,m/s\n";

        let err = RaceChrono.parse(contents).unwrap_err().to_string();
        assert!(err.contains("No data rows"), "unexpected error: {}", err);
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
