use std::env;
use std::fs;

// Import from the library
use ultralog::parsers::{EcuMaster, EcuType, Haltech, Link, Parseable, Speeduino};

fn main() {
    // Get file path from command line or use default
    let args: Vec<String> = env::args().collect();
    let path = if args.len() > 1 {
        &args[1]
    } else {
        "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv"
    };

    println!("Reading file: {}", path);
    let binary_data = fs::read(path).expect("Failed to read file");
    println!("File size: {} bytes", binary_data.len());

    // Auto-detect file format - try binary formats first
    let (ecu_type, log) = if Speeduino::detect(&binary_data) {
        println!("\nDetected: Speeduino/rusEFI MLG format");
        println!("Parsing Speeduino/rusEFI log...");
        match Speeduino::parse_binary(&binary_data) {
            Ok(log) => (EcuType::Speeduino, log),
            Err(e) => {
                eprintln!("Parse error: {}", e);
                std::process::exit(1);
            }
        }
    } else if Link::detect(&binary_data) {
        println!("\nDetected: Link ECU LLG format");
        println!("Parsing Link ECU log...");
        match Link::parse_binary(&binary_data) {
            Ok(log) => (EcuType::Link, log),
            Err(e) => {
                eprintln!("Parse error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Try text-based formats
        let contents = match std::str::from_utf8(&binary_data) {
            Ok(c) => c.to_string(),
            Err(_) => String::from_utf8_lossy(&binary_data).to_string(),
        };

        if EcuMaster::detect(&contents) {
            println!("\nDetected: ECUMaster format");
            println!("Parsing ECUMaster log...");
            let parser = EcuMaster;
            match parser.parse(&contents) {
                Ok(log) => (EcuType::EcuMaster, log),
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            println!("\nDetected: Haltech format");
            println!("Parsing Haltech log...");
            let parser = Haltech;
            match parser.parse(&contents) {
                Ok(log) => (EcuType::Haltech, log),
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    };

    println!("\n=== Parse Results ===");
    println!("ECU Type: {}", ecu_type.name());
    println!("Channels: {}", log.channels.len());
    println!("Data points: {}", log.data.len());
    println!(
        "Time range: {:.3} to {:.3} seconds",
        log.times.first().unwrap_or(&0.0),
        log.times.last().unwrap_or(&0.0)
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
    println!("  ----------+-{}", ["-------------"; 8].join("-+-"));

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

    // Show some key channels with values
    println!("\n=== Key Channel Values (first data point) ===");
    if let Some(first_row) = log.data.first() {
        for (i, channel) in log.channels.iter().enumerate() {
            let name = channel.name().to_lowercase();
            // Show specific interesting channels
            if name.contains("rpm")
                || name.contains("tps")
                || name.contains("throttle")
                || name.contains("map")
                || name.contains("manifold")
                || name.contains("battery")
                || name.contains("voltage")
                || name.contains("lambda")
                || name.contains("afr")
                || name.contains("oil")
                || name.contains("fuel")
                || name.contains("ignition")
                || name.contains("angle")
                || name.contains("speed")
                || name.contains("temp")
            {
                if let Some(value) = first_row.get(i) {
                    let unit = channel.unit();
                    println!("  {}: {:.2} {}", channel.name(), value.as_f64(), unit);
                }
            }
        }
    }

    println!("\n=== Success! Parser working correctly ===");
}
