//! Diagnostic: parse an MLG file and replicate `detect_gps_channels` end-to-end.

use std::env;
use std::fs;

use ultralog::adapters::registry;
use ultralog::parsers::Speeduino;

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "exampleLogs/rusefi/Log1_gps.mlg".to_string());
    let data = fs::read(&path).expect("read file");
    let log = Speeduino::parse_binary(&data).expect("parse");
    println!("path: {}", path);
    println!("channels: {}", log.channels.len());

    let mut lat_idx: Option<usize> = None;
    let mut lon_idx: Option<usize> = None;
    let mut gps_hits = Vec::new();
    for (i, ch) in log.channels.iter().enumerate() {
        let name = ch.name();
        if name.to_lowercase().contains("gps")
            || name.to_lowercase().contains("lat")
            || name.to_lowercase().contains("lon")
        {
            let canon = registry::get_channel_metadata(&name)
                .map(|m| m.canonical_id)
                .unwrap_or_else(|| "<no metadata>".to_string());
            gps_hits.push((i, name.clone(), canon.clone()));
            match canon.as_str() {
                "gps_latitude" => lat_idx = Some(i),
                "gps_longitude" => lon_idx = Some(i),
                _ => {}
            }
        }
    }
    println!("GPS-ish channels found: {}", gps_hits.len());
    for (i, name, canon) in &gps_hits {
        println!("  [{}] name={:?}  canonical_id={}", i, name, canon);
    }
    println!("detect_gps_channels result: {:?}", (lat_idx, lon_idx));
}
