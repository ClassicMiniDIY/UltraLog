//! Emerald ECU (.lg1/.lg2) binary format parser
//!
//! Emerald K6/M3D ECUs use a proprietary binary format for log files:
//! - .lg2 file: Text file containing channel definitions (which parameters are logged)
//! - .lg1 file: Binary file containing timestamped data records
//!
//! Format structure:
//! - LG2 file: INI-like format with \[chan1\] through \[chan8\] sections mapping to channel IDs
//! - LG1 file: 24-byte records (8-byte OLE timestamp + 8 x 2-byte u16 values)
//!
//! The channel IDs map to specific ECU parameters (RPM, TPS, temperatures, etc.)
//!
//! LG1 values are stored as the logger's display value times a fixed factor
//! (TPS 0-1000 for 0-100.0 %), not as raw ECU bytes. Pressure is the exception:
//! the `[ValU]` section of the LG2 records the pressure unit the logger was set
//! to, and MAP is stored in that unit (see `PressureUnit`).

use serde::Serialize;
use std::error::Error;
use std::path::Path;

use super::types::{Channel, Log, Meta, Value};

/// Known Emerald ECU channel IDs and their metadata
/// These are reverse-engineered from observed data patterns
#[derive(Clone, Debug)]
struct ChannelDefinition {
    name: &'static str,
    unit: &'static str,
    /// Scale factor to apply to raw u16 value
    scale: f64,
    /// Offset to apply after scaling
    offset: f64,
}

/// Pressure unit recorded in the second value of the LG2 `[ValU]` section.
///
/// Two logs from the same car show the effect: with `1` MAP is stored in whole
/// kPa (25-235), with `2` it is stored in mbar (586 at idle, 2327 on boost).
/// Any other code keeps the mbar scaling the parser has always used.
///
/// Only ID 32 is known to follow this setting. The other pressure IDs (3, 10,
/// 33) are unconfirmed and keep a fixed x0.1 until a log shows otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PressureUnit {
    Kpa,
    #[default]
    Mbar,
}

impl PressureUnit {
    fn from_valu(code: Option<u8>) -> Self {
        match code {
            Some(1) => PressureUnit::Kpa,
            _ => PressureUnit::Mbar,
        }
    }

    /// Scale from the stored value to kPa
    fn kpa_scale(self) -> f64 {
        match self {
            PressureUnit::Kpa => 1.0,
            PressureUnit::Mbar => 0.1,
        }
    }
}

/// Channel layout and unit settings read from an LG2 file
#[derive(Clone, Debug, PartialEq)]
struct Lg2Config {
    /// (slot, channel ID) pairs, sorted by slot
    channels: Vec<(u8, u8)>,
    pressure_unit: PressureUnit,
}

/// Channel ID of MAP, the one channel scaled by the `[ValU]` pressure unit
const MAP_ID: u8 = 32;

/// Channel IDs confirmed against EM Soft by the log attached to issue #93
/// (MG ZS turbo). The other IDs are unconfirmed guesses from the original
/// reverse engineering.
const CONFIRMED_IDS: [u8; 8] = [1, 15, 20, 24, 26, 31, MAP_ID, 41];

/// Number of value slots in an LG1 record (`[chan1]`..`[chan8]`)
const SLOTS: u8 = 8;

/// Plausible OLE record timestamps: days since 1899-12-30, ~1995 to ~2050
const OLE_DATE_RANGE: std::ops::RangeInclusive<f64> = 35000.0..=55000.0;

/// Get channel definition for a known channel ID
fn get_channel_definition(id: u8) -> ChannelDefinition {
    match id {
        // "AFR/Lambda" in EM Soft: the K6's own lambda input, a 0-1 V
        // narrowband signal stored in mV (every log tops out at 900). The
        // name says "Voltage" so the table generators never take it as lambda.
        1 => ChannelDefinition {
            name: "Lambda Sensor Voltage",
            unit: "V",
            scale: 0.001,
            offset: 0.0,
        },
        2 => ChannelDefinition {
            name: "Air Temp",
            unit: "°C",
            scale: 1.0,
            offset: 0.0,
        },
        3 => ChannelDefinition {
            name: "MAP",
            unit: "kPa",
            scale: 0.1,
            offset: 0.0,
        },
        4 => ChannelDefinition {
            name: "Lambda",
            unit: "λ",
            scale: 0.001,
            offset: 0.0,
        },
        5 => ChannelDefinition {
            name: "Fuel Pressure",
            unit: "bar",
            scale: 0.01,
            offset: 0.0,
        },
        6 => ChannelDefinition {
            name: "Oil Pressure",
            unit: "bar",
            scale: 0.01,
            offset: 0.0,
        },
        7 => ChannelDefinition {
            name: "Oil Temp",
            unit: "°C",
            scale: 1.0,
            offset: 0.0,
        },
        8 => ChannelDefinition {
            name: "Fuel Temp",
            unit: "°C",
            scale: 1.0,
            offset: 0.0,
        },
        9 => ChannelDefinition {
            name: "Exhaust Temp",
            unit: "°C",
            scale: 1.0,
            offset: 0.0,
        },
        10 => ChannelDefinition {
            name: "Boost Target",
            unit: "kPa",
            scale: 0.1,
            offset: 0.0,
        },
        11 => ChannelDefinition {
            name: "Boost Duty",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        12 => ChannelDefinition {
            name: "Load",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        13 => ChannelDefinition {
            name: "Fuel Cut",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
        14 => ChannelDefinition {
            name: "Spark Cut",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
        // "BoostPWM" in EM Soft
        15 => ChannelDefinition {
            name: "Boost PWM",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        16 => ChannelDefinition {
            name: "Speed",
            unit: "km/h",
            scale: 0.1,
            offset: 0.0,
        },
        17 => ChannelDefinition {
            name: "Battery",
            unit: "V",
            scale: 0.01,
            offset: 0.0,
        },
        18 => ChannelDefinition {
            name: "AFR Target",
            unit: "AFR",
            scale: 0.1,
            offset: 0.0,
        },
        19 => ChannelDefinition {
            name: "Coolant Temp",
            unit: "°C",
            scale: 1.0,
            offset: 0.0,
        },
        20 => ChannelDefinition {
            name: "RPM",
            unit: "RPM",
            scale: 1.0,
            offset: 0.0,
        },
        21 => ChannelDefinition {
            name: "Ignition Advance",
            unit: "°",
            scale: 0.1,
            offset: 0.0,
        },
        22 => ChannelDefinition {
            name: "Inj Pulse Width",
            unit: "ms",
            scale: 0.01,
            offset: 0.0,
        },
        23 => ChannelDefinition {
            name: "Inj Duty Cycle",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        // "Ign Adv" in EM Soft. Stored in 0.5° steps with a +50° offset:
        // 555 at idle -> 5.5°, 850 at light-load cruise -> 35°, 640-675 at
        // 230 kPa boost -> 14-17.5°. The offset is inferred from those
        // values, not from Emerald documentation.
        24 => ChannelDefinition {
            name: "Ignition Advance",
            unit: "°",
            scale: 0.1,
            offset: -50.0,
        },
        25 => ChannelDefinition {
            name: "Coolant Temp Corr",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        // "Inj Duration" in EM Soft, which reports it as duty (%), not ms
        26 => ChannelDefinition {
            name: "Inj Duration",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        27 => ChannelDefinition {
            name: "Acceleration Enrich",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        28 => ChannelDefinition {
            name: "Warmup Enrich",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        29 => ChannelDefinition {
            name: "Ignition Timing",
            unit: "°BTDC",
            scale: 0.1,
            offset: 0.0,
        },
        30 => ChannelDefinition {
            name: "Idle Valve",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        // "Load site" in EM Soft: the fuel/ignition map row index (0-15)
        31 => ChannelDefinition {
            name: "Load Site",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
        // Scale here is the mbar default; parse_binary_with_channels
        // replaces it from the [ValU] pressure unit.
        MAP_ID => ChannelDefinition {
            name: "MAP",
            unit: "kPa",
            scale: 0.1,
            offset: 0.0,
        },
        33 => ChannelDefinition {
            name: "Barometric Pressure",
            unit: "kPa",
            scale: 0.1,
            offset: 0.0,
        },
        34 => ChannelDefinition {
            name: "Aux Input 34",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
        35 => ChannelDefinition {
            name: "Aux Input 35",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
        // "Throttle Pos" in EM Soft
        41 => ChannelDefinition {
            name: "TPS",
            unit: "%",
            scale: 0.1,
            offset: 0.0,
        },
        // AFR/Lambda channels
        45 => ChannelDefinition {
            name: "AFR",
            unit: "AFR",
            scale: 0.1,
            offset: 0.0,
        },
        46 => ChannelDefinition {
            name: "AFR",
            unit: "AFR",
            scale: 0.1,
            offset: 0.0,
        },
        47 => ChannelDefinition {
            name: "Lambda",
            unit: "λ",
            scale: 0.01,
            offset: 0.0,
        },
        // Default for unknown channels
        _ => ChannelDefinition {
            name: "Unknown",
            unit: "",
            scale: 1.0,
            offset: 0.0,
        },
    }
}

/// Append the channel ID to repeated names.
///
/// Several IDs share a name (21 and 24 are both ignition advance, 45 and 46
/// are both AFR), and name lookups return the first match, so a repeat would
/// be unreachable by name. A confirmed ID keeps the plain name whatever slot
/// it is in; the others get ` (ID n)`.
fn disambiguate_names(channels: &mut [EmeraldChannel]) {
    for i in 0..channels.len() {
        let group: Vec<usize> = (0..channels.len())
            .filter(|&j| channels[j].name == channels[i].name)
            .collect();
        if group.len() < 2 {
            continue;
        }
        let keep = group
            .iter()
            .copied()
            .find(|&j| CONFIRMED_IDS.contains(&channels[j].channel_id))
            .unwrap_or(group[0]);
        for j in group {
            if j != keep {
                channels[j].name = format!("{} (ID {})", channels[j].name, channels[j].channel_id);
            }
        }
    }
}

/// Emerald ECU channel metadata
#[derive(Clone, Debug, Serialize)]
pub struct EmeraldChannel {
    pub name: String,
    pub unit: String,
    pub channel_id: u8,
    /// Scale factor applied to convert raw u16 to engineering value
    #[serde(skip)]
    pub scale: f64,
    /// Offset applied after scaling
    #[serde(skip)]
    pub offset: f64,
}

impl EmeraldChannel {
    /// Get the display unit for this channel
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Emerald ECU log metadata
#[derive(Clone, Debug, Serialize, Default)]
pub struct EmeraldMeta {
    /// Source file name (without extension)
    pub source_file: String,
    /// Number of records in the log
    pub record_count: usize,
    /// Duration of the log in seconds
    pub duration_seconds: f64,
    /// Sample rate in Hz (approximate)
    pub sample_rate_hz: f64,
}

/// Emerald ECU log file parser
pub struct Emerald;

impl Emerald {
    /// Check if a file path looks like an Emerald log file (.lg1 or .lg2)
    pub fn is_emerald_path(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            ext_lower == "lg1" || ext_lower == "lg2"
        } else {
            false
        }
    }

    /// Check if a file path is specifically an LG1 file
    pub fn is_lg1_path(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            ext.to_string_lossy().to_lowercase() == "lg1"
        } else {
            false
        }
    }

    /// Check if a file path is specifically an LG2 file
    pub fn is_lg2_path(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            ext.to_string_lossy().to_lowercase() == "lg2"
        } else {
            false
        }
    }

    /// Detect if binary data is Emerald LG1 format
    /// LG1 files have 24-byte records with OLE timestamp at the start
    pub fn detect(data: &[u8]) -> bool {
        // Must have at least one complete record (24 bytes)
        if data.len() < 24 {
            return false;
        }

        // File size must be a multiple of 24 bytes
        if !data.len().is_multiple_of(24) {
            return false;
        }

        // Check if first 8 bytes look like a valid OLE date
        // OLE dates are f64 days since 1899-12-30
        // Valid range: ~35000 (1995) to ~55000 (2050)
        let timestamp = f64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);

        // Check for reasonable OLE date range
        if !OLE_DATE_RANGE.contains(&timestamp) {
            return false;
        }

        // Check that subsequent records also have valid timestamps
        if data.len() >= 48 {
            let timestamp2 = f64::from_le_bytes([
                data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
            ]);

            // Second timestamp should be close to first (within 1 day). A zero
            // is EM Soft filler (see parse_binary_with_channels), not a mismatch.
            if timestamp2 != 0.0 && (timestamp2 - timestamp).abs() > 1.0 {
                return false;
            }
        }

        true
    }

    /// Detect if text data is Emerald LG2 format (channel definitions)
    /// LG2 files have \[chan1\] through \[chan8\] sections
    pub fn detect_lg2(data: &[u8]) -> bool {
        // Must be valid UTF-8 text
        let text = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Must contain [chan1] section marker
        if !text.contains("[chan1]") {
            return false;
        }

        // Should have at least a few channel definitions
        let channel_count = (1..=8)
            .filter(|i| text.contains(&format!("[chan{}]", i)))
            .count();

        channel_count >= 4
    }

    /// Parse the LG2 channel definition file
    fn parse_lg2(contents: &str) -> Result<Lg2Config, Box<dyn Error>> {
        let mut channels: Vec<(u8, u8)> = Vec::new();
        let mut valu: Option<Vec<&str>> = None;

        let lines: Vec<&str> = contents.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Look for [chanN] headers
            if line.starts_with("[chan") && line.ends_with(']') {
                // Extract channel slot number (1-8)
                let slot_str = &line[5..line.len() - 1];
                if let Ok(slot) = slot_str.parse::<u8>() {
                    // Next line should be the channel ID
                    if i + 1 < lines.len() {
                        let id_line = lines[i + 1].trim();
                        // An LG1 record has 8 value slots; a slot outside
                        // 1-8, or a repeat, has no column of its own.
                        if let Ok(channel_id) = id_line.parse::<u8>()
                            && (1..=SLOTS).contains(&slot)
                            && !channels.iter().any(|(s, _)| *s == slot)
                        {
                            channels.push((slot, channel_id));
                        }
                        i += 1;
                    }
                }
            } else if line == "[ValU]" {
                // One value per line up to the next section. Blank lines are
                // skipped so they cannot shift which value is which.
                valu = Some(
                    lines[i + 1..]
                        .iter()
                        .map(|l| l.trim())
                        .take_while(|l| !l.starts_with('['))
                        .filter(|l| !l.is_empty())
                        .collect(),
                );
            }

            i += 1;
        }

        if channels.is_empty() {
            return Err("No channel definitions found in LG2 file".into());
        }

        // Sort by slot number to ensure correct order
        channels.sort_by_key(|(slot, _)| *slot);

        Ok(Lg2Config {
            channels,
            // The second [ValU] value is the pressure unit
            pressure_unit: PressureUnit::from_valu(
                valu.and_then(|v| v.get(1).and_then(|c| c.parse::<u8>().ok())),
            ),
        })
    }

    /// Parse Emerald log files (requires both .lg1 and .lg2)
    pub fn parse_file(path: &Path) -> Result<Log, Box<dyn Error>> {
        // Determine the base path (without extension)
        let base_path = path.with_extension("");

        // Read LG2 file (channel definitions)
        let lg2_path = base_path.with_extension("lg2");
        let lg2_contents = std::fs::read_to_string(&lg2_path).map_err(|e| {
            format!(
                "Cannot read LG2 file '{}': {}. Both .lg1 and .lg2 files are required.",
                lg2_path.display(),
                e
            )
        })?;

        // Parse channel definitions
        let config = Self::parse_lg2(&lg2_contents)?;

        // Read LG1 file (binary data)
        let lg1_path = base_path.with_extension("lg1");
        let lg1_data = std::fs::read(&lg1_path).map_err(|e| {
            format!(
                "Cannot read LG1 file '{}': {}. Both .lg1 and .lg2 files are required.",
                lg1_path.display(),
                e
            )
        })?;

        Self::parse_binary_with_channels(&lg1_data, &config, path)
    }

    /// Parse the LG1 binary data with channel definitions
    fn parse_binary_with_channels(
        data: &[u8],
        config: &Lg2Config,
        source_path: &Path,
    ) -> Result<Log, Box<dyn Error>> {
        if !Self::detect(data) {
            return Err("Invalid LG1 file - not recognized as Emerald format".into());
        }

        const RECORD_SIZE: usize = 24;
        let num_records = data.len() / RECORD_SIZE;

        if num_records == 0 {
            return Err("LG1 file contains no data records".into());
        }

        // Build channel metadata. `columns` holds each channel's byte offset
        // inside a record: the value for [chanN] is always column N, even
        // when an earlier slot is missing from the LG2.
        let mut channels: Vec<EmeraldChannel> = Vec::with_capacity(config.channels.len());
        let mut columns: Vec<usize> = Vec::with_capacity(config.channels.len());
        for (slot, channel_id) in &config.channels {
            let def = get_channel_definition(*channel_id);
            let name = if def.name == "Unknown" {
                format!("Channel {} (ID {})", slot, channel_id)
            } else {
                def.name.to_string()
            };
            let scale = if *channel_id == MAP_ID {
                config.pressure_unit.kpa_scale()
            } else {
                def.scale
            };

            channels.push(EmeraldChannel {
                name,
                unit: def.unit.to_string(),
                channel_id: *channel_id,
                scale,
                offset: def.offset,
            });
            columns.push(8 + (*slot as usize - 1) * 2);
        }
        disambiguate_names(&mut channels);

        // Parse binary data
        let mut times: Vec<f64> = Vec::with_capacity(num_records);
        let mut data_matrix: Vec<Vec<Value>> = Vec::with_capacity(num_records);

        let mut first_timestamp: Option<f64> = None;
        let mut last_timestamp = f64::NEG_INFINITY;
        let mut skipped = 0usize;

        for i in 0..num_records {
            let offset = i * RECORD_SIZE;

            // Read OLE timestamp (8 bytes, f64)
            let ole_timestamp = f64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // EM Soft writes filler records with a zero timestamp (5 % of the
            // issue #93 log) that repeat the previous values. Skip them, and
            // any backwards timestamp, so times stay monotonic: a zero would
            // otherwise land ~4e9 s before the log start.
            if !OLE_DATE_RANGE.contains(&ole_timestamp) || ole_timestamp < last_timestamp {
                skipped += 1;
                continue;
            }
            last_timestamp = ole_timestamp;

            // Convert OLE date to seconds since start
            let first_ts = *first_timestamp.get_or_insert(ole_timestamp);
            let time_seconds = (ole_timestamp - first_ts) * 24.0 * 60.0 * 60.0;
            times.push(time_seconds);

            // Read 8 channel values (16 bytes, 8 x u16)
            let mut row: Vec<Value> = Vec::with_capacity(channels.len());
            for (channel, column) in channels.iter().zip(&columns) {
                let value_offset = offset + column;
                let raw_value =
                    u16::from_le_bytes([data[value_offset], data[value_offset + 1]]) as f64;

                // Apply scaling and offset
                let scaled_value = raw_value * channel.scale + channel.offset;
                row.push(Value::Float(scaled_value));
            }

            data_matrix.push(row);
        }

        if times.is_empty() {
            return Err("LG1 file contains no records with a valid timestamp".into());
        }
        if skipped > 0 {
            tracing::warn!(
                "Skipped {} of {} Emerald records with an invalid or backwards timestamp",
                skipped,
                num_records
            );
        }
        let record_count = times.len();

        // Calculate metadata
        let duration = times.last().copied().unwrap_or(0.0);
        let sample_rate = if duration > 0.0 {
            record_count as f64 / duration
        } else {
            0.0
        };

        let source_file = source_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let meta = EmeraldMeta {
            source_file,
            record_count,
            duration_seconds: duration,
            sample_rate_hz: sample_rate,
        };

        tracing::info!(
            "Parsed Emerald ECU log: {} channels, {} records, {:.1}s duration, {:.1} Hz",
            channels.len(),
            record_count,
            duration,
            sample_rate
        );

        Ok(Log {
            meta: Meta::Emerald(meta),
            channels: channels.into_iter().map(Channel::Emerald).collect(),
            times,
            data: data_matrix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_valid_lg1() {
        // Create minimal valid LG1 data (one record)
        let mut data = vec![0u8; 24];

        // Write a valid OLE timestamp (e.g., 46022.5 = Dec 2025)
        let timestamp: f64 = 46022.5;
        data[0..8].copy_from_slice(&timestamp.to_le_bytes());

        // Write some channel values
        for i in 0..8 {
            let value: u16 = (i * 100) as u16;
            let offset = 8 + i * 2;
            data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }

        assert!(Emerald::detect(&data));
    }

    #[test]
    fn test_detect_invalid_data() {
        // Empty data
        assert!(!Emerald::detect(&[]));

        // Too short
        assert!(!Emerald::detect(&[0u8; 23]));

        // Wrong size (not a multiple of 24)
        assert!(!Emerald::detect(&[0u8; 25]));

        // Invalid timestamp (too old)
        let mut data = vec![0u8; 24];
        let old_timestamp: f64 = 1000.0; // Way too old
        data[0..8].copy_from_slice(&old_timestamp.to_le_bytes());
        assert!(!Emerald::detect(&data));

        // Invalid timestamp (too new)
        let mut data = vec![0u8; 24];
        let future_timestamp: f64 = 100000.0; // Way too far in future
        data[0..8].copy_from_slice(&future_timestamp.to_le_bytes());
        assert!(!Emerald::detect(&data));
    }

    #[test]
    fn test_parse_lg2() {
        let lg2_content = "[chan1]\n19\n[chan2]\n46\n[chan3]\n2\n[chan4]\n20\n[chan5]\n1\n[chan6]\n31\n[chan7]\n32\n[chan8]\n17\n[ValU]\n0\n2\n0\n0\n0\n";

        let config = Emerald::parse_lg2(lg2_content).unwrap();
        let channels = &config.channels;
        assert_eq!(channels.len(), 8);
        assert_eq!(channels[0], (1, 19)); // Coolant Temp
        assert_eq!(channels[1], (2, 46)); // AFR
        assert_eq!(channels[2], (3, 2)); // Air Temp
        assert_eq!(channels[3], (4, 20)); // RPM
        assert_eq!(channels[4], (5, 1)); // Lambda Sensor
        assert_eq!(channels[5], (6, 31)); // Load Site
        assert_eq!(channels[6], (7, 32)); // MAP
        assert_eq!(channels[7], (8, 17)); // Battery
        assert_eq!(config.pressure_unit, PressureUnit::Mbar);
    }

    #[test]
    fn test_parse_lg2_pressure_unit() {
        let kpa = "[chan1]\n20\n[chan2]\n32\n[chan3]\n1\n[chan4]\n41\n[ValU]\n0\n1\n0\n0\n0\n";
        assert_eq!(
            Emerald::parse_lg2(kpa).unwrap().pressure_unit,
            PressureUnit::Kpa
        );

        // A missing [ValU] section keeps the mbar scaling
        let none = "[chan1]\n20\n[chan2]\n32\n[chan3]\n1\n[chan4]\n41\n";
        assert_eq!(
            Emerald::parse_lg2(none).unwrap().pressure_unit,
            PressureUnit::Mbar
        );
    }

    #[test]
    fn test_channel_definitions() {
        // Test known channel IDs
        let rpm = get_channel_definition(20);
        assert_eq!(rpm.name, "RPM");
        assert_eq!(rpm.unit, "RPM");

        let coolant = get_channel_definition(19);
        assert_eq!(coolant.name, "Coolant Temp");
        assert_eq!(coolant.unit, "°C");

        // Issue #93: ID 41 is Throttle Pos and ID 1 is the lambda input
        let tps = get_channel_definition(41);
        assert_eq!(tps.name, "TPS");
        assert_eq!(tps.unit, "%");
        assert_eq!(get_channel_definition(1).name, "Lambda Sensor Voltage");

        // Test unknown channel
        let unknown = get_channel_definition(255);
        assert_eq!(unknown.name, "Unknown");
    }

    #[test]
    fn test_is_emerald_path() {
        assert!(Emerald::is_emerald_path(Path::new("test.lg1")));
        assert!(Emerald::is_emerald_path(Path::new("test.lg2")));
        assert!(Emerald::is_emerald_path(Path::new("test.LG1")));
        assert!(Emerald::is_emerald_path(Path::new("/path/to/file.lg2")));

        assert!(!Emerald::is_emerald_path(Path::new("test.csv")));
        assert!(!Emerald::is_emerald_path(Path::new("test.llg")));
        assert!(!Emerald::is_emerald_path(Path::new("test")));
    }

    #[test]
    fn test_parse_emerald_example_files() {
        // Try to parse the example files
        let base_path = Path::new("exampleLogs/emerald/EM Log MG ZS Turbo idle and rev");

        // Check if files exist
        let lg1_path = base_path.with_extension("lg1");
        let lg2_path = base_path.with_extension("lg2");

        if !lg1_path.exists() || !lg2_path.exists() {
            eprintln!(
                "Skipping test: example files not found at {}",
                base_path.display()
            );
            return;
        }

        // Parse the files
        let log = Emerald::parse_file(&lg1_path).expect("Should parse successfully");

        // Verify structure
        assert_eq!(log.channels.len(), 8, "Should have 8 channels");
        assert!(!log.times.is_empty(), "Should have time data");
        assert!(!log.data.is_empty(), "Should have data records");

        // Verify channel names
        for channel in &log.channels {
            let name = channel.name();
            assert!(!name.is_empty(), "Channel name should not be empty");
            eprintln!("Channel: {} ({})", name, channel.unit());
        }

        // Verify metadata
        if let Meta::Emerald(meta) = &log.meta {
            eprintln!("Source: {}", meta.source_file);
            eprintln!("Records: {}", meta.record_count);
            eprintln!("Duration: {:.1}s", meta.duration_seconds);
            eprintln!("Sample rate: {:.1} Hz", meta.sample_rate_hz);
        }

        eprintln!("Parsed {} data records", log.data.len());
    }

    /// First-record values as a (name, unit, value) list
    fn first_record(log: &Log) -> Vec<(String, String, f64)> {
        log.channels
            .iter()
            .zip(&log.data[0])
            .map(|(ch, v)| (ch.name(), ch.unit().to_string(), v.as_f64()))
            .collect()
    }

    fn assert_channel(record: &[(String, String, f64)], name: &str, unit: &str, value: f64) {
        let (_, got_unit, got) = record
            .iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("missing channel {name}: {record:?}"));
        assert_eq!(got_unit, unit, "{name} unit");
        assert!((got - value).abs() < 1e-9, "{name}: {got} != {value}");
    }

    #[test]
    fn test_issue_93_channel_mapping() {
        // Log attached to issue #93: [chan1..8] = 20, 41, 1, 32, 24, 26, 15, 31,
        // [ValU] pressure unit 1 (kPa). The reporter's EM Soft labels are
        // Engine Speed, Throttle Pos, AFR/Lambda, MAP, Ign Adv, Inj Duration,
        // BoostPWM, Load site.
        let path = Path::new("exampleLogs/emerald/EM Log MG ZS Turbo boost run.lg1");
        let log = Emerald::parse_file(path).expect("Should parse successfully");
        // 105 of the 2000 records are zero-timestamp filler
        assert_eq!(log.data.len(), 1895);
        assert!(log.times.windows(2).all(|w| w[0] <= w[1]));

        let record = first_record(&log);
        let names: Vec<&str> = record.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "RPM",
                "TPS",
                "Lambda Sensor Voltage",
                "MAP",
                "Ignition Advance",
                "Inj Duration",
                "Boost PWM",
                "Load Site"
            ]
        );

        // Raw first record: 3732, 580, 430, 150, 745, 210, 286, 11
        assert_channel(&record, "RPM", "RPM", 3732.0);
        assert_channel(&record, "TPS", "%", 58.0);
        assert_channel(&record, "Lambda Sensor Voltage", "V", 0.43);
        assert_channel(&record, "MAP", "kPa", 150.0);
        assert_channel(&record, "Ignition Advance", "°", 24.5);
        assert_channel(&record, "Inj Duration", "%", 21.0);
        assert_channel(&record, "Boost PWM", "%", 28.6);
        assert_channel(&record, "Load Site", "", 11.0);

        // Load site is a 0-15 map row index for the whole log
        let slot = names.iter().position(|n| *n == "Load Site").unwrap();
        assert!(
            log.data
                .iter()
                .all(|r| (0.0..=15.0).contains(&r[slot].as_f64()))
        );
    }

    /// One LG1 record whose 8 slot values are `values`
    fn record(values: [u16; 8]) -> Vec<u8> {
        let mut data = 46022.5f64.to_le_bytes().to_vec();
        for v in values {
            data.extend_from_slice(&v.to_le_bytes());
        }
        data
    }

    fn parse_config(config: &Lg2Config, data: &[u8]) -> Log {
        Emerald::parse_binary_with_channels(data, config, Path::new("x.lg1"))
            .expect("Should parse successfully")
    }

    #[test]
    fn test_duplicate_names_get_the_channel_id() {
        // The confirmed ID 24 keeps the plain name in either slot order
        for channels in [vec![(1, 21), (2, 24)], vec![(1, 24), (2, 21)]] {
            let config = Lg2Config {
                channels,
                pressure_unit: PressureUnit::Mbar,
            };
            let log = parse_config(&config, &record([0; 8]));
            let plain = log
                .channels
                .iter()
                .find(|c| c.name() == "Ignition Advance")
                .expect("plain name kept");
            let Channel::Emerald(ch) = plain else {
                panic!("not an Emerald channel")
            };
            assert_eq!(ch.channel_id, 24);
            assert!(
                log.channels
                    .iter()
                    .any(|c| c.name() == "Ignition Advance (ID 21)")
            );
        }

        // Neither AFR ID is confirmed: the first keeps the plain name
        let config = Lg2Config {
            channels: vec![(1, 45), (2, 46)],
            pressure_unit: PressureUnit::Mbar,
        };
        let names: Vec<String> = parse_config(&config, &record([0; 8]))
            .channels
            .iter()
            .map(|c| c.name())
            .collect();
        assert_eq!(names, ["AFR", "AFR (ID 46)"]);
    }

    #[test]
    fn test_value_column_follows_slot_number() {
        // [chan2] is unreadable, so slot 3 must still read column 3
        let lg2 = "[chan1]\n20\n[chan2]\n\n[chan3]\n41\n[chan4]\n19\n";
        let config = Emerald::parse_lg2(lg2).unwrap();
        assert_eq!(config.channels, [(1, 20), (3, 41), (4, 19)]);

        let log = parse_config(&config, &record([3000, 999, 500, 90, 0, 0, 0, 0]));
        let values: Vec<f64> = log.data[0].iter().map(|v| v.as_f64()).collect();
        assert_eq!(values, [3000.0, 50.0, 90.0]);
    }

    #[test]
    fn test_slots_outside_record_are_dropped() {
        // [chan9] has no column and a repeated [chan1] would read slot 1 twice
        let lg2 = "[chan1]\n20\n[chan1]\n41\n[chan9]\n19\n[chan0]\n2\n[chan8]\n31\n";
        let config = Emerald::parse_lg2(lg2).unwrap();
        assert_eq!(config.channels, [(1, 20), (8, 31)]);

        // Parsing the last record must not read past the end of the data
        let data = [record([1; 8]), record([2; 8])].concat();
        assert_eq!(parse_config(&config, &data).data.len(), 2);
    }

    #[test]
    fn test_valu_ignores_blank_lines() {
        let lg2 = "[chan1]\n20\n[chan2]\n32\n[chan3]\n1\n[chan4]\n41\n[ValU]\n\n0\n\n1\n0\n0\n0\n";
        assert_eq!(
            Emerald::parse_lg2(lg2).unwrap().pressure_unit,
            PressureUnit::Kpa
        );

        // A [ValU] with a single value has no pressure code
        let short = "[chan1]\n20\n[chan2]\n32\n[chan3]\n1\n[chan4]\n41\n[ValU]\n0\n[chan5]\n1\n";
        assert_eq!(
            Emerald::parse_lg2(short).unwrap().pressure_unit,
            PressureUnit::Mbar
        );
    }

    #[test]
    fn test_zero_and_backwards_timestamps_are_skipped() {
        let config = Lg2Config {
            channels: vec![(1, 20)],
            pressure_unit: PressureUnit::Mbar,
        };
        let at = |days: f64, rpm: u16| {
            let mut r = record([rpm, 0, 0, 0, 0, 0, 0, 0]);
            r[0..8].copy_from_slice(&days.to_le_bytes());
            r
        };
        let second = 1.0 / 86_400.0;
        let data = [
            at(46022.5, 1000),
            at(0.0, 1000),
            at(46022.5 + second, 2000),
            at(46022.5 + second / 2.0, 3000),
            at(46022.5 + 2.0 * second, 4000),
        ]
        .concat();
        let log = parse_config(&config, &data);
        let rpm: Vec<f64> = log.data.iter().map(|r| r[0].as_f64()).collect();
        assert_eq!(rpm, [1000.0, 2000.0, 4000.0]);
        assert!((log.times[2] - 2.0).abs() < 1e-6);
        if let Meta::Emerald(meta) = &log.meta {
            assert_eq!(meta.record_count, 3);
        }
    }

    #[test]
    fn test_map_scale_follows_pressure_unit() {
        for (unit, expected) in [(PressureUnit::Kpa, 150.0), (PressureUnit::Mbar, 15.0)] {
            let config = Lg2Config {
                channels: vec![(1, MAP_ID)],
                pressure_unit: unit,
            };
            let log = parse_config(&config, &record([150, 0, 0, 0, 0, 0, 0, 0]));
            assert_eq!(log.data[0][0].as_f64(), expected);
        }
    }

    #[test]
    fn test_mbar_pressure_unit_map() {
        // [ValU] pressure unit 2: MAP stored in mbar, 586 at idle
        let path = Path::new("exampleLogs/emerald/EM Log MG ZS Turbo idle and rev.lg1");
        let log = Emerald::parse_file(path).expect("Should parse successfully");
        let record = first_record(&log);
        assert_channel(&record, "MAP", "kPa", 58.6);
        assert_channel(&record, "Lambda Sensor Voltage", "V", 0.52);
        assert_channel(&record, "Load Site", "", 0.0);
    }
}
