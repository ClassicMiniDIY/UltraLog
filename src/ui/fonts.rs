//! Font families and the type scale.
//!
//! Three brand faces, each with one job (design/design-system/README.md → Typography): Saira for
//! titles and brand moments, Outfit for the interface, JetBrains Mono for numbers. egui families
//! have no weight axis, so every weight the type scale uses is its own named family, built from a
//! static instance in `assets/fonts/` (see `scripts/fonts/build_static_fonts.sh`).
//!
//! Every family falls back to egui's default fonts (Greek, Cyrillic, emoji), then the bundled
//! Noto faces for Devanagari, Bengali and Arabic, then a tiny CJK subset that covers only the
//! language picker's own names, then, for the `ja` / `zh-CN` locales, a CJK font from the OS.

use std::collections::BTreeMap;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

use crate::i18n::Language;
use crate::ui::theme::tokens::text;

/// Outfit 500: labels, buttons, tabs.
pub const UI_MEDIUM: &str = "ui-medium";
/// Outfit 600: section headings, card titles.
pub const UI_SEMIBOLD: &str = "ui-semibold";
/// Saira 600: panel and window titles, the overline.
pub const DISPLAY: &str = "display";
/// Saira 700 italic: headlines (brand moments only).
pub const DISPLAY_BOLD_ITALIC: &str = "display-bold-italic";
/// Saira 800 italic: the display size (brand moments only).
pub const DISPLAY_BLACK_ITALIC: &str = "display-black-italic";
/// JetBrains Mono 500: large data readouts.
pub const MONO_MEDIUM: &str = "mono-medium";
/// Phosphor fill: transport glyphs (play, stop) and the favourite star only.
pub const PHOSPHOR_FILL: &str = "phosphor-fill";

/// Phosphor regular sits right after each brand face, so an icon codepoint in any label
/// resolves to the outline icon set.
const PHOSPHOR: &str = "phosphor";

/// Named text styles beyond egui's five (Body, Button, Small, Heading, Monospace).
pub mod style {
    pub const TITLE: &str = "title";
    pub const OVERLINE: &str = "overline";
    pub const LABEL: &str = "label";
    pub const HEADLINE: &str = "headline";
    pub const DISPLAY: &str = "display";
    pub const DATA: &str = "data";
    pub const DATA_LG: &str = "data_lg";
    pub const DATA_SM: &str = "data_sm";
}

/// `TextStyle::Name` for one of the [`style`] names.
pub fn text_style(name: &str) -> TextStyle {
    TextStyle::Name(name.into())
}

const BRAND_FACES: [(&str, &[u8]); 8] = [
    (
        "Outfit-Regular",
        include_bytes!("../../assets/fonts/Outfit-Regular.ttf"),
    ),
    (
        "Outfit-Medium",
        include_bytes!("../../assets/fonts/Outfit-Medium.ttf"),
    ),
    (
        "Outfit-SemiBold",
        include_bytes!("../../assets/fonts/Outfit-SemiBold.ttf"),
    ),
    (
        "Saira-SemiBold",
        include_bytes!("../../assets/fonts/Saira-SemiBold.ttf"),
    ),
    (
        "Saira-BoldItalic",
        include_bytes!("../../assets/fonts/Saira-BoldItalic.ttf"),
    ),
    (
        "Saira-ExtraBoldItalic",
        include_bytes!("../../assets/fonts/Saira-ExtraBoldItalic.ttf"),
    ),
    (
        "JetBrainsMono-Regular",
        include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "JetBrainsMono-Medium",
        include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
    ),
];

/// Bundled script fallbacks, in fallback order.
const SCRIPT_FACES: [(&str, &[u8]); 4] = [
    (
        "NotoSansDevanagari",
        include_bytes!("../../assets/fonts/NotoSansDevanagari-Regular.ttf"),
    ),
    (
        "NotoSansBengali",
        include_bytes!("../../assets/fonts/NotoSansBengali-Regular.ttf"),
    ),
    (
        "NotoSansArabic",
        include_bytes!("../../assets/fonts/NotoSansArabic-Regular.ttf"),
    ),
    (
        "NotoSansSC-LanguageNames",
        include_bytes!("../../assets/fonts/NotoSansSC-LanguageNames.ttf"),
    ),
];

const SYSTEM_CJK: &str = "system-cjk";

/// Font definitions for `language`. Only `ja` and `zh-CN` read a CJK font from disk.
pub fn definitions(language: Language) -> FontDefinitions {
    let system_cjk = system_cjk_font(language);
    build_definitions(system_cjk)
}

fn build_definitions(system_cjk: Option<FontData>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let egui_proportional = fonts.families[&FontFamily::Proportional].clone();
    let egui_monospace = fonts.families[&FontFamily::Monospace].clone();

    for (name, bytes) in BRAND_FACES.iter().chain(SCRIPT_FACES.iter()) {
        fonts
            .font_data
            .insert((*name).to_owned(), FontData::from_static(bytes).into());
    }
    let mut scripts: Vec<String> = SCRIPT_FACES.iter().map(|(n, _)| (*n).to_owned()).collect();
    if let Some(data) = system_cjk {
        fonts.font_data.insert(SYSTEM_CJK.to_owned(), data.into());
        scripts.push(SYSTEM_CJK.to_owned());
    }

    fonts.font_data.insert(
        PHOSPHOR.to_owned(),
        FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
    );
    fonts.font_data.insert(
        PHOSPHOR_FILL.to_owned(),
        FontData::from_static(egui_phosphor::Variant::Fill.font_bytes()).into(),
    );

    let with_tail = |face: &str, tail: &[String]| -> Vec<String> {
        [face.to_owned(), PHOSPHOR.to_owned()]
            .into_iter()
            .chain(tail.iter().cloned())
            .chain(scripts.iter().cloned())
            .collect()
    };

    let families = [
        (
            FontFamily::Proportional,
            "Outfit-Regular",
            &egui_proportional,
        ),
        (
            FontFamily::Monospace,
            "JetBrainsMono-Regular",
            &egui_monospace,
        ),
        (
            FontFamily::Name(UI_MEDIUM.into()),
            "Outfit-Medium",
            &egui_proportional,
        ),
        (
            FontFamily::Name(UI_SEMIBOLD.into()),
            "Outfit-SemiBold",
            &egui_proportional,
        ),
        (
            FontFamily::Name(DISPLAY.into()),
            "Saira-SemiBold",
            &egui_proportional,
        ),
        (
            FontFamily::Name(DISPLAY_BOLD_ITALIC.into()),
            "Saira-BoldItalic",
            &egui_proportional,
        ),
        (
            FontFamily::Name(DISPLAY_BLACK_ITALIC.into()),
            "Saira-ExtraBoldItalic",
            &egui_proportional,
        ),
        (
            FontFamily::Name(MONO_MEDIUM.into()),
            "JetBrainsMono-Medium",
            &egui_monospace,
        ),
    ];
    for (family, face, tail) in families {
        fonts.families.insert(family, with_tail(face, tail));
    }
    fonts.families.insert(
        FontFamily::Name(PHOSPHOR_FILL.into()),
        vec![PHOSPHOR_FILL.to_owned()],
    );
    fonts
}

/// The type scale (`design/tokens/tokens.json` → type) as egui text styles, sizes multiplied by
/// the user's font scale and rounded like [`crate::app::UltraLogApp::scaled_font`].
pub fn text_styles(scale: f32) -> BTreeMap<TextStyle, FontId> {
    let id = |(_, size, _): (&str, f32, f32), family: FontFamily| {
        FontId::new((size * scale).round(), family)
    };
    let named = |name: &str| FontFamily::Name(name.into());
    BTreeMap::from([
        (TextStyle::Body, id(text::BODY, FontFamily::Proportional)),
        (TextStyle::Button, id(text::LABEL, named(UI_MEDIUM))),
        (
            TextStyle::Small,
            id(text::CAPTION, FontFamily::Proportional),
        ),
        (TextStyle::Heading, id(text::HEADING, named(UI_SEMIBOLD))),
        (TextStyle::Monospace, id(text::DATA, FontFamily::Monospace)),
        (text_style(style::TITLE), id(text::TITLE, named(DISPLAY))),
        (
            text_style(style::OVERLINE),
            id(text::OVERLINE, named(DISPLAY)),
        ),
        (text_style(style::LABEL), id(text::LABEL, named(UI_MEDIUM))),
        (
            text_style(style::HEADLINE),
            id(text::HEADLINE, named(DISPLAY_BOLD_ITALIC)),
        ),
        (
            text_style(style::DISPLAY),
            id(text::DISPLAY, named(DISPLAY_BLACK_ITALIC)),
        ),
        (
            text_style(style::DATA),
            id(text::DATA, FontFamily::Monospace),
        ),
        (
            text_style(style::DATA_LG),
            id(text::DATA_LG, named(MONO_MEDIUM)),
        ),
        (
            text_style(style::DATA_SM),
            id(text::DATA_SM, FontFamily::Monospace),
        ),
    ])
}

/// System CJK font candidates per locale: `(path, face index in a .ttc)`. The first one that
/// exists is used. Noto Sans CJK `.ttc` files order their faces JP, KR, SC, TC, HK.
fn cjk_candidates(language: Language) -> &'static [(&'static str, u32)] {
    match language {
        Language::Japanese => {
            #[cfg(target_os = "macos")]
            {
                &[
                    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
                    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
                ]
            }
            #[cfg(target_os = "windows")]
            {
                &[
                    ("C:\\Windows\\Fonts\\YuGothR.ttc", 0),
                    ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
                    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),
                ]
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                &[
                    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
                    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
                    (
                        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                        0,
                    ),
                    ("/usr/share/fonts/truetype/fonts-japanese-gothic.ttf", 0),
                ]
            }
        }
        Language::ChineseSimplified => {
            #[cfg(target_os = "macos")]
            {
                &[
                    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
                    ("/System/Library/Fonts/STHeiti Light.ttc", 0),
                ]
            }
            #[cfg(target_os = "windows")]
            {
                &[
                    ("C:\\Windows\\Fonts\\msyh.ttc", 0),
                    ("C:\\Windows\\Fonts\\simsun.ttc", 0),
                ]
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                &[
                    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
                    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 2),
                    (
                        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                        2,
                    ),
                    ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
                ]
            }
        }
        _ => &[],
    }
}

fn system_cjk_font(language: Language) -> Option<FontData> {
    for (path, index) in cjk_candidates(language) {
        match std::fs::read(path) {
            Ok(bytes) => {
                tracing::info!("Loaded CJK font {path}");
                let mut data = FontData::from_owned(bytes);
                data.index = *index;
                return Some(data);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => tracing::warn!("Could not read CJK font {path}: {e}"),
        }
    }
    if !cjk_candidates(language).is_empty() {
        tracing::warn!(
            "No system CJK font found for {}; text will show missing glyphs",
            language.locale_code()
        );
    }
    None
}

/// Install the fonts for `language` on `ctx`.
pub fn install(ctx: &egui::Context, language: Language) {
    ctx.set_fonts(definitions(language));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_starts_with_its_brand_face_and_keeps_fallbacks() {
        let fonts = build_definitions(None);
        let first = |f: FontFamily| fonts.families[&f][0].clone();
        assert_eq!(first(FontFamily::Proportional), "Outfit-Regular");
        assert_eq!(first(FontFamily::Monospace), "JetBrainsMono-Regular");
        assert_eq!(first(FontFamily::Name(DISPLAY.into())), "Saira-SemiBold");
        for (family, list) in &fonts.families {
            for name in list {
                assert!(fonts.font_data.contains_key(name), "{name} has no data");
            }
            if *family == FontFamily::Name(PHOSPHOR_FILL.into()) {
                continue; // icon-only family
            }
            assert_eq!(
                list[1], PHOSPHOR,
                "{family:?}: icons resolve right after the face"
            );
            assert!(list.iter().any(|n| n == "NotoSansDevanagari"));
            assert!(list.iter().any(|n| n == "NotoSansSC-LanguageNames"));
            // egui's defaults stay as fallbacks for Greek, Cyrillic and emoji.
            assert!(list.len() > SCRIPT_FACES.len() + 2, "{list:?}");
        }
    }

    /// Lay out one frame with `fonts` and report which characters of `text` have no glyph in
    /// `family` (after its fallbacks).
    fn missing_glyphs(fonts: FontDefinitions, family: FontFamily, text: &str) -> String {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear(); // no renderer in a test
        let id = FontId::new(14.0, family);
        ctx.fonts_mut(|f| text.chars().filter(|c| !f.has_glyph(&id, *c)).collect())
    }

    #[test]
    fn every_locale_name_and_brand_glyph_renders_without_a_system_font() {
        // The language picker shows every language in its own script, in every locale.
        let names: String = Language::all().iter().map(|l| l.display_name()).collect();
        for (family, symbols) in [
            (FontFamily::Proportional, "ƒ−·°λ→✓"),
            (FontFamily::Monospace, "−·°λ→"),
            (FontFamily::Name(DISPLAY.into()), "ƒ−·°"),
        ] {
            let missing = missing_glyphs(
                build_definitions(None),
                family.clone(),
                &format!("{names}{symbols}"),
            );
            let missing: String = missing.chars().filter(|c| !c.is_whitespace()).collect();
            assert!(missing.is_empty(), "{family:?} lacks {missing:?}");
        }
    }

    #[test]
    fn system_cjk_font_covers_the_ui_when_the_os_has_one() {
        for (language, sample) in [
            (Language::Japanese, "設定ファイル表示"),
            (Language::ChineseSimplified, "设置文件显示"),
        ] {
            let Some(data) = system_cjk_font(language) else {
                continue; // No CJK font on this machine (e.g. a bare CI image).
            };
            let missing = missing_glyphs(
                build_definitions(Some(data)),
                FontFamily::Proportional,
                sample,
            );
            assert!(missing.is_empty(), "{language:?} lacks {missing:?}");
        }
    }

    #[test]
    fn non_cjk_locales_read_no_system_font() {
        assert!(cjk_candidates(Language::English).is_empty());
        assert!(cjk_candidates(Language::Hindi).is_empty());
        assert!(!cjk_candidates(Language::Japanese).is_empty());
        assert!(!cjk_candidates(Language::ChineseSimplified).is_empty());
    }

    #[test]
    fn text_styles_follow_the_type_scale_and_font_scale() {
        let s = text_styles(1.0);
        assert_eq!(s[&TextStyle::Body].size, 14.0);
        assert_eq!(s[&TextStyle::Button].size, 13.0);
        assert_eq!(s[&TextStyle::Small].size, 12.0);
        assert_eq!(s[&TextStyle::Heading].size, 15.0);
        assert_eq!(s[&text_style(style::TITLE)].size, 18.0);
        assert_eq!(
            s[&text_style(style::TITLE)].family,
            FontFamily::Name(DISPLAY.into())
        );
        assert_eq!(s[&TextStyle::Monospace].family, FontFamily::Monospace);
        let large = text_styles(1.25);
        assert_eq!(large[&TextStyle::Body].size, (14.0f32 * 1.25).round());
    }
}
