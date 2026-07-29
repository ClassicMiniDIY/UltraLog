//! Injects fake GPS coordinates into a MegaLogViewer (.mlg) file.
//!
//! Reads an existing MLG (Speeduino/rusEFI/MegaSquirt) log, appends two new
//! S32 fields (`GPS Latitude`, `GPS Longitude`, scale 1e-7 deg) to the field
//! definitions, and synthesizes coordinates for every data record so the
//! resulting log traces a closed-loop track. The names match `source_names`
//! already declared in the AiM OECUA adapter spec, so UltraLog's Track Map
//! widget detects them via the global canonical-id lookup without any
//! parser-side change.
//!
//! Field data layout in the synthesized record mirrors what the canlogger
//! firmware emits in demo mode (see `firmware/Lib/demo_gen.c` and
//! `firmware/test/demo_config.ini` in the canlogger repo).
//!
//! Usage:
//!     cargo run --example inject_fake_gps_mlg -- <input.mlg> [output.mlg]

use std::env;
use std::fs;
use std::process::ExitCode;

const FIELD_LEN_V2: usize = 89;
const FIELD_LEN_V1: usize = 55;
const HEADER_LEN_V2: usize = 24;
const HEADER_LEN_V1: usize = 22;

const FT_S32: u8 = 5;

// Closed-loop trace approximating the Kartodrom "Zmeinka" kart circuit
// (Vladivostok). An outer egg-shaped loop with an inner switchback so the
// shape reads as a real track on the OSM tile, not a perfect oval.
const CENTER_LAT: f64 = 43.081965;
const CENTER_LON: f64 = 131.905281;
const LAP_PERIOD_S: f64 = 35.0;
const M_PER_DEG_LAT: f64 = 111_320.0;

/// Lap waypoints in `(east_m, north_m)` relative to `CENTER_*`, walked in
/// order, looping back to the first point. Roughly traced by eye from the
/// satellite outline; each leg is linearly interpolated so a constant
/// `LAP_PERIOD_S` lap time yields constant ground speed regardless of leg
/// length.
const WAYPOINTS: &[(f64, f64)] = &[
    (-110.0, -55.0), // 0  start/finish (near Kartodrom "Zmeinka" marker)
    (-95.0, -25.0),  // 1  turn 1 entry, climbing east-north
    (-75.0, 5.0),    // 2
    (-50.0, 35.0),   // 3
    (-15.0, 60.0),   // 4
    (30.0, 75.0),    // 5  top of outer loop
    (85.0, 80.0),    // 6
    (140.0, 70.0),   // 7
    (180.0, 50.0),   // 8  hairpin top-right
    (195.0, 20.0),   // 9
    (180.0, -5.0),   // 10
    (140.0, -15.0),  // 11 back inside, entering switchback
    (95.0, -5.0),    // 12
    (60.0, 20.0),    // 13 inner loop top
    (25.0, 35.0),    // 14
    (-5.0, 25.0),    // 15
    (5.0, 0.0),      // 16 inner crossover dip
    (40.0, -15.0),   // 17
    (85.0, -30.0),   // 18
    (130.0, -50.0),  // 19 sweep along bottom
    (115.0, -75.0),  // 20
    (60.0, -85.0),   // 21
    (0.0, -85.0),    // 22 bottom straight
    (-60.0, -80.0),  // 23
    (-100.0, -70.0), // 24 return to start/finish
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: {} <input.mlg> [output.mlg]",
            args.first()
                .map(String::as_str)
                .unwrap_or("inject_fake_gps_mlg")
        );
        return ExitCode::from(2);
    }
    let in_path = &args[1];
    let out_path = args.get(2).cloned().unwrap_or_else(|| {
        if let Some(stem) = in_path.strip_suffix(".mlg") {
            format!("{}_gps.mlg", stem)
        } else {
            format!("{}.gps.mlg", in_path)
        }
    });

    let data = match fs::read(in_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to read {}: {}", in_path, e);
            return ExitCode::FAILURE;
        }
    };

    if data.len() < HEADER_LEN_V1 || &data[0..5] != b"MLVLG" {
        eprintln!("not an MLVLG file: {}", in_path);
        return ExitCode::FAILURE;
    }

    let format_version = i16::from_be_bytes([data[6], data[7]]);
    let is_v2 = format_version == 2;
    let header_len = if is_v2 { HEADER_LEN_V2 } else { HEADER_LEN_V1 };
    let field_len = if is_v2 { FIELD_LEN_V2 } else { FIELD_LEN_V1 };
    if data.len() < header_len {
        eprintln!("MLG header is truncated");
        return ExitCode::FAILURE;
    }

    // Header offsets (matches the layout the speeduino parser walks).
    let info_off = 6 + 2 + 4;
    let data_begin_off = info_off + if is_v2 { 4 } else { 2 };
    let record_len_off = data_begin_off + 4;
    let num_fields_off = record_len_off + 2;

    let info_data_start = if is_v2 {
        u32::from_be_bytes([
            data[info_off],
            data[info_off + 1],
            data[info_off + 2],
            data[info_off + 3],
        ]) as usize
    } else {
        u16::from_be_bytes([data[info_off], data[info_off + 1]]) as usize
    };
    let data_begin_index = u32::from_be_bytes([
        data[data_begin_off],
        data[data_begin_off + 1],
        data[data_begin_off + 2],
        data[data_begin_off + 3],
    ]) as usize;
    let record_length = u16::from_be_bytes([data[record_len_off], data[record_len_off + 1]]);
    let num_fields = u16::from_be_bytes([data[num_fields_off], data[num_fields_off + 1]]);

    let fields_end = header_len + (num_fields as usize) * field_len;
    if fields_end > data.len()
        || data_begin_index > data.len()
        || data_begin_index < fields_end
        || (info_data_start != 0
            && (info_data_start < fields_end || info_data_start > data_begin_index))
    {
        eprintln!("header offsets out of bounds");
        return ExitCode::FAILURE;
    }

    let Some(new_record_length) = (record_length as usize).checked_add(8) else {
        eprintln!("record length overflow");
        return ExitCode::FAILURE;
    };
    let Some(new_num_fields) = num_fields.checked_add(2) else {
        eprintln!("field count overflow");
        return ExitCode::FAILURE;
    };
    let inserted_header_bytes = 2 * field_len;
    let Some(new_data_begin) = data_begin_index.checked_add(inserted_header_bytes) else {
        eprintln!("data_begin overflow");
        return ExitCode::FAILURE;
    };
    let new_info_data_start = if info_data_start == 0 {
        0
    } else {
        let Some(value) = info_data_start.checked_add(inserted_header_bytes) else {
            eprintln!("info_data_start overflow");
            return ExitCode::FAILURE;
        };
        value
    };
    if new_record_length > u16::MAX as usize
        || new_data_begin > u32::MAX as usize
        || new_info_data_start
            > if is_v2 {
                u32::MAX as usize
            } else {
                u16::MAX as usize
            }
    {
        eprintln!("updated MLG header value exceeds its field width");
        return ExitCode::FAILURE;
    }

    // ---- Build new file in memory ----
    let mut out = Vec::with_capacity(data.len() + 2 * field_len + 8 * 4096);

    // Header: copy verbatim, then patch the four changed fields below.
    out.extend_from_slice(&data[..header_len]);
    if is_v2 {
        out[info_off..info_off + 4].copy_from_slice(&(new_info_data_start as u32).to_be_bytes());
    } else {
        out[info_off..info_off + 2].copy_from_slice(&(new_info_data_start as u16).to_be_bytes());
    }
    out[data_begin_off..data_begin_off + 4].copy_from_slice(&(new_data_begin as u32).to_be_bytes());
    out[record_len_off..record_len_off + 2]
        .copy_from_slice(&(new_record_length as u16).to_be_bytes());
    out[num_fields_off..num_fields_off + 2].copy_from_slice(&new_num_fields.to_be_bytes());

    // Existing fields, followed by two new GPS field descriptors.
    out.extend_from_slice(&data[header_len..fields_end]);
    out.extend_from_slice(&build_field_descriptor(
        is_v2,
        "GPS Latitude",
        "deg",
        1e-7,
        "GPS",
    ));
    out.extend_from_slice(&build_field_descriptor(
        is_v2,
        "GPS Longitude",
        "deg",
        1e-7,
        "GPS",
    ));

    // Info section verbatim (between fields_end and data_begin_index).
    out.extend_from_slice(&data[fields_end..data_begin_index]);

    // ---- Walk data section, rewriting each record ----
    let mut off = data_begin_index;
    let mut record_index: usize = 0;
    let mut prev_raw_ts: u16 = 0;
    let mut wrap_count: u64 = 0;
    let mut data_record_count: usize = 0;
    let mut marker_count: usize = 0;

    while off + 4 <= data.len() {
        let block_type = data[off];
        let counter = data[off + 1];
        let raw_ts = u16::from_be_bytes([data[off + 2], data[off + 3]]);

        match block_type {
            0 => {
                let payload_end = off + 4 + record_length as usize;
                if payload_end + 1 > data.len() {
                    eprintln!(
                        "truncated data record at offset {} (need {}, have {})",
                        off,
                        record_length as usize + 5,
                        data.len() - off
                    );
                    return ExitCode::FAILURE;
                }

                if raw_ts < prev_raw_ts && (prev_raw_ts - raw_ts) > 30_000 {
                    wrap_count += 1;
                }
                prev_raw_ts = raw_ts;
                let timestamp_s = (raw_ts as f64 + wrap_count as f64 * 65_536.0) * 1e-5;

                let (lat_raw, lon_raw) = synth_lat_lon(timestamp_s, record_index);

                // Rewrite block: same 4-byte block header, original field
                // data, then 8 bytes of GPS, then a CRC over the field data
                // (matches canlogger's `sum of data bytes` algorithm).
                out.push(block_type);
                out.push(counter);
                out.extend_from_slice(&raw_ts.to_be_bytes());
                let original_payload = &data[off + 4..payload_end];
                out.extend_from_slice(original_payload);
                out.extend_from_slice(&lat_raw.to_be_bytes());
                out.extend_from_slice(&lon_raw.to_be_bytes());

                let mut crc: u8 = 0;
                for b in original_payload {
                    crc = crc.wrapping_add(*b);
                }
                for b in lat_raw.to_be_bytes() {
                    crc = crc.wrapping_add(b);
                }
                for b in lon_raw.to_be_bytes() {
                    crc = crc.wrapping_add(b);
                }
                out.push(crc);

                off = payload_end + 1; // include CRC
                record_index += 1;
                data_record_count += 1;
            }
            1 => {
                let marker_end = off + 4 + 50;
                if marker_end > data.len() {
                    eprintln!("truncated marker block at offset {}", off);
                    return ExitCode::FAILURE;
                }
                out.extend_from_slice(&data[off..marker_end]);
                off = marker_end;
                marker_count += 1;
            }
            _ => {
                eprintln!(
                    "unknown block type {} at offset {} - stopping at first unknown block",
                    block_type, off
                );
                return ExitCode::FAILURE;
            }
        }
    }

    if off != data.len() {
        eprintln!(
            "truncated block header at offset {} (have {} trailing bytes)",
            off,
            data.len() - off
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = fs::write(&out_path, &out) {
        eprintln!("failed to write {}: {}", out_path, e);
        return ExitCode::FAILURE;
    }

    println!("wrote {} bytes to {}", out.len(), out_path);
    println!(
        "  format v{}  fields {} -> {}  record_length {} -> {}",
        format_version, num_fields, new_num_fields, record_length, new_record_length
    );
    println!(
        "  info_data_start {} -> {}  data_begin {} -> {}",
        info_data_start, new_info_data_start, data_begin_index, new_data_begin
    );
    println!(
        "  data records: {}  markers: {}",
        data_record_count, marker_count
    );
    println!(
        "  trace: Zmeinka polyline ({} waypoints) centered at ({:.4}, {:.4}), {}s/lap",
        WAYPOINTS.len(),
        CENTER_LAT,
        CENTER_LON,
        LAP_PERIOD_S
    );

    ExitCode::SUCCESS
}

fn build_field_descriptor(
    is_v2: bool,
    name: &str,
    units: &str,
    scale: f32,
    category: &str,
) -> Vec<u8> {
    let len = if is_v2 { FIELD_LEN_V2 } else { FIELD_LEN_V1 };
    let mut buf = vec![0u8; len];
    buf[0] = FT_S32;
    write_padded(&mut buf[1..1 + 34], name);
    write_padded(&mut buf[35..35 + 10], units);
    buf[45] = 0; // display_style = MLG_FLOAT
    buf[46..50].copy_from_slice(&scale.to_be_bytes());
    buf[50..54].copy_from_slice(&0f32.to_be_bytes()); // transform
    buf[54] = 7; // digits
    if is_v2 {
        write_padded(&mut buf[55..55 + 34], category);
    }
    buf
}

fn write_padded(dst: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(dst.len());
    dst[..n].copy_from_slice(&bytes[..n]);
}

/// Synthesize a (lat, lon) pair as MLG raw S32 values (scale 1e-7 deg).
///
/// Walks the [`WAYPOINTS`] polyline at constant ground speed parameterised
/// by cumulative distance, completing one full lap every `LAP_PERIOD_S`.
/// A small per-record sine jitter adds GPS-like noise so the trace doesn't
/// look perfectly synthetic.
fn synth_lat_lon(t_s: f64, record_index: usize) -> (i32, i32) {
    let (east_m, north_m) = sample_polyline(t_s);
    let jitter = (record_index as f64).sin() * 0.5; // ±0.5 m

    let dlat_m = north_m + jitter;
    let dlon_m = east_m + jitter;

    let dlat = dlat_m / M_PER_DEG_LAT;
    let dlon = dlon_m / (M_PER_DEG_LAT * (CENTER_LAT.to_radians()).cos());
    let lat = CENTER_LAT + dlat;
    let lon = CENTER_LON + dlon;

    let lat_raw = (lat / 1e-7).round() as i32;
    let lon_raw = (lon / 1e-7).round() as i32;
    (lat_raw, lon_raw)
}

/// Sample the closed `WAYPOINTS` polyline at time `t_s`, returning local
/// `(east_m, north_m)` offsets from the track centre. Each segment is
/// linearly interpolated against its share of the perimeter so the
/// resulting motion is constant-speed.
fn sample_polyline(t_s: f64) -> (f64, f64) {
    let n = WAYPOINTS.len();
    debug_assert!(n >= 2);

    // Per-segment lengths (closing the loop with the last -> first leg).
    let mut seg_len = [0f64; 64];
    let mut perimeter = 0.0;
    for i in 0..n {
        let (ax, ay) = WAYPOINTS[i];
        let (bx, by) = WAYPOINTS[(i + 1) % n];
        let dx = bx - ax;
        let dy = by - ay;
        let l = (dx * dx + dy * dy).sqrt();
        seg_len[i] = l;
        perimeter += l;
    }

    let lap_progress = (t_s / LAP_PERIOD_S).rem_euclid(1.0);
    let mut target = lap_progress * perimeter;

    for i in 0..n {
        if target <= seg_len[i] {
            let (ax, ay) = WAYPOINTS[i];
            let (bx, by) = WAYPOINTS[(i + 1) % n];
            let f = if seg_len[i] > 0.0 {
                target / seg_len[i]
            } else {
                0.0
            };
            return (ax + (bx - ax) * f, ay + (by - ay) * f);
        }
        target -= seg_len[i];
    }
    WAYPOINTS[0]
}
