use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Adapter specs to download from GitHub if submodule is missing
const ADAPTER_SPECS: &[(&str, &str)] = &[
    (
        "haltech/haltech-nsp.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/haltech/haltech-nsp.adapter.yaml",
    ),
    (
        "ecumaster/ecumaster-emu-csv.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/ecumaster/ecumaster-emu-csv.adapter.yaml",
    ),
    (
        "link/link-llg.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/link/link-llg.adapter.yaml",
    ),
    (
        "aim/aim-xrk.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/aim/aim-xrk.adapter.yaml",
    ),
    (
        "romraider/romraider-csv.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/romraider/romraider-csv.adapter.yaml",
    ),
    (
        "speeduino/speeduino-mlg.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/speeduino/speeduino-mlg.adapter.yaml",
    ),
    (
        "rusefi/rusefi-mlg.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/rusefi/rusefi-mlg.adapter.yaml",
    ),
    (
        "emerald/emerald-lg.adapter.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/adapters/emerald/emerald-lg.adapter.yaml",
    ),
];

/// Protocol specs to download from GitHub if submodule is missing
const PROTOCOL_SPECS: &[(&str, &str)] = &[
    (
        "haltech/haltech-elite-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/haltech/haltech-elite-broadcast.protocol.yaml",
    ),
    (
        "ecumaster/ecumaster-emu-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/ecumaster/ecumaster-emu-broadcast.protocol.yaml",
    ),
    (
        "speeduino/speeduino-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/speeduino/speeduino-broadcast.protocol.yaml",
    ),
    (
        "rusefi/rusefi-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/rusefi/rusefi-broadcast.protocol.yaml",
    ),
    (
        "aem/aem-infinity-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/aem/aem-infinity-broadcast.protocol.yaml",
    ),
    (
        "megasquirt/megasquirt-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/megasquirt/megasquirt-broadcast.protocol.yaml",
    ),
    (
        "maxxecu/maxxecu-default.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/maxxecu/maxxecu-default.protocol.yaml",
    ),
    (
        "syvecs/syvecs-s7-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/syvecs/syvecs-s7-broadcast.protocol.yaml",
    ),
    (
        "emtron/emtron-broadcast.protocol.yaml",
        "https://raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs/main/protocols/emtron/emtron-broadcast.protocol.yaml",
    ),
];

fn main() {
    // Ensure OpenECU Alliance specs are available
    ensure_adapter_specs();
    ensure_protocol_specs();

    // Only run on Windows
    #[cfg(windows)]
    {
        // Embed icon and version info into Windows executable
        // Note: Windows resource embedding requires an .ico file
        // Convert assets/icons/windows.png to .ico format if needed:
        //   magick assets/icons/windows.png -define icon:auto-resize=256,128,64,48,32,16 assets/icons/windows.ico

        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/windows.ico")
            .set("ProductName", "UltraLog")
            .set("FileDescription", "High-performance ECU log viewer")
            .set("LegalCopyright", "Copyright (c) 2025 Cole Gentry");

        // Only compile if icon exists
        if std::path::Path::new("assets/icons/windows.ico").exists() {
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}

/// Ensure adapter specs are available, either from submodule or by downloading
fn ensure_adapter_specs() {
    let spec_dir = Path::new("spec/OECUASpecs/adapters");

    // Check if submodule is initialized (has the haltech adapter)
    let haltech_spec = spec_dir.join("haltech/haltech-nsp.adapter.yaml");
    if haltech_spec.exists() {
        println!("cargo:rerun-if-changed=spec/OECUASpecs/adapters");
        return;
    }

    println!("cargo:warning=OECUASpecs submodule not initialized, attempting to initialize...");

    // Try to initialize submodule
    let submodule_result = Command::new("git")
        .args(["submodule", "update", "--init", "--recursive"])
        .status();

    if let Ok(status) = submodule_result {
        if status.success() && haltech_spec.exists() {
            println!("cargo:warning=Successfully initialized OECUASpecs submodule");
            println!("cargo:rerun-if-changed=spec/OECUASpecs/adapters");
            return;
        }
    }

    // Submodule init failed, try downloading directly
    println!("cargo:warning=Submodule init failed, downloading adapter specs from GitHub...");
    download_adapter_specs(spec_dir);
}

/// Ensure protocol specs are available
fn ensure_protocol_specs() {
    let spec_dir = Path::new("spec/OECUASpecs/protocols");

    // Check if submodule is initialized (has the haltech protocol)
    let haltech_spec = spec_dir.join("haltech/haltech-elite-broadcast.protocol.yaml");
    if haltech_spec.exists() {
        println!("cargo:rerun-if-changed=spec/OECUASpecs/protocols");
        return;
    }

    println!("cargo:warning=Protocol specs not found, downloading from GitHub...");
    download_specs(spec_dir, PROTOCOL_SPECS);
}

/// Download adapter specs directly from GitHub
fn download_adapter_specs(spec_dir: &Path) {
    download_specs(spec_dir, ADAPTER_SPECS);
}

/// Generic spec downloader
fn download_specs(spec_dir: &Path, specs: &[(&str, &str)]) {
    // Create directory structure
    for (path, _) in specs {
        let full_path = spec_dir.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).ok();
        }
    }

    // Try curl first (available on most systems)
    for (path, url) in specs {
        let full_path = spec_dir.join(path);

        if full_path.exists() {
            continue;
        }

        // Try curl
        let result = Command::new("curl")
            .args(["-sSL", "-o", full_path.to_str().unwrap(), url])
            .status();

        if let Ok(status) = result {
            if status.success() {
                println!(
                    "cargo:warning=Downloaded {}",
                    full_path.file_name().unwrap().to_str().unwrap()
                );
                continue;
            }
        }

        // Try wget as fallback
        let result = Command::new("wget")
            .args(["-q", "-O", full_path.to_str().unwrap(), url])
            .status();

        if let Ok(status) = result {
            if status.success() {
                println!(
                    "cargo:warning=Downloaded {}",
                    full_path.file_name().unwrap().to_str().unwrap()
                );
                continue;
            }
        }

        // Try PowerShell on Windows
        #[cfg(windows)]
        {
            let ps_script = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                url,
                full_path.to_str().unwrap()
            );
            let result = Command::new("powershell")
                .args(["-Command", &ps_script])
                .status();

            if let Ok(status) = result {
                if status.success() {
                    println!(
                        "cargo:warning=Downloaded {}",
                        full_path.file_name().unwrap().to_str().unwrap()
                    );
                    continue;
                }
            }
        }

        // If we get here, all download methods failed
        // Create a placeholder that will cause a compile error with helpful message
        let error_content = format!(
            r#"# ERROR: Failed to download spec from GitHub
# URL: {}
#
# Please run one of the following:
#   git submodule update --init
# OR
#   curl -sSL -o {} {}
"#,
            url,
            full_path.display(),
            url
        );

        if let Ok(mut file) = fs::File::create(&full_path) {
            file.write_all(error_content.as_bytes()).ok();
        }

        println!(
            "cargo:warning=Failed to download {}, created placeholder",
            path
        );
    }

    println!("cargo:rerun-if-changed={}", spec_dir.display());
}
