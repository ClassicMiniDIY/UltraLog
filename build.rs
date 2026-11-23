fn main() {
    // The OpenECU spec corpus is vendored in this repository under spec/openecu and
    // embedded by src/adapters/registry.rs with include_str!. It used to be downloaded
    // at build time from the (now archived) ClassicMiniDIY/OECUASpecs repository, which
    // meant a clean clone could not build without network access. UltraLog owns it now.
    println!("cargo:rerun-if-changed=spec/openecu");

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
            .set("LegalCopyright", "Copyright (c) 2026 Cole Gentry");

        // Only compile if icon exists
        if std::path::Path::new("assets/icons/windows.ico").exists() {
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
