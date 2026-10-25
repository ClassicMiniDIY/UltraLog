//! Generator for `tokens.rs` from `design/tokens/tokens.json`.
//!
//! Run `cargo run --example gen_theme` after editing the JSON. The test at the bottom fails when
//! the committed `tokens.rs` no longer matches the JSON.
//!
//! Resolution rules (they match `design/tokens/tokens.ts`):
//! - A colour token whose value is a string applies to every theme.
//! - A theme with no entry of its own takes the `night` value.
//! - `{name}` references resolve inside the same theme.
//! - Effects come from the shadow tokens: glow from `glow-cyan` (first blurred layer), bloom from
//!   `bloom-series`, the redline is flat when `redline-0` points at `line-control`, the plot floor
//!   is on when `plot-floor` is not fully transparent. High contrast turns every effect off.

use serde_json::Value;
use std::fmt::Write as _;

const FALLBACK_THEME: &str = "night";

/// Generate the Rust source of `tokens.rs` from the JSON text of `tokens.json`.
pub fn generate(json: &str) -> Result<String, String> {
    let doc: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let themes: Vec<String> = doc["color"]["themes"]
        .as_array()
        .ok_or("color.themes missing")?
        .iter()
        .filter_map(|t| t["id"].as_str().map(str::to_owned))
        .collect();
    let tokens = doc["color"]["tokens"]
        .as_array()
        .ok_or("color.tokens missing")?;
    let names: Vec<&str> = tokens.iter().filter_map(|t| t["name"].as_str()).collect();

    let mut out = String::new();
    out.push_str(HEADER);

    out.push_str("#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Tokens {\n");
    for name in &names {
        let _ = writeln!(out, "    pub {}: Color32,", ident(name));
    }
    out.push_str("    pub fx: Effects,\n}\n");

    for theme in &themes {
        let _ = writeln!(
            out,
            "\npub const {}: Tokens = Tokens {{",
            theme.to_uppercase()
        );
        for name in &names {
            let hex = resolve(tokens, name, theme, 0)?;
            let _ = writeln!(out, "    {}: {},", ident(name), color_expr(&hex)?);
        }
        let fx = effects(&doc, tokens, theme)?;
        let _ = writeln!(
            out,
            "    fx: Effects {{ glow_blur: {:?}, glow_alpha: {:?}, bloom: {:?}, redline_gradient: {}, plot_floor: {} }},",
            fx.glow_blur, fx.glow_alpha, fx.bloom, fx.redline_gradient, fx.plot_floor
        );
        out.push_str("};\n");
    }

    out.push('\n');
    px_module(&mut out, &doc["spacing"]["tokens"], "space", "f32")?;
    px_module(&mut out, &doc["radius"]["tokens"], "radius", "u8")?;
    px_module(&mut out, &doc["stroke"]["tokens"], "stroke", "f32")?;
    text_module(&mut out, &doc["type"]["groups"])?;
    Ok(out)
}

const HEADER: &str = "\
//! UltraLog design tokens — generated from `design/tokens/tokens.json` by
//! `cargo run --example gen_theme`. Do not edit by hand.
//! Seven anchor themes. The live theme is built by `super::tokens_at`.
use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// Blur radius of glow-cyan / glow-magenta (px). 0 = no glow.
    pub glow_blur: f32,
    /// Alpha of the glow colour.
    pub glow_alpha: f32,
    /// Width of the translucent bloom drawn under series lines (px). 0 = off.
    pub bloom: f32,
    /// Draw the sunset redline as a gradient (false = flat line-control hairline).
    pub redline_gradient: bool,
    /// Draw the perspective floor inside plots.
    pub plot_floor: bool,
}

";

fn ident(name: &str) -> String {
    name.replace('-', "_")
}

fn find<'a>(tokens: &'a [Value], name: &str) -> Option<&'a Value> {
    tokens.iter().find(|t| t["name"].as_str() == Some(name))
}

/// The raw value of a token for a theme, before reference resolution.
fn raw<'a>(token: &'a Value, theme: &str) -> Option<&'a str> {
    match &token["value"] {
        Value::String(s) => Some(s),
        Value::Object(m) => m
            .get(theme)
            .or_else(|| m.get(FALLBACK_THEME))
            .and_then(Value::as_str),
        _ => None,
    }
}

fn resolve(tokens: &[Value], name: &str, theme: &str, depth: u8) -> Result<String, String> {
    if depth > 8 {
        return Err(format!("reference loop at {name}"));
    }
    let token = find(tokens, name).ok_or_else(|| format!("unknown token {name}"))?;
    let v = raw(token, theme).ok_or_else(|| format!("{name} has no value for {theme}"))?;
    match v.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        Some(target) => resolve(tokens, target, theme, depth + 1),
        None => Ok(v.to_ascii_lowercase()),
    }
}

fn parse_hex(hex: &str) -> Result<[u8; 4], String> {
    let h = hex
        .strip_prefix('#')
        .ok_or_else(|| format!("not a hex colour: {hex}"))?;
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|e| format!("{hex}: {e}"));
    match h.len() {
        6 => Ok([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Ok([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => Err(format!("not a hex colour: {hex}")),
    }
}

/// Opaque colours use `from_rgb`; translucent ones are premultiplied in sRGB space.
fn color_expr(hex: &str) -> Result<String, String> {
    let [r, g, b, a] = parse_hex(hex)?;
    if a == 255 {
        return Ok(format!("Color32::from_rgb({r}, {g}, {b})"));
    }
    let pm = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
    Ok(format!(
        "Color32::from_rgba_premultiplied({}, {}, {}, {a})",
        pm(r),
        pm(g),
        pm(b)
    ))
}

struct Fx {
    glow_blur: f32,
    glow_alpha: f32,
    bloom: f32,
    redline_gradient: bool,
    plot_floor: bool,
}

fn effects(doc: &Value, tokens: &[Value], theme: &str) -> Result<Fx, String> {
    if theme == "hc" {
        return Ok(Fx {
            glow_blur: 0.0,
            glow_alpha: 0.0,
            bloom: 0.0,
            redline_gradient: false,
            plot_floor: false,
        });
    }
    let shadows = doc["shadow"]["tokens"]
        .as_array()
        .ok_or("shadow.tokens missing")?;
    let shadow = |name: &str| {
        find(shadows, name)
            .and_then(|t| raw(t, theme))
            .unwrap_or("none")
    };

    let (glow_blur, glow_alpha) = first_blurred_layer(shadow("glow-cyan"));
    let bloom = shadow("bloom-series")
        .split_whitespace()
        .filter_map(px)
        .nth(2)
        .unwrap_or(0.0);
    let redline_raw = find(tokens, "redline-0").and_then(|t| raw(t, theme));
    let redline_gradient = redline_raw != Some("{line-control}");
    let plot_floor = parse_hex(&resolve(tokens, "plot-floor", theme, 0)?)?[3] > 0;
    Ok(Fx {
        glow_blur,
        glow_alpha,
        bloom,
        redline_gradient,
        plot_floor,
    })
}

/// Blur and alpha of the first CSS box-shadow layer with a non-zero blur, e.g.
/// `0 0 0 1px #2de2ff, 0 0 16px rgba(45,226,255,0.35)` -> (16, 0.35).
fn first_blurred_layer(css: &str) -> (f32, f32) {
    for layer in shadow_layers(css) {
        let lengths: Vec<f32> = layer.split_whitespace().filter_map(px).collect();
        let blur = lengths.get(2).copied().unwrap_or(0.0);
        if blur > 0.0 {
            let alpha = layer
                .rsplit(',')
                .next()
                .and_then(|a| a.trim().trim_end_matches(')').parse::<f32>().ok())
                .unwrap_or(1.0);
            return (blur, alpha);
        }
    }
    (0.0, 0.0)
}

/// Split a CSS shadow list on the commas between layers, not those inside `rgba(..)`.
fn shadow_layers(css: &str) -> Vec<&str> {
    let mut layers = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, ch) in css.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                layers.push(css[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    layers.push(css[start..].trim());
    layers
}

fn px(s: &str) -> Option<f32> {
    if s == "0" {
        return Some(0.0);
    }
    s.strip_suffix("px")?.parse().ok()
}

fn px_module(out: &mut String, tokens: &Value, module: &str, ty: &str) -> Result<(), String> {
    let tokens = tokens
        .as_array()
        .ok_or_else(|| format!("{module} tokens missing"))?;
    let _ = writeln!(out, "pub mod {module} {{");
    for t in tokens {
        let name = t["name"].as_str().ok_or("token without a name")?;
        let value = t["value"]
            .as_str()
            .and_then(px)
            .ok_or_else(|| format!("{name}: expected a px value"))?;
        let literal = if ty == "u8" {
            format!("{}", value.min(255.0) as u8)
        } else {
            format!("{value:?}")
        };
        let _ = writeln!(
            out,
            "    pub const {}: {ty} = {literal};",
            ident(name).to_uppercase()
        );
    }
    out.push_str("}\n");
    Ok(())
}

fn text_module(out: &mut String, groups: &Value) -> Result<(), String> {
    let groups = groups.as_array().ok_or("type.groups missing")?;
    out.push_str(
        "\n/// Type scale: (family, size px, line height px). Families: \"display\" = Saira, \
         \"ui\" = Outfit, \"mono\" = JetBrains Mono.\npub mod text {\n",
    );
    for g in groups {
        let family = g["family"].as_str().ok_or("type group without a family")?;
        for s in g["styles"].as_array().ok_or("type group without styles")? {
            let name = s["name"].as_str().ok_or("type style without a name")?;
            let size = s["fontSize"].as_str().and_then(px);
            let line = s["lineHeight"].as_str().and_then(px);
            let (Some(size), Some(line)) = (size, line) else {
                return Err(format!("{name}: fontSize/lineHeight must be px"));
            };
            let _ = writeln!(
                out,
                "    pub const {}: (&str, f32, f32) = ({family:?}, {size:?}, {line:?});",
                ident(name).to_uppercase()
            );
        }
    }
    out.push_str("}\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = include_str!("../../../design/tokens/tokens.json");
    const COMMITTED: &str = include_str!("tokens.rs");

    #[test]
    fn committed_tokens_match_json() {
        let generated = generate(JSON).expect("tokens.json must generate");
        assert!(
            generated == COMMITTED,
            "src/ui/theme/tokens.rs is stale: run `cargo run --example gen_theme`"
        );
    }

    #[test]
    fn glow_layer_parsing() {
        assert_eq!(
            first_blurred_layer("0 0 0 1px #2de2ff, 0 0 16px rgba(45,226,255,0.35)"),
            (16.0, 0.35)
        );
        assert_eq!(first_blurred_layer("none"), (0.0, 0.0));
    }
}
