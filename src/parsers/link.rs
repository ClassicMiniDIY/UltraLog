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

    /// Iterate every well-formed block in the file.
    ///
    /// Link files are a sequence of blocks, each framed as:
    ///     <u32 LE total_size><3-byte ASCII marker><u8 type><content>
    ///
    /// `total_size` is the whole block (header + content). Fixed-size blocks
    /// (`lf3`, `ld2`, `lm1`) sit contiguously. `ds3` (channel definition)
    /// blocks are each followed by a variable-length `(time_f32, value_f32)`
    /// data region that has no framing of its own.
    ///
    /// To handle both cases uniformly, we advance to the next block by
    /// scanning forward from `pos + size` for the next `<u32 size><known
    /// marker>` prefix (see `find_next_block`). This works whether the
    /// previous block had a trailing data region or not.
    ///
    /// Returns `(block_offset, total_size, marker, type_byte)` for each block.
    fn iter_blocks(data: &[u8]) -> Vec<(usize, usize, [u8; 3], u8)> {
        let mut blocks = Vec::new();
        let mut pos: usize = 0;
        while let Some(block_off) = Self::find_next_block(data, pos) {
            let size = Self::read_u32(data, block_off) as usize;
            let marker = [
                data[block_off + 4],
                data[block_off + 5],
                data[block_off + 6],
            ];
            let type_byte = data[block_off + 7];
            blocks.push((block_off, size, marker, type_byte));
            pos = block_off + size;
        }
        blocks
    }

    /// Scan forward from `start` for the next position where a well-formed
    /// block header begins. A position `p` qualifies if:
    /// - `p + 8 <= data.len()` (room for size prefix + 3-byte marker + type byte)
    /// - the u32 LE at `p` is between 8 and `data.len() - p` (plausible total size)
    /// - the 3-byte marker at `p + 4` is one of the four known markers
    ///
    /// Returns the first qualifying offset, or `None` if scanning reaches EOF.
    fn find_next_block(data: &[u8], start: usize) -> Option<usize> {
        let mut p = start;
        while p + 8 <= data.len() {
            let size = Self::read_u32(data, p) as usize;
            if size >= 8 && size <= data.len() - p {
                let marker = [data[p + 4], data[p + 5], data[p + 6]];
                if matches!(&marker, b"lf3" | b"ld2" | b"lm1" | b"ds3") {
                    return Some(p);
                }
            }
            p += 1;
        }
        None
    }

    /// Locate channel ID + name + unit inside a `ds3` block's content bytes.
    ///
    /// Empirical layout observed across .llg, .llg5, and .llgx files:
    /// - Content is 637 bytes (block total_size 645 minus the 8-byte header).
    /// - Somewhere inside (typically around offset 0xd3) sits the channel ID
    ///   as a little-endian u32 whose upper two bytes are zero.
    /// - Immediately after the u32 is a UTF-16 LE name, null-terminated and
    ///   padded to 200 bytes total.
    /// - 200 bytes past the start of the ID is a UTF-16 LE unit string.
    ///
    /// Returns `None` if no plausible channel header is found.
    fn parse_ds3_channel(data: &[u8], block_offset: usize, block_size: usize) -> Option<LinkChannel> {
        let content_start = block_offset + 8;
        let content_end = block_offset + block_size;
        // Scan for: low-id u32 (two high bytes zero, low two nonzero) followed by
        // a printable UTF-16 LE character (ASCII byte + null).
        let scan_end = content_end.saturating_sub(8).min(content_start + 400);
        for p in content_start..scan_end {
            if p + 6 > data.len() {
                return None;
            }
            let b0 = data[p];
            let b1 = data[p + 1];
            let b2 = data[p + 2];
            let b3 = data[p + 3];
            let nb = data[p + 4];
            let nb1 = data[p + 5];
            if b2 == 0 && b3 == 0 && (b0 | b1) != 0
                && (0x20..=0x7e).contains(&nb) && nb1 == 0
            {
                let channel_id = Self::read_u32(data, p);
                // Sanity-check: channel IDs are small positive integers
                if !(1..10_000).contains(&channel_id) {
                    continue;
                }
                let name = Self::read_utf16_string(data, p + 4, 100);
                if name.len() < 2 {
                    continue;
                }
                // Unit lives 200 bytes past the start of the u32 id.
                let unit = Self::read_utf16_string(data, p + 204, 100);
                return Some(LinkChannel {
                    name,
                    unit,
                    channel_id,
                });
            }
        }
        None
    }

    /// Parse the LLG/LLG5/LLGX binary format.
    ///
    /// See `iter_blocks` for the block framing. This function walks the blocks,
    /// extracts metadata from the `ld2` block, collects channel definitions from
    /// each `ds3` block, and reads the `(time_f32_LE, value_f32_LE)` pair stream
    /// that lives between consecutive `ds3` blocks (or after the last `ds3` to EOF).
    pub fn parse_binary(data: &[u8]) -> Result<Log, Box<dyn Error>> {
        if !Self::detect(data) {
            return Err("Invalid LLG file header - expected 'lf3' magic".into());
        }

        let blocks = Self::iter_blocks(data);
        if blocks.is_empty() || &blocks[0].2 != b"lf3" {
            return Err("Missing or malformed lf3 header block".into());
        }

        // Metadata. Keys inside ld2 are at known offsets relative to file start
        // (ld2 starts right after lf3 at offset 215 for all observed files).
        let mut meta = LinkMeta::default();
        if data.len() > 0x1A00 {
            meta.ecu_model = Self::read_utf16_string(data, 0x336, 32);
            meta.log_date = Self::read_utf16_string(data, 0x1786, 16);
            meta.log_time = Self::read_utf16_string(data, 0x184e, 16);
            meta.software_version = Self::read_utf16_string(data, 0x1916, 20);
            meta.source = Self::read_utf16_string(data, 0x1aa6, 20);
        }

        // Collect ds3 blocks and their data-region boundaries.
        // Data region for ds3[i] is [blocks[i].0 + blocks[i].1, next_ds3_offset).
        // "next_ds3_offset" = start of the next ds3 block, or EOF if this is the
        // last ds3. (Other block markers don't appear between ds3 blocks in any
        // sample file observed across the three format subtypes.)
        let mut ds3_indices: Vec<usize> = Vec::new();
        for (i, &(_, _, marker, _)) in blocks.iter().enumerate() {
            if &marker == b"ds3" {
                ds3_indices.push(i);
            }
        }

        let mut channels: Vec<LinkChannel> = Vec::with_capacity(ds3_indices.len());
        let mut channel_samples: Vec<Vec<(f32, f32)>> = Vec::with_capacity(ds3_indices.len());

        for (k, &block_idx) in ds3_indices.iter().enumerate() {
            let (block_off, block_size, _, _) = blocks[block_idx];
            let Some(channel) = Self::parse_ds3_channel(data, block_off, block_size) else {
                continue;
            };

            let data_start = block_off + block_size;
            let data_end = if k + 1 < ds3_indices.len() {
                let next_block_idx = ds3_indices[k + 1];
                blocks[next_block_idx].0
            } else {
                data.len()
            };

            let mut points: Vec<(f32, f32)> = Vec::new();
            if data_end > data_start {
                let pair_count = (data_end - data_start) / 8;
                points.reserve(pair_count);
                let mut pos = data_start;
                for _ in 0..pair_count {
                    let time = Self::read_f32(data, pos);
                    let value = Self::read_f32(data, pos + 4);
                    // Only drop pairs that are non-finite. Negative times are
                    // legal: Link supports pre-trigger logging where the trigger
                    // event is t=0 and earlier samples have t<0.
                    if time.is_finite() && value.is_finite() {
                        points.push((time, value));
                    }
                    pos += 8;
                }
                points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }

            channels.push(channel);
            channel_samples.push(points);
        }

        // Build the common timeline: all distinct timestamps, sorted.
        let mut all_times: Vec<f32> = Vec::new();
        for points in &channel_samples {
            for &(t, _) in points {
                all_times.push(t);
            }
        }
        all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_times.dedup();

        if all_times.is_empty() {
            return Ok(Log {
                meta: Meta::Link(meta),
                channels: channels.into_iter().map(Channel::Link).collect(),
                times: Vec::new(),
                data: Vec::new(),
            });
        }

        // Normalize to a f64 seconds axis starting at 0.
        let first_time = *all_times.first().unwrap();
        let times: Vec<f64> = all_times.iter().map(|t| (*t - first_time) as f64).collect();

        // Build the data matrix with last-observation-carried-forward semantics.
        let row_cap = channels.len();
        let mut data_matrix: Vec<Vec<Value>> = Vec::with_capacity(all_times.len());
        for (time_idx, &time) in all_times.iter().enumerate() {
            let mut row: Vec<Value> = Vec::with_capacity(row_cap);
            for points in &channel_samples {
                let value = if points.is_empty() {
                    0.0
                } else {
                    match points.binary_search_by(|probe| {
                        probe.0.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        Ok(idx) => points[idx].1,
                        Err(idx) => {
                            if idx == 0 {
                                points[0].1
                            } else {
                                points[idx - 1].1
                            }
                        }
                    }
                };
                row.push(Value::Float(value as f64));
            }
            data_matrix.push(row);
            if time_idx >= 50_000 {
                tracing::warn!("Truncating Link log timeline at 50000 samples");
                break;
            }
        }

        tracing::info!(
            "Parsed Link log: {} channels, {} timeline samples, ECU: {}",
            channels.len(),
            data_matrix.len(),
            meta.ecu_model
        );

        let times_out = times[..data_matrix.len()].to_vec();
        Ok(Log {
            meta: Meta::Link(meta),
            channels: channels.into_iter().map(Channel::Link).collect(),
            times: times_out,
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
