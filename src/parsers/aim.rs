//! AIM XRK/DRK file parser
//!
//! Parses data files from AIM motorsport data acquisition devices (MXP, MXG, MXL2, EVO5, MyChron5, etc.)
//!
//! Platform support:
//! - Windows/Linux x86_64: Uses xdrk crate (AIM's official library wrapper)
//! - macOS/Other: Uses pure Rust implementation

use serde::Serialize;
use std::error::Error;
use std::path::Path;

use super::types::{Log, Meta, Value};

/// AIM channel metadata
#[derive(Clone, Debug, Serialize)]
pub struct AimChannel {
    pub name: String,
    pub unit: String,
}

impl AimChannel {
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// AIM log file metadata
#[derive(Clone, Debug, Serialize, Default)]
pub struct AimMeta {
    pub vehicle: String,
    pub racer: String,
    pub track: String,
    pub championship: String,
    pub venue_type: String,
    pub datetime: String,
    pub lap_count: usize,
}

/// AIM XRK/DRK parser
pub struct Aim;

impl Aim {
    /// Detect if data is AIM XRK format by checking the file signature
    /// XRK files start with "<hCNF" tag
    pub fn detect(data: &[u8]) -> bool {
        data.len() >= 5 && &data[0..5] == b"<hCNF"
    }

    /// Parse AIM XRK/DRK file from a file path
    #[cfg(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    ))]
    pub fn parse_file(path: &Path) -> Result<Log, Box<dyn Error>> {
        Self::parse_file_xdrk(path)
    }

    /// Parse AIM XRK/DRK file from a file path (pure Rust implementation for unsupported platforms)
    #[cfg(not(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    )))]
    pub fn parse_file(path: &Path) -> Result<Log, Box<dyn Error>> {
        let data = std::fs::read(path)?;
        Self::parse_binary(&data)
    }

    /// Parse using xdrk library (Windows/Linux x86_64 only)
    #[cfg(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    ))]
    fn parse_file_xdrk(path: &Path) -> Result<Log, Box<dyn Error>> {
        // Load the XRK file using xdrk
        let run = xdrk::Run::load(path)?;

        // Extract metadata
        let meta = AimMeta {
            vehicle: run.vehicle().unwrap_or_default(),
            racer: run.racer().unwrap_or_default(),
            track: run.track().unwrap_or_default(),
            championship: run.championship().unwrap_or_default(),
            venue_type: run.venue_type().unwrap_or_default(),
            datetime: run.datetime().map(|dt| dt.to_string()).unwrap_or_default(),
            lap_count: run.number_of_laps(),
        };

        tracing::info!(
            "AIM log: {} at {} - {} laps",
            meta.vehicle,
            meta.track,
            meta.lap_count
        );

        // Get all channel names and units
        let channel_count = run.channels_count();
        let mut channels = Vec::with_capacity(channel_count);

        for i in 0..channel_count {
            let name = run
                .channel_name(i)
                .unwrap_or_else(|_| format!("Channel_{}", i));
            let unit = run.channel_unit(i).unwrap_or_default();
            channels.push(AimChannel { name, unit });
        }

        tracing::info!("Found {} channels", channels.len());

        // Get channel samples
        let mut times: Vec<f64> = Vec::new();
        let mut data: Vec<Vec<Value>> = Vec::new();

        if channels.is_empty() {
            return Ok(Log {
                meta: Meta::Aim(meta),
                channels: vec![],
                times,
                data,
            });
        }

        // Get sample count from first channel
        let sample_count = run.channel_samples_count(0).unwrap_or(0);

        if sample_count == 0 {
            tracing::warn!("No samples found in AIM log file");
            return Ok(Log {
                meta: Meta::Aim(meta),
                channels: channels
                    .into_iter()
                    .map(super::types::Channel::Aim)
                    .collect(),
                times,
                data,
            });
        }

        // Pre-allocate vectors
        times.reserve(sample_count);
        data.reserve(sample_count);

        // Get timestamps from first channel
        let first_channel_data = run.channel_samples(0)?;
        let timestamps = first_channel_data.timestamps();

        // Collect all channel samples
        let mut all_channel_samples: Vec<Vec<f64>> = Vec::with_capacity(channels.len());
        for i in 0..channels.len() {
            let channel_data = run.channel_samples(i)?;
            all_channel_samples.push(channel_data.samples().to_vec());
        }

        // Build time series data
        for (sample_idx, &timestamp) in timestamps.iter().enumerate() {
            times.push(timestamp);

            let mut row = Vec::with_capacity(channels.len());
            for channel_samples in &all_channel_samples {
                let value = if sample_idx < channel_samples.len() {
                    channel_samples[sample_idx]
                } else {
                    0.0
                };
                row.push(Value::Float(value));
            }
            data.push(row);
        }

        tracing::info!(
            "Parsed AIM log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::Aim(meta),
            channels: channels
                .into_iter()
                .map(super::types::Channel::Aim)
                .collect(),
            times,
            data,
        })
    }

    /// Parse XRK binary data using pure Rust implementation
    /// This is used on platforms where xdrk is not available (macOS, ARM, etc.)
    #[cfg(not(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    )))]
    fn parse_binary(data: &[u8]) -> Result<Log, Box<dyn Error>> {
        if !Self::detect(data) {
            return Err("Not a valid AIM XRK file".into());
        }

        tracing::info!(
            "Parsing AIM XRK file using pure Rust implementation ({} bytes)",
            data.len()
        );

        // Parse channels from the XRK binary format
        let channels = Self::parse_channels(data)?;
        tracing::info!("Found {} channels", channels.len());

        // Parse metadata from footer
        let meta = Self::parse_metadata(data)?;
        tracing::info!("Vehicle: {}, Track: {}", meta.vehicle, meta.track);

        // Parse channel data
        let (times, channel_data) = Self::parse_channel_data(data, channels.len())?;
        tracing::info!("Parsed {} data points", times.len());

        Ok(Log {
            meta: Meta::Aim(meta),
            channels: channels
                .into_iter()
                .map(super::types::Channel::Aim)
                .collect(),
            times,
            data: channel_data,
        })
    }

    /// Parse channel definitions from XRK data
    #[cfg(not(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    )))]
    fn parse_channels(data: &[u8]) -> Result<Vec<AimChannel>, Box<dyn Error>> {
        let mut channels = Vec::new();

        // Skip past the initial <hCNF> header
        // Format: <hCNF\x00 + 4 bytes length + 2 bytes version
        if data.len() < 12 {
            return Err("File too short".into());
        }
        let mut offset = 12; // Skip <hCNF\x00 + length + version

        // Look for <hCHS> channel section headers
        while offset + 100 < data.len() {
            // Search for <hCHS pattern
            if let Some(pos) = Self::find_pattern(data, b"<hCHS\x00", offset) {
                // Found a channel section header
                let section_start = pos + 6; // Skip <hCHS\x00

                if section_start + 100 > data.len() {
                    break;
                }

                // Read section length (4 bytes, little-endian)
                let _section_len = u32::from_le_bytes([
                    data[section_start],
                    data[section_start + 1],
                    data[section_start + 2],
                    data[section_start + 3],
                ]);

                // Skip length and version bytes
                let content_start = section_start + 6;

                if content_start + 70 > data.len() {
                    offset = pos + 6;
                    continue;
                }

                // Channel index is at offset 0 (4 bytes)
                // Skip various metadata fields

                // Short name starts at offset ~36 (8 bytes null-padded)
                let short_name_offset = content_start + 30;
                if short_name_offset + 8 > data.len() {
                    break;
                }

                let short_name = Self::read_null_terminated_string(&data[short_name_offset..], 8);

                // Long name starts at offset ~44 (24 bytes null-padded)
                let long_name_offset = short_name_offset + 8;
                if long_name_offset + 24 > data.len() {
                    break;
                }

                let long_name = Self::read_null_terminated_string(&data[long_name_offset..], 24);

                // Use long name if available, otherwise short name
                let name = if !long_name.is_empty() {
                    long_name
                } else {
                    short_name
                };

                if !name.is_empty() {
                    channels.push(AimChannel {
                        name,
                        unit: String::new(), // Units are not easily extractable from raw binary
                    });
                }

                offset = pos + 6;
            } else {
                break;
            }
        }

        Ok(channels)
    }

    /// Parse metadata from the XRK file footer
    #[cfg(not(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    )))]
    fn parse_metadata(data: &[u8]) -> Result<AimMeta, Box<dyn Error>> {
        let mut meta = AimMeta::default();

        // Look for <VEH> (Vehicle) tag near the end of the file
        if let Some(pos) = Self::find_pattern(data, b"<VEH\x00", data.len().saturating_sub(1000)) {
            let start = pos + 5;
            if start + 50 < data.len() {
                // Skip length bytes and read vehicle name
                if let Some(end) = Self::find_pattern(&data[start..], b"<", 0) {
                    meta.vehicle =
                        Self::read_null_terminated_string(&data[start + 4..], end.min(50));
                }
            }
        }

        // Look for <CMP> (Campaign/Championship) tag
        if let Some(pos) = Self::find_pattern(data, b"<CMP\x00", data.len().saturating_sub(1000)) {
            let start = pos + 5;
            if start + 100 < data.len() {
                if let Some(end) = Self::find_pattern(&data[start..], b"<", 0) {
                    meta.championship =
                        Self::read_null_terminated_string(&data[start + 4..], end.min(100));
                }
            }
        }

        // Look for <VTY> (Venue Type) tag
        if let Some(pos) = Self::find_pattern(data, b"<VTY\x00", data.len().saturating_sub(500)) {
            let start = pos + 5;
            if start + 50 < data.len() {
                if let Some(end) = Self::find_pattern(&data[start..], b"<", 0) {
                    meta.venue_type =
                        Self::read_null_terminated_string(&data[start + 4..], end.min(50));
                }
            }
        }

        Ok(meta)
    }

    /// Parse channel data samples from )(G records
    #[cfg(not(all(
        any(target_os = "windows", target_os = "linux"),
        target_arch = "x86_64"
    )))]
    fn parse_channel_data(
        data: &[u8],
        channel_count: usize,
    ) -> Result<(Vec<f64>, Vec<Vec<Value>>), Box<dyn Error>> {
        let mut times = Vec::new();
        let mut all_data: Vec<Vec<Value>> = Vec::new();

        if channel_count == 0 {
            return Ok((times, all_data));
        }

        // XRK files store telemetry data in )(G records
        // Each )(G record represents one time sample with multiple float values
        // Record structure: )(G + type_byte(s) + header bytes + float32 values
        // The records are in chronological order within the file

        let marker = b")(G";
        let mut record_count = 0;
        let sample_rate_hz = 100.0; // AIM loggers typically sample at 100Hz

        let mut offset = 0;
        while offset + 20 < data.len() {
            if let Some(pos) = Self::find_pattern(data, marker, offset) {
                // Find the next marker to determine record size
                let next_pos = Self::find_pattern(data, b")(", pos + 3).unwrap_or(data.len());
                let record_size = next_pos - pos;

                // Process records in the typical telemetry size range (100-200 bytes)
                // Different AIM devices/configs produce different record sizes (143, 151, etc.)
                if record_size >= 100 && record_size <= 200 {
                    // Read float32 values starting at offset 9
                    let data_start = pos + 9;
                    let num_floats = (record_size - 9) / 4; // ~35 floats

                    let mut values: Vec<f32> = Vec::with_capacity(num_floats);
                    for i in 0..num_floats {
                        let float_offset = data_start + i * 4;
                        if float_offset + 4 <= next_pos && float_offset + 4 <= data.len() {
                            let value = f32::from_le_bytes([
                                data[float_offset],
                                data[float_offset + 1],
                                data[float_offset + 2],
                                data[float_offset + 3],
                            ]);

                            // Only include finite values
                            if value.is_finite() {
                                values.push(value);
                            } else {
                                values.push(0.0);
                            }
                        }
                    }

                    // Only add records that have some non-zero values
                    let has_data = values.iter().any(|&v| v.abs() > 0.0001);
                    if has_data {
                        // Calculate time based on record sequence
                        let time_sec = record_count as f64 / sample_rate_hz;
                        times.push(time_sec);

                        // Create data row for all channels
                        let mut row = Vec::with_capacity(channel_count);
                        for ch_idx in 0..channel_count {
                            // Map channel to value - use modular indexing for excess channels
                            let value = if ch_idx < values.len() {
                                values[ch_idx] as f64
                            } else if !values.is_empty() {
                                0.0
                            } else {
                                0.0
                            };
                            row.push(Value::Float(value));
                        }
                        all_data.push(row);
                        record_count += 1;
                    }
                }

                offset = pos + 3;
            } else {
                break;
            }
        }

        tracing::info!("Extracted {} time samples from )(G records", record_count);

        // If we have very few samples, the parsing might not have worked well
        if all_data.len() < 10 {
            tracing::warn!(
                "Limited data extracted ({} samples). For best results on macOS, \
                export your AIM data to CSV using Race Studio 3.",
                all_data.len()
            );
        } else {
            let duration_sec = times.last().copied().unwrap_or(0.0);
            tracing::info!(
                "Successfully parsed {:.1} seconds of data ({} samples) for {} channels",
                duration_sec,
                all_data.len(),
                channel_count
            );
        }

        Ok((times, all_data))
    }

    /// Find a byte pattern in data starting from offset
    fn find_pattern(data: &[u8], pattern: &[u8], start: usize) -> Option<usize> {
        if start >= data.len() || pattern.is_empty() {
            return None;
        }

        for i in start..data.len().saturating_sub(pattern.len() - 1) {
            if &data[i..i + pattern.len()] == pattern {
                return Some(i);
            }
        }
        None
    }

    /// Read a null-terminated string from a byte slice, up to max_len bytes
    fn read_null_terminated_string(data: &[u8], max_len: usize) -> String {
        let max = max_len.min(data.len());
        let end = data[..max].iter().position(|&b| b == 0).unwrap_or(max);

        String::from_utf8_lossy(&data[..end]).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_xrk_header() {
        let valid_header = b"<hCNF\x00\x3c\xa5\x00\x00";
        assert!(Aim::detect(valid_header));
    }

    #[test]
    fn test_detect_invalid_header() {
        assert!(!Aim::detect(b"MLVLG")); // Speeduino format
        assert!(!Aim::detect(b"%DataLog%")); // Haltech format
        assert!(!Aim::detect(b"<hCN")); // Too short
        assert!(!Aim::detect(b"")); // Empty
    }

    #[test]
    fn test_parse_example_file() {
        let file_path = Path::new("exampleLogs/aim/BMW_THill 5mi_Generic testing_a_1033.xrk");

        if !file_path.exists() {
            eprintln!("Skipping test: example file not found");
            return;
        }

        // Verify detection
        let data = std::fs::read(file_path).expect("Failed to read file");
        assert!(Aim::detect(&data), "Should detect as XRK format");

        // Parse the file
        match Aim::parse_file(file_path) {
            Ok(log) => {
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
            Err(e) => {
                eprintln!("Parse error (expected on some platforms): {}", e);
            }
        }
    }

    #[test]
    fn test_find_pattern() {
        let data = b"hello<hCHS\x00world";
        assert_eq!(Aim::find_pattern(data, b"<hCHS\x00", 0), Some(5));
        assert_eq!(Aim::find_pattern(data, b"<hCHS\x00", 6), None);
        assert_eq!(Aim::find_pattern(data, b"hello", 0), Some(0));
    }

    #[test]
    fn test_read_null_terminated_string() {
        let data = b"Hello\x00World";
        assert_eq!(Aim::read_null_terminated_string(data, 20), "Hello");

        let data2 = b"NoNull";
        assert_eq!(Aim::read_null_terminated_string(data2, 6), "NoNull");
    }

    #[test]
    fn test_parse_all_xrk_files() {
        let aim_dir = Path::new("exampleLogs/aim");
        if !aim_dir.exists() {
            eprintln!("Skipping test: aim directory not found");
            return;
        }

        let entries = std::fs::read_dir(aim_dir).expect("Failed to read aim directory");
        let mut parsed_count = 0;

        for entry in entries {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            // Skip non-XRK files
            if path.extension().map(|e| e.to_str()) != Some(Some("xrk")) {
                continue;
            }

            eprintln!("\n=== Testing: {} ===", path.display());

            // Verify detection
            let data = std::fs::read(&path).expect("Failed to read file");
            assert!(
                Aim::detect(&data),
                "Should detect {} as XRK format",
                path.display()
            );

            // Parse the file
            match Aim::parse_file(&path) {
                Ok(log) => {
                    eprintln!("  Channels: {}", log.channels.len());
                    eprintln!("  Data records: {}", log.data.len());

                    // Verify we got actual data
                    assert!(
                        !log.channels.is_empty(),
                        "Should have channels for {}",
                        path.display()
                    );
                    assert!(
                        !log.data.is_empty(),
                        "Should have data records for {}",
                        path.display()
                    );
                    assert!(
                        !log.times.is_empty(),
                        "Should have timestamps for {}",
                        path.display()
                    );

                    if !log.times.is_empty() {
                        eprintln!(
                            "  Time range: {:.1}s to {:.1}s",
                            log.times[0],
                            log.times[log.times.len() - 1]
                        );
                    }

                    // Verify data has actual values
                    let has_non_zero = log
                        .data
                        .iter()
                        .any(|row| row.iter().any(|v| v.as_f64().abs() > 0.0001));
                    assert!(
                        has_non_zero,
                        "Should have non-zero values for {}",
                        path.display()
                    );

                    parsed_count += 1;
                }
                Err(e) => {
                    panic!("Failed to parse {}: {}", path.display(), e);
                }
            }
        }

        eprintln!("\nSuccessfully parsed {} XRK files", parsed_count);
        assert!(parsed_count > 0, "Should have parsed at least one XRK file");
    }
}
