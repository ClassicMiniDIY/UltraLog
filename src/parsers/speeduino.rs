//! Speeduino/rusEFI MegaLogViewer (.mlg) binary format parser
//!
//! Speeduino and rusEFI both use the MegaLogViewer binary format for data logging.
//! Format structure based on mlg-converter reference:
//! - Header: "MLVLG" (6 bytes including version byte)
//! - Format version (int16) and metadata
//! - Field definitions (55 bytes for v1, 89 bytes for v2)
//! - Binary data records (block type + timestamp + field values)

use serde::Serialize;
use std::error::Error;

use super::types::{Log, Parseable, Value};

/// MLG field data types (from mlg-converter)
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum FieldType {
    U08 = 0,
    S08 = 1,
    U16 = 2,
    S16 = 3,
    U32 = 4,
    S32 = 5,
    S64 = 6,
    F32 = 7,
    U08Bitfield = 10,
    U16Bitfield = 11,
    U32Bitfield = 12,
}

impl FieldType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::U08),
            1 => Some(Self::S08),
            2 => Some(Self::U16),
            3 => Some(Self::S16),
            4 => Some(Self::U32),
            5 => Some(Self::S32),
            6 => Some(Self::S64),
            7 => Some(Self::F32),
            10 => Some(Self::U08Bitfield),
            11 => Some(Self::U16Bitfield),
            12 => Some(Self::U32Bitfield),
            _ => None,
        }
    }

    fn byte_size(&self) -> usize {
        match self {
            Self::U08 | Self::S08 | Self::U08Bitfield => 1,
            Self::U16 | Self::S16 | Self::U16Bitfield => 2,
            Self::U32 | Self::S32 | Self::F32 | Self::U32Bitfield => 4,
            Self::S64 => 8,
        }
    }
}

/// Speeduino field metadata
#[derive(Clone, Debug, Serialize)]
pub struct SpeeduinoChannel {
    pub name: String,
    pub unit: String,
    pub scale: f32,
    pub transform: f32,
    pub field_type: u8,
}

impl SpeeduinoChannel {
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Speeduino log metadata
#[derive(Clone, Debug, Serialize, Default)]
pub struct SpeeduinoMeta {
    pub version: String,
    pub capture_date: String,
}

/// Speeduino parser for MegaLogViewer binary format
pub struct Speeduino;

impl Speeduino {
    /// Detect if data is Speeduino MegaLogViewer format
    pub fn detect(data: &[u8]) -> bool {
        data.len() >= 5 && &data[0..5] == b"MLVLG"
    }

    /// Parse MegaLogViewer binary format (based on mlg-converter reference)
    pub fn parse_binary(data: &[u8]) -> Result<Log, Box<dyn Error>> {
        let mut offset = 0;

        // Read file format (6 bytes: "MLVLG" + 1 extra byte)
        if data.len() < 6 || &data[0..5] != b"MLVLG" {
            return Err("Invalid MLG file header".into());
        }
        offset += 6;

        // Read format version (int16, big-endian like DataView default)
        let format_version = i16::from_be_bytes([data[offset], data[offset + 1]]);
        offset += 2;

        let is_v2 = format_version == 2;
        let field_length = if is_v2 { 89 } else { 55 };

        eprintln!(
            "DEBUG: MLG format version: {}, field_length: {}",
            format_version, field_length
        );

        // Read timestamp (int32, big-endian)
        offset += 4;

        // Read info_data_start (int16 for v1, int32 for v2, big-endian)
        let info_data_start = if is_v2 {
            u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize
        } else {
            u16::from_be_bytes([data[offset], data[offset + 1]]) as usize
        };
        offset += if is_v2 { 4 } else { 2 };

        // Read data_begin_index (int32, big-endian)
        let data_begin_index = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Read record_length (int16, big-endian)
        offset += 2;

        // Read num_logger_fields (int16, big-endian)
        let num_fields = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        eprintln!(
            "DEBUG: num_fields: {}, data_begin_index: {}",
            num_fields, data_begin_index
        );

        // Validate bounds before parsing
        if num_fields > 1000 {
            return Err(format!("Unreasonable field count: {}", num_fields).into());
        }
        if data_begin_index > data.len() {
            return Err(format!(
                "data_begin_index {} exceeds file size {}",
                data_begin_index,
                data.len()
            )
            .into());
        }

        // Parse field definitions
        let mut channels = Vec::new();
        for i in 0..num_fields {
            if offset + field_length > data.len() {
                return Err(format!(
                    "Not enough data for field {} at offset {} (need {}, have {})",
                    i,
                    offset,
                    field_length,
                    data.len() - offset
                )
                .into());
            }
            // Read type (1 byte)
            let field_type = data[offset];
            offset += 1;

            // Read name (34 bytes)
            let name_bytes = &data[offset..offset + 34];
            let name = String::from_utf8_lossy(name_bytes)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            offset += 34;

            // Read units (10 bytes)
            let units_bytes = &data[offset..offset + 10];
            let unit = String::from_utf8_lossy(units_bytes)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            offset += 10;

            // Read display_style (1 byte)
            offset += 1;

            let (scale, transform) = if field_type < 10 {
                // Scalar field
                let scale = f32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                let transform = f32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;

                // Skip digits (1 byte)
                offset += 1;

                // Skip category if v2 (34 bytes)
                if is_v2 {
                    offset += 34;
                }

                (scale, transform)
            } else {
                // Bitfield - skip remaining bytes
                offset += field_length - 46; // Already read 46 bytes
                (1.0, 0.0)
            };

            channels.push(SpeeduinoChannel {
                name,
                unit,
                scale,
                transform,
                field_type,
            });
        }

        eprintln!("DEBUG: Parsed {} channels", channels.len());
        for (idx, ch) in channels.iter().enumerate() {
            eprintln!(
                "  [{}] {} ({}) type={} scale={} transform={}",
                idx, ch.name, ch.unit, ch.field_type, ch.scale, ch.transform
            );
        }

        // Extract metadata from info section
        let mut meta = SpeeduinoMeta::default();
        if info_data_start < data_begin_index && data_begin_index < data.len() {
            let info_bytes = &data[info_data_start..data_begin_index];
            let info_str = String::from_utf8_lossy(info_bytes);

            if let Some(version_start) = info_str.find("speeduino") {
                if let Some(version_end) = info_str[version_start..].find('"') {
                    meta.version = info_str[version_start..version_start + version_end].to_string();
                }
            }
            if let Some(date_start) = info_str.find("Capture Date:") {
                if let Some(date_end) = info_str[date_start..].find('"') {
                    meta.capture_date = info_str[date_start..date_start + date_end].to_string();
                }
            }
        }

        // Parse data blocks
        offset = data_begin_index;
        // Estimate record count: remaining data / approximate record size (timestamp + data + CRC)
        // Each record is roughly: 1 (block type) + 2 (timestamp) + num_fields * ~4 bytes + 1 (CRC)
        let remaining_data = data.len().saturating_sub(data_begin_index);
        let estimated_record_size = 4 + channels.len() * 4;
        let estimated_records = if estimated_record_size > 0 {
            remaining_data / estimated_record_size
        } else {
            1000 // Fallback estimate
        };
        let mut times: Vec<f64> = Vec::with_capacity(estimated_records);
        let mut data_records: Vec<Vec<Value>> = Vec::with_capacity(estimated_records);

        // Track timestamp wraparound (u16 wraps at 65535ms = 65.535 seconds)
        let mut prev_raw_timestamp: u16 = 0;
        let mut wrap_count: u64 = 0;
        // If timestamp drops by more than 30 seconds, it definitely wrapped
        // (actual wraparounds show ~58.7s drop when going from ~65s to ~6s)
        const WRAP_THRESHOLD: u16 = 30000;

        while offset + 4 <= data.len() {
            // Read block type (1 byte)
            let block_type = data[offset];
            offset += 1;

            // Skip counter (1 byte)
            offset += 1;

            // Read timestamp (uint16, big-endian)
            if offset + 2 > data.len() {
                break;
            }
            let raw_timestamp = u16::from_be_bytes([data[offset], data[offset + 1]]);
            offset += 2;

            // Detect wraparound: if current timestamp is much smaller than previous, it wrapped
            if raw_timestamp < prev_raw_timestamp
                && (prev_raw_timestamp - raw_timestamp) > WRAP_THRESHOLD
            {
                wrap_count += 1;
            }
            prev_raw_timestamp = raw_timestamp;

            // Calculate actual timestamp with wraparound compensation
            let timestamp = (raw_timestamp as f64 / 1000.0) + (wrap_count as f64 * 65.536);

            if block_type == 0 {
                // Data record - calculate required bytes for all channels
                let mut required_bytes = 0;
                for channel in &channels {
                    if let Some(field_type) = FieldType::from_u8(channel.field_type) {
                        required_bytes += field_type.byte_size();
                    }
                }
                required_bytes += 1; // Add CRC byte

                // Check if we have enough data for this record BEFORE adding timestamp
                if offset + required_bytes > data.len() {
                    eprintln!(
                        "DEBUG: Not enough data for complete record at offset {} (need {}, have {})",
                        offset,
                        required_bytes,
                        data.len() - offset
                    );
                    break;
                }

                // Now it's safe to add the timestamp and read the record
                let mut record = Vec::new();

                for channel in &channels {
                    if let Some(field_type) = FieldType::from_u8(channel.field_type) {
                        let value = match field_type {
                            FieldType::U08 => {
                                let v = data[offset] as f64;
                                offset += 1;
                                // Formula: (value + transform) * scale
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::S08 => {
                                let v = data[offset] as i8 as f64;
                                offset += 1;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::U16 => {
                                let v = u16::from_be_bytes([data[offset], data[offset + 1]]) as f64;
                                offset += 2;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::S16 => {
                                let v = i16::from_be_bytes([data[offset], data[offset + 1]]) as f64;
                                offset += 2;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::U32 => {
                                let v = u32::from_be_bytes([
                                    data[offset],
                                    data[offset + 1],
                                    data[offset + 2],
                                    data[offset + 3],
                                ]) as f64;
                                offset += 4;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::S32 => {
                                let v = i32::from_be_bytes([
                                    data[offset],
                                    data[offset + 1],
                                    data[offset + 2],
                                    data[offset + 3],
                                ]) as f64;
                                offset += 4;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::F32 => {
                                let v = f32::from_be_bytes([
                                    data[offset],
                                    data[offset + 1],
                                    data[offset + 2],
                                    data[offset + 3],
                                ]) as f64;
                                offset += 4;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::S64 => {
                                let v = i64::from_be_bytes([
                                    data[offset],
                                    data[offset + 1],
                                    data[offset + 2],
                                    data[offset + 3],
                                    data[offset + 4],
                                    data[offset + 5],
                                    data[offset + 6],
                                    data[offset + 7],
                                ]) as f64;
                                offset += 8;
                                Value::Float((v + channel.transform as f64) * channel.scale as f64)
                            }
                            FieldType::U08Bitfield
                            | FieldType::U16Bitfield
                            | FieldType::U32Bitfield => {
                                offset += field_type.byte_size();
                                Value::Float(0.0) // Bitfields not fully supported yet
                            }
                        };
                        record.push(value);
                    } else {
                        return Err(format!("Unknown field type: {}", channel.field_type).into());
                    }
                }

                // Only add the timestamp and record together to ensure they stay in sync
                times.push(timestamp);
                data_records.push(record);

                // Skip CRC (1 byte)
                offset += 1;
            } else if block_type == 1 {
                // Marker record - skip marker message (50 bytes)
                if offset + 50 > data.len() {
                    eprintln!(
                        "DEBUG: Not enough data for marker block at offset {} (need 50, have {})",
                        offset,
                        data.len() - offset
                    );
                    break;
                }
                offset += 50;
            } else {
                eprintln!(
                    "DEBUG: Unknown block type {} at offset {}",
                    block_type,
                    offset - 3
                );
                break; // Unknown block type
            }
        }

        eprintln!("DEBUG: Parsed {} data records", data_records.len());
        eprintln!("DEBUG: Times vector length: {}", times.len());

        // Debug: Check if timestamps are monotonically increasing
        if times.len() > 1 {
            eprintln!("DEBUG: Timestamp analysis:");
            let mut non_monotonic_count = 0;
            let mut prev_time: f64 = 0.0;
            for (i, &t) in times.iter().enumerate() {
                if i > 0 && t < prev_time {
                    non_monotonic_count += 1;
                    if non_monotonic_count <= 5 {
                        eprintln!(
                            "  Non-monotonic at index {}: {} -> {} (delta: {:.3})",
                            i,
                            prev_time,
                            t,
                            t - prev_time
                        );
                    }
                }
                prev_time = t;
            }
            if non_monotonic_count > 0 {
                eprintln!("  Total non-monotonic jumps: {}", non_monotonic_count);
            } else {
                eprintln!("  All timestamps are monotonically increasing");
            }

            // Print first 10 and last 5 timestamps
            eprintln!("DEBUG: First 10 timestamps:");
            for (i, t) in times.iter().take(10).enumerate() {
                eprintln!("  [{}] {}", i, t);
            }
            if times.len() > 15 {
                eprintln!("DEBUG: Last 5 timestamps:");
                for (i, t) in times.iter().skip(times.len() - 5).enumerate() {
                    eprintln!("  [{}] {}", times.len() - 5 + i, t);
                }
            }
        }

        // Debug: Show first few records to verify data structure
        if !data_records.is_empty() {
            eprintln!("DEBUG: First record (time={}):", times[0]);
            for (idx, val) in data_records[0].iter().enumerate() {
                if idx < channels.len() {
                    eprintln!("  [{}] {} = {:.3}", idx, channels[idx].name, val.as_f64());
                }
            }
            if data_records.len() > 1 {
                eprintln!("DEBUG: Second record (time={}):", times[1]);
                for (idx, val) in data_records[1].iter().enumerate() {
                    if idx < channels.len() {
                        eprintln!("  [{}] {} = {:.3}", idx, channels[idx].name, val.as_f64());
                    }
                }
            }
        }

        // Validate that times and data match
        if times.len() != data_records.len() {
            return Err(format!(
                "Data integrity error: {} timestamps but {} data records",
                times.len(),
                data_records.len()
            )
            .into());
        }

        // Validate that all data records have the correct number of values
        let channel_count = channels.len();
        for (i, record) in data_records.iter().enumerate() {
            if record.len() != channel_count {
                return Err(format!(
                    "Data integrity error: record {} has {} values but {} channels expected",
                    i,
                    record.len(),
                    channel_count
                )
                .into());
            }
        }

        Ok(Log {
            meta: super::types::Meta::Speeduino(meta),
            channels: channels
                .into_iter()
                .map(super::types::Channel::Speeduino)
                .collect(),
            times,
            data: data_records,
        })
    }
}

impl Parseable for Speeduino {
    fn parse(&self, _data: &str) -> Result<Log, Box<dyn Error>> {
        // This method is for text-based parsing
        // Speeduino/rusEFI uses binary MLG format, so this will return an error
        Err("Speeduino/rusEFI MLG files are binary format. Use parse_binary() instead.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // FieldType Tests
    // ============================================

    #[test]
    fn test_field_type_from_u8() {
        assert!(matches!(FieldType::from_u8(0), Some(FieldType::U08)));
        assert!(matches!(FieldType::from_u8(1), Some(FieldType::S08)));
        assert!(matches!(FieldType::from_u8(2), Some(FieldType::U16)));
        assert!(matches!(FieldType::from_u8(3), Some(FieldType::S16)));
        assert!(matches!(FieldType::from_u8(4), Some(FieldType::U32)));
        assert!(matches!(FieldType::from_u8(5), Some(FieldType::S32)));
        assert!(matches!(FieldType::from_u8(6), Some(FieldType::S64)));
        assert!(matches!(FieldType::from_u8(7), Some(FieldType::F32)));
        assert!(matches!(
            FieldType::from_u8(10),
            Some(FieldType::U08Bitfield)
        ));
        assert!(matches!(
            FieldType::from_u8(11),
            Some(FieldType::U16Bitfield)
        ));
        assert!(matches!(
            FieldType::from_u8(12),
            Some(FieldType::U32Bitfield)
        ));
        // Invalid types
        assert!(FieldType::from_u8(8).is_none());
        assert!(FieldType::from_u8(9).is_none());
        assert!(FieldType::from_u8(13).is_none());
        assert!(FieldType::from_u8(255).is_none());
    }

    #[test]
    fn test_field_type_byte_size() {
        assert_eq!(FieldType::U08.byte_size(), 1);
        assert_eq!(FieldType::S08.byte_size(), 1);
        assert_eq!(FieldType::U08Bitfield.byte_size(), 1);
        assert_eq!(FieldType::U16.byte_size(), 2);
        assert_eq!(FieldType::S16.byte_size(), 2);
        assert_eq!(FieldType::U16Bitfield.byte_size(), 2);
        assert_eq!(FieldType::U32.byte_size(), 4);
        assert_eq!(FieldType::S32.byte_size(), 4);
        assert_eq!(FieldType::F32.byte_size(), 4);
        assert_eq!(FieldType::U32Bitfield.byte_size(), 4);
        assert_eq!(FieldType::S64.byte_size(), 8);
    }

    // ============================================
    // Detection Tests
    // ============================================

    #[test]
    fn test_detect_valid_mlg_header() {
        let valid_header = b"MLVLG\x00\x00\x01";
        assert!(Speeduino::detect(valid_header));
    }

    #[test]
    fn test_detect_invalid_header() {
        // Wrong magic bytes
        assert!(!Speeduino::detect(b"WRONG"));
        assert!(!Speeduino::detect(b"MLV"));
        assert!(!Speeduino::detect(b""));
        // Too short
        assert!(!Speeduino::detect(b"MLVL"));
        // Completely different format
        assert!(!Speeduino::detect(b"DataLog"));
        assert!(!Speeduino::detect(b"%DataLog%"));
    }

    #[test]
    fn test_detect_exact_header_length() {
        // Exactly 5 bytes with correct header should work
        assert!(Speeduino::detect(b"MLVLG"));
    }

    // ============================================
    // SpeeduinoChannel Tests
    // ============================================

    #[test]
    fn test_speeduino_channel_unit() {
        let channel = SpeeduinoChannel {
            name: "RPM".to_string(),
            unit: "rpm".to_string(),
            scale: 1.0,
            transform: 0.0,
            field_type: 2,
        };
        assert_eq!(channel.unit(), "rpm");
    }

    // ============================================
    // Binary Parsing Error Tests
    // ============================================

    #[test]
    fn test_parse_binary_invalid_header() {
        let invalid_data = b"NOT_MLG_FORMAT";
        let result = Speeduino::parse_binary(invalid_data);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid MLG file header"));
    }

    #[test]
    fn test_parse_binary_too_short() {
        // Valid header but truncated
        let short_data = b"MLVLG";
        let result = Speeduino::parse_binary(short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_binary_unreasonable_field_count() {
        // Create a minimal header with unreasonable field count
        let mut data = Vec::new();
        data.extend_from_slice(b"MLVLG\x00"); // Header (6 bytes)
        data.extend_from_slice(&1_i16.to_be_bytes()); // Format version
        data.extend_from_slice(&0_i32.to_be_bytes()); // Timestamp
        data.extend_from_slice(&100_u16.to_be_bytes()); // info_data_start
        data.extend_from_slice(&200_u32.to_be_bytes()); // data_begin_index
        data.extend_from_slice(&10_u16.to_be_bytes()); // record_length
        data.extend_from_slice(&5000_u16.to_be_bytes()); // num_fields (unreasonable)

        let result = Speeduino::parse_binary(&data);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unreasonable field count"));
    }

    // ============================================
    // Text Parser Error Test
    // ============================================

    #[test]
    fn test_text_parser_returns_error() {
        let parser = Speeduino;
        let result = parser.parse("some text data");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("binary format"));
    }

    // ============================================
    // Integration Test with Example File
    // ============================================

    #[test]
    fn test_parse_speeduino_example_file() {
        // Read the example Speeduino MLG file
        let file_path = "exampleLogs/speeduino/speeduino.mlg";
        let data = match std::fs::read(file_path) {
            Ok(d) => d,
            Err(_) => {
                // Skip test if example file is not available
                eprintln!("Skipping test: {} not found", file_path);
                return;
            }
        };

        // Verify detection
        assert!(Speeduino::detect(&data), "Should detect as MLG format");

        // Parse the file
        let log = Speeduino::parse_binary(&data).expect("Should parse successfully");

        // Verify basic structure
        assert!(!log.channels.is_empty(), "Should have channels");
        assert!(!log.times.is_empty(), "Should have timestamps");
        assert!(!log.data.is_empty(), "Should have data records");

        // Verify data integrity
        assert_eq!(
            log.times.len(),
            log.data.len(),
            "Times and data should have same length"
        );

        // Verify all records have correct channel count
        let channel_count = log.channels.len();
        for (i, record) in log.data.iter().enumerate() {
            assert_eq!(
                record.len(),
                channel_count,
                "Record {} should have {} values",
                i,
                channel_count
            );
        }

        // Verify timestamps are monotonically increasing (after wraparound handling)
        for window in log.times.windows(2) {
            assert!(
                window[1] >= window[0],
                "Timestamps should be monotonically increasing"
            );
        }

        // Print some debug info
        eprintln!("Parsed {} channels", log.channels.len());
        eprintln!("Parsed {} data records", log.data.len());
        if !log.times.is_empty() {
            eprintln!(
                "Time range: {:.3}s to {:.3}s",
                log.times[0],
                log.times[log.times.len() - 1]
            );
        }
    }

    #[test]
    fn test_parse_rusefi_example_file() {
        // Read the example rusEFI MLG file
        let file_path = "exampleLogs/rusefi/rusefilog.mlg";
        let data = match std::fs::read(file_path) {
            Ok(d) => d,
            Err(_) => {
                // Skip test if example file is not available
                eprintln!("Skipping test: {} not found", file_path);
                return;
            }
        };

        // Verify detection
        assert!(Speeduino::detect(&data), "Should detect as MLG format");

        // Parse the file
        let log = Speeduino::parse_binary(&data).expect("Should parse successfully");

        // Verify basic structure
        assert!(!log.channels.is_empty(), "Should have channels");
        assert!(!log.times.is_empty(), "Should have timestamps");
        assert!(!log.data.is_empty(), "Should have data records");

        // Verify data integrity
        assert_eq!(
            log.times.len(),
            log.data.len(),
            "Times and data should have same length"
        );

        // Verify channel names are parsed correctly (not empty/garbage)
        for channel in &log.channels {
            let name = channel.name();
            assert!(!name.is_empty(), "Channel name should not be empty");
            // Check that name contains printable characters
            assert!(
                name.chars().all(|c| c.is_ascii_graphic() || c == ' '),
                "Channel name should contain valid characters: {}",
                name
            );
        }

        eprintln!("Parsed {} channels from rusEFI log", log.channels.len());
        eprintln!("Parsed {} data records", log.data.len());
    }
}
