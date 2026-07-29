//! Quick diagnostic: do "GPS Latitude" / "GPS Longitude" resolve to canonical
//! GPS IDs in the OECUA registry the way the Track Map widget expects?

use ultralog::adapters::registry;

fn main() {
    let names = [
        "GPS Latitude",
        "GPS Longitude",
        "gps latitude",
        "GPS Lat",
        "Latitude",
        "Lon",
    ];
    println!("adapters loaded: {}", registry::get_adapters().len());
    for name in names {
        match registry::get_channel_metadata(name) {
            Some(meta) => println!(
                "  {:<14} -> canonical_id={:<14} display={:<14} vendor={}",
                format!("\"{}\"", name),
                meta.canonical_id,
                meta.display_name,
                meta.vendor
            ),
            None => println!("  \"{}\" -> NOT FOUND", name),
        }
    }
}
