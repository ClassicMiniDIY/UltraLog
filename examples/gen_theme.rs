//! Regenerate `src/ui/theme/tokens.rs` from `design/tokens/tokens.json`.
//!
//! Usage: `cargo run --example gen_theme`

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json_path = root.join("design/tokens/tokens.json");
    let out_path = root.join("src/ui/theme/tokens.rs");

    let json = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", json_path.display()));
    let source = ultralog::ui::theme::codegen::generate(&json)
        .unwrap_or_else(|e| panic!("generate from {}: {e}", json_path.display()));
    std::fs::write(&out_path, source)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
    println!("wrote {}", out_path.display());
}
