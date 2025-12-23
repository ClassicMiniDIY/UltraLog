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
        let mut times = Vec::new();
        let mut data_records: Vec<Vec<Value>> = Vec::new();

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
                times.push(format!("{:.3}", timestamp));
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
            for (i, time_str) in times.iter().enumerate() {
                if let Ok(t) = time_str.parse::<f64>() {
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
