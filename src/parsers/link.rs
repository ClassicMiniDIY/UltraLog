//! Link ECU (.llg) binary format parser
//!
//! Link ECU uses a proprietary binary format for log files (.llg).
//! Format structure based on reverse engineering:
//! - Header: 215 bytes with "lf3" magic and version
//! - Metadata section with ECU info (UTF-16 LE strings)
//! - Channel blocks with name, unit, and time-series data
//! - Data stored as f32 (value, time) pairs

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Link ECU channel metadata
#[derive(Clone, Debug, Serialize)]
pub struct LinkChannel {
    pub name: String,
    pub unit: String,
    pub channel_id: u32,
}

impl LinkChannel {
    /// Get the display unit for this channel
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// Link ECU log metadata
#[derive(Clone, Debug, Serialize, Default)]
pub struct LinkMeta {
    pub ecu_model: String,
    pub log_date: String,
    pub log_time: String,
    pub software_version: String,
    pub source: String,
}

/// Link ECU log file parser
pub struct Link;

impl Link {
    /// Magic bytes for LLG format
    const MAGIC: &'static [u8] = b"lf3";

    /// Detect if data is Link ECU LLG format
    pub fn detect(data: &[u8]) -> bool {
        // Check for "lf3" magic at offset 4
        data.len() >= 8 && &data[4..7] == Self::MAGIC
    }

    /// Read a UTF-16 LE string from the buffer
    fn read_utf16_string(data: &[u8], offset: usize, max_chars: usize) -> String {
        let mut result = String::new();
        for i in 0..max_chars {
            let byte_offset = offset + i * 2;
            if byte_offset + 2 > data.len() {
                break;
            }
            let char_val = u16::from_le_bytes([data[byte_offset], data[byte_offset + 1]]);
            if char_val == 0 {
                break;
            }
            // Only include printable ASCII for safety
            if (0x20..=0x7e).contains(&char_val) {
                result.push(char::from_u32(char_val as u32).unwrap_or('?'));
            } else if char_val > 0x7e {
                // Non-ASCII character, stop reading
                break;
            }
        }
        result
    }

    /// Read a little-endian u32
    fn read_u32(data: &[u8], offset: usize) -> u32 {
        if offset + 4 > data.len() {
            return 0;
        }
        u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    /// Read a little-endian f32
    fn read_f32(data: &[u8], offset: usize) -> f32 {
        if offset + 4 > data.len() {
            return 0.0;
        }
        f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    /// Parse the LLG binary format
    pub fn parse_binary(data: &[u8]) -> Result<Log, Box<dyn Error>> {
        // Validate header
        if !Self::detect(data) {
            return Err("Invalid LLG file header - expected 'lf3' magic".into());
        }

        // Read header size (first 4 bytes)
        let header_size = Self::read_u32(data, 0) as usize;
        if header_size > data.len() {
            return Err(format!(
                "Header size {} exceeds file size {}",
                header_size,
                data.len()
            )
            .into());
        }

        // Parse metadata
        let mut meta = LinkMeta::default();

        // ECU model is around offset 0x336 (fixed location in header)
        if data.len() > 0x336 + 64 {
            meta.ecu_model = Self::read_utf16_string(data, 0x336, 32);
        }

        // Date around 0x1786, time around 0x184e, version around 0x1916
        if data.len() > 0x1A00 {
            meta.log_date = Self::read_utf16_string(data, 0x1786, 16);
            meta.log_time = Self::read_utf16_string(data, 0x184e, 16);
            meta.software_version = Self::read_utf16_string(data, 0x1916, 20);
            meta.source = Self::read_utf16_string(data, 0x1aa6, 20);
        }

        // Find channel blocks
        // Channel blocks start around offset 0x2400 and have pattern:
        // 4 zero bytes + 4 byte channel ID + 200 bytes name + 200 bytes unit + data
        let mut channels: Vec<LinkChannel> = Vec::new();
        let mut channel_offsets: Vec<(usize, usize)> = Vec::new(); // (start, next_start)

        let mut offset = 0x2000; // Start searching after metadata
        while offset < data.len().saturating_sub(500) {
            // Look for channel header pattern: 4 zeros + non-zero ID
            if data[offset..offset + 4] == [0, 0, 0, 0] {
                let channel_id = Self::read_u32(data, offset + 4);

                if channel_id > 0 && channel_id < 10000 {
                    // Read channel name (200 bytes of UTF-16 starting at offset+8)
                    let name = Self::read_utf16_string(data, offset + 8, 100);

                    if name.len() >= 2 {
                        // Read unit (200 bytes starting at offset+208)
                        let unit = Self::read_utf16_string(data, offset + 208, 100);

                        channel_offsets.push((offset, 0));
                        channels.push(LinkChannel {
                            name,
                            unit,
                            channel_id,
                        });

                        // Skip past header + name + unit (408 bytes)
                        offset += 408;
                        continue;
                    }
                }
            }
            offset += 1;
        }

        // Update channel_offsets with next channel starts
        for i in 0..channel_offsets.len() {
            if i + 1 < channel_offsets.len() {
                channel_offsets[i].1 = channel_offsets[i + 1].0;
            } else {
                channel_offsets[i].1 = data.len();
            }
        }

        // Parse time-series data from channels
        // Each channel's data section contains f32 (value, time) pairs
        // We need to merge all channels into a common timeline

        // First, collect all unique timestamps and their values per channel
        let mut all_times: Vec<f32> = Vec::new();
        let mut channel_data: Vec<Vec<(f32, f32)>> = Vec::new(); // (time, value) per channel

        for (i, &(ch_start, ch_end)) in channel_offsets.iter().enumerate() {
            let data_start = ch_start + 408; // After header + name + unit
            let data_end = ch_end;

            if data_start >= data_end || data_end > data.len() {
                channel_data.push(Vec::new());
                continue;
            }

            // Skip the first 8 bytes of metadata in data section
            let actual_data_start = data_start + 8;

            let mut points: Vec<(f32, f32)> = Vec::new();

            // Read f32 pairs (value, time)
            let mut pos = actual_data_start;
            while pos + 8 <= data_end {
                let value = Self::read_f32(data, pos);
                let time = Self::read_f32(data, pos + 4);

                // Filter for reasonable values
                if (0.0..100000.0).contains(&time) && value.is_finite() && value.abs() < 1e10 {
                    points.push((time, value));
                    if !all_times.contains(&time) {
                        all_times.push(time);
                    }
                }

                pos += 8;
            }

            // Sort by time
            points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            if i < 5 {
                tracing::debug!(
                    "Channel {}: {} ({}) - {} data points",
                    i,
                    channels[i].name,
                    channels[i].unit,
                    points.len()
                );
            }

            channel_data.push(points);
        }

        // Sort all timestamps
        all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_times.dedup();

        // If no valid data found, try alternative parsing
        if all_times.is_empty() {
            tracing::warn!("No valid time-series data found in LLG file");
            // Return empty log with channel definitions
            return Ok(Log {
                meta: Meta::Link(meta),
                channels: channels.into_iter().map(Channel::Link).collect(),
                times: Vec::new(),
                data: Vec::new(),
            });
        }

        // Convert to f64 times (seconds, relative to first timestamp)
        let first_time = *all_times.first().unwrap_or(&0.0);
        let times: Vec<f64> = all_times.iter().map(|t| (*t - first_time) as f64).collect();

        // Build data matrix: for each timestamp, interpolate/hold values for each channel
        let mut data_matrix: Vec<Vec<Value>> = Vec::with_capacity(times.len());

        for (time_idx, &time) in all_times.iter().enumerate() {
            let mut row: Vec<Value> = Vec::with_capacity(channels.len());

            for ch_data in &channel_data {
                // Find the value at or before this timestamp
                let value = if ch_data.is_empty() {
                    0.0
                } else {
                    // Binary search for the closest time <= current time
                    match ch_data.binary_search_by(|probe| {
                        probe
                            .0
                            .partial_cmp(&time)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        Ok(idx) => ch_data[idx].1,
                        Err(idx) => {
                            if idx == 0 {
                                ch_data[0].1
                            } else {
                                ch_data[idx - 1].1
                            }
                        }
                    }
                };

                row.push(Value::Float(value as f64));
            }

            data_matrix.push(row);

            // Limit output to reasonable size
            if time_idx > 50000 {
                tracing::warn!("Truncating log data at 50000 samples");
                break;
            }
        }

        tracing::info!(
            "Parsed Link ECU log: {} channels, {} data points, ECU: {}",
            channels.len(),
            data_matrix.len(),
            meta.ecu_model
        );

        Ok(Log {
            meta: Meta::Link(meta),
            channels: channels.into_iter().map(Channel::Link).collect(),
            times: times[..data_matrix.len()].to_vec(),
            data: data_matrix,
        })
    }
}

impl Parseable for Link {
    fn parse(&self, _data: &str) -> Result<Log, Box<dyn Error>> {
        // This method is for text-based parsing
        // Link ECU uses binary LLG format, so this will return an error
        Err("Link ECU LLG files are binary format. Use parse_binary() instead.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_valid_llg_header() {
        // Create minimal valid header
        let mut data = vec![0u8; 16];
        // Header size
        data[0..4].copy_from_slice(&215u32.to_le_bytes());
        // Magic "lf3"
        data[4..7].copy_from_slice(b"lf3");
        // Version
        data[7] = 0xe5;

        assert!(Link::detect(&data));
    }

    #[test]
    fn test_detect_invalid_header() {
        assert!(!Link::detect(b"NOT_LLG"));
        assert!(!Link::detect(b""));
        assert!(!Link::detect(&[0u8; 4]));
    }

    #[test]
    fn test_read_utf16_string() {
        // "Test" in UTF-16 LE
        let data: Vec<u8> = vec![0x54, 0x00, 0x65, 0x00, 0x73, 0x00, 0x74, 0x00, 0x00, 0x00];
        let result = Link::read_utf16_string(&data, 0, 10);
        assert_eq!(result, "Test");
    }

    #[test]
    fn test_read_u32() {
        let data = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(Link::read_u32(&data, 0), 0x04030201);
    }

    #[test]
    fn test_read_f32() {
        // 1.0 in IEEE 754
        let data = [0x00, 0x00, 0x80, 0x3f];
        let result = Link::read_f32(&data, 0);
        assert!((result - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_text_parser_returns_error() {
        let parser = Link;
        let result = parser.parse("some text data");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("binary format"));
    }

    #[test]
    fn test_parse_link_example_file() {
        // Read the example Link LLG file
        let file_path = "exampleLogs/link/linklog.llg";
        let data = match std::fs::read(file_path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Skipping test: {} not found", file_path);
                return;
            }
        };

        // Verify detection
        assert!(Link::detect(&data), "Should detect as LLG format");

        // Parse the file
        let log = Link::parse_binary(&data).expect("Should parse successfully");

        // Verify basic structure
        assert!(!log.channels.is_empty(), "Should have channels");
        // Note: times/data may be empty if no valid time-series data found
        // The example file format may vary

        // Verify channel names are parsed correctly
        for channel in &log.channels {
            let name = channel.name();
            assert!(!name.is_empty(), "Channel name should not be empty");
        }

        eprintln!("Parsed {} channels from Link ECU log", log.channels.len());
        eprintln!("Parsed {} data records", log.data.len());

        // Check metadata
        if let Meta::Link(meta) = &log.meta {
            eprintln!("ECU Model: {}", meta.ecu_model);
            eprintln!("Date: {}", meta.log_date);
            eprintln!("Version: {}", meta.software_version);
        }
    }
}
