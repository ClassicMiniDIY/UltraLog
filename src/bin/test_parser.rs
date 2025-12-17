use std::fs;

// Import from the library
use ultralog::parsers::{Haltech, Parseable};

fn main() {
    let path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";

    println!("Reading file: {}", path);
    let contents = fs::read_to_string(path).expect("Failed to read file");
    println!("File size: {} bytes", contents.len());

    println!("\nParsing Haltech log...");
    let parser = Haltech;
    match parser.parse(&contents) {
        Ok(log) => {
            println!("\n=== Parse Results ===");
            println!("Channels: {}", log.channels.len());
            println!("Data points: {}", log.data.len());
            println!(
                "Time range: {} to {} seconds",
                log.times.first().unwrap_or(&"N/A".to_string()),
                log.times.last().unwrap_or(&"N/A".to_string())
            );

            println!("\n=== First 15 Channels (with units) ===");
            for (i, channel) in log.channels.iter().take(15).enumerate() {
                let unit = channel.unit();
                let unit_str = if unit.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", unit)
                };
                println!(
                    "  {:2}. {} ({}){}",
                    i + 1,
                    channel.name(),
                    channel.type_name(),
                    unit_str
                );
            }

            if log.channels.len() > 15 {
                println!("  ... and {} more channels", log.channels.len() - 15);
            }

            println!("\n=== Sample Data (first 5 rows, first 8 channels) ===");
            // Print channel headers
            let header: Vec<String> = log
                .channels
                .iter()
                .take(8)
                .map(|c| {
                    let name = c.name();
                    let short_name = if name.len() > 12 { &name[..12] } else { &name };
                    format!("{:>12}", short_name)
                })
                .collect();
            println!("  Time      | {}", header.join(" | "));
            println!("  ----------+-{}", vec!["-------------"; 8].join("-+-"));

            for (time, row) in log.times.iter().zip(log.data.iter()).take(5) {
                let values: Vec<String> = row
                    .iter()
                    .zip(log.channels.iter())
                    .take(8)
                    .map(|(v, c)| {
                        let val = v.as_f64();
                        let unit = c.unit();
                        if unit.is_empty() {
                            format!("{:>12.2}", val)
                        } else {
                            format!("{:>9.2} {:<2}", val, unit)
                        }
                    })
                    .collect();
                println!("  {:>8}s | {}", time, values.join(" | "));
            }

            // Show some key channels with converted values
            println!("\n=== Key Channel Values (first data point) ===");
            if let Some(first_row) = log.data.first() {
                for (i, channel) in log.channels.iter().enumerate() {
                    let name = channel.name();
                    // Show specific interesting channels
                    if name.contains("RPM")
                        || name.contains("Manifold Pressure") && !name.contains("Derivative")
                        || name.contains("Battery Voltage")
                        || name.contains("Throttle Position") && !name.contains("Derivative")
                        || name.contains("Wideband O2")
                        || name.contains("Oil Pressure")
                        || name.contains("Fuel Pressure")
                        || name.contains("Ignition Angle") && !name.contains("Bank")
                        || name == "Driven Wheel Speed"
                        || name == "Undriven Wheel Speed"
                    {
                        if let Some(value) = first_row.get(i) {
                            let unit = channel.unit();
                            println!("  {}: {:.2} {}", name, value.as_f64(), unit);
                        }
                    }
                }
            }

            println!("\n=== Success! Parser working correctly ===");
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    }
}
