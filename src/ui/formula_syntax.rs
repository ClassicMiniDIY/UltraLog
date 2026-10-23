//! Syntax colouring for computed-channel formulas
//! (`design/design-system/components/FormulaEditor/README.md`).
//!
//! Roles: channel names `cyan`, numbers and constants `gold`, functions `violet`, time shifts
//! (`@-0.1s`, `[-1]`) `orange`, operators `ink_muted`. Everything is set in mono, so a token's
//! role also reads from its shape (quotes, brackets, `@`, a following parenthesis), never from
//! colour alone.

use std::ops::Range;

use eframe::egui::{Color32, FontId, TextFormat, text::LayoutJob};

use crate::ui::theme::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Channel,
    Number,
    Function,
    TimeShift,
    Operator,
    Space,
}

const CONSTANTS: [&str; 4] = ["pi", "e", "tau", "phi"];

/// Split `text` into coloured runs. Unknown characters fall into `Operator`.
pub fn tokens(text: &str) -> Vec<(Range<usize>, Role)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let c = text[i..].chars().next().unwrap_or(' ');
        let start = i;
        let role = if c.is_whitespace() {
            i += c.len_utf8();
            while let Some(n) = text[i..].chars().next().filter(|n| n.is_whitespace()) {
                i += n.len_utf8();
            }
            Role::Space
        } else if c == '"' {
            // Quoted channel name, up to the closing quote (or the end while typing).
            i += 1;
            while i < text.len() && bytes[i] != b'"' {
                i += text[i..].chars().next().map_or(1, char::len_utf8);
            }
            i = (i + 1).min(text.len());
            Role::Channel
        } else if c == '@' {
            // Time offset: @-0.1s, @0.5s
            i += 1;
            if i < text.len() && matches!(bytes[i], b'-' | b'+') {
                i += 1;
            }
            while i < text.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < text.len() && bytes[i] == b's' {
                i += 1;
            }
            Role::TimeShift
        } else if c == '[' {
            // Index offset: [-1]
            i += 1;
            while i < text.len() && bytes[i] != b']' && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < text.len() && bytes[i] == b']' {
                i += 1;
            }
            Role::TimeShift
        } else if c.is_ascii_digit() || (c == '.' && next_is_digit(bytes, i + 1)) {
            i += 1;
            while i < text.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            // Exponent: 1e-3
            if i + 1 < text.len()
                && matches!(bytes[i], b'e' | b'E')
                && (bytes[i + 1].is_ascii_digit()
                    || (matches!(bytes[i + 1], b'-' | b'+') && next_is_digit(bytes, i + 2)))
            {
                i += 2;
                while i < text.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            Role::Number
        } else if c.is_alphabetic() || c == '_' {
            while let Some(n) = text[i..]
                .chars()
                .next()
                .filter(|n| n.is_alphanumeric() || *n == '_')
            {
                i += n.len_utf8();
            }
            let word = &text[start..i];
            let rest = text[i..].trim_start();
            if rest.starts_with('(') {
                Role::Function
            } else if CONSTANTS.contains(&word) {
                Role::Number
            } else {
                Role::Channel
            }
        } else {
            i += c.len_utf8();
            Role::Operator
        };
        out.push((start..i, role));
    }
    out
}

fn next_is_digit(bytes: &[u8], at: usize) -> bool {
    bytes.get(at).is_some_and(u8::is_ascii_digit)
}

/// The colour of a role.
pub fn color(role: Role, t: &Tokens) -> Color32 {
    match role {
        Role::Channel => t.cyan,
        Role::Number => t.gold,
        Role::Function => t.violet,
        Role::TimeShift => t.orange,
        Role::Operator | Role::Space => t.ink_muted,
    }
}

/// A syntax-coloured layout of `text` for a `TextEdit` layouter.
pub fn layout(text: &str, font: FontId, t: &Tokens, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (range, role) in tokens(text) {
        job.append(
            &text[range],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: color(role, t),
                ..Default::default()
            },
        );
    }
    job.wrap.max_width = wrap_width;
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(text: &str) -> Vec<(&str, Role)> {
        tokens(text)
            .into_iter()
            .filter(|(_, r)| *r != Role::Space)
            .map(|(range, r)| (&text[range], r))
            .collect()
    }

    #[test]
    fn colours_each_role_of_the_reference_formula() {
        let f = "\"Manifold Pressure\" - \"Manifold Pressure\"@-0.1s * 10 / abs(RPM[-1])";
        assert_eq!(
            roles(f),
            vec![
                ("\"Manifold Pressure\"", Role::Channel),
                ("-", Role::Operator),
                ("\"Manifold Pressure\"", Role::Channel),
                ("@-0.1s", Role::TimeShift),
                ("*", Role::Operator),
                ("10", Role::Number),
                ("/", Role::Operator),
                ("abs", Role::Function),
                ("(", Role::Operator),
                ("RPM", Role::Channel),
                ("[-1]", Role::TimeShift),
                (")", Role::Operator),
            ]
        );
    }

    #[test]
    fn constants_exponents_and_stat_variables() {
        assert_eq!(
            roles("_mean_RPM * pi + 1e-3"),
            vec![
                ("_mean_RPM", Role::Channel),
                ("*", Role::Operator),
                ("pi", Role::Number),
                ("+", Role::Operator),
                ("1e-3", Role::Number),
            ]
        );
    }

    #[test]
    fn covers_every_byte_even_while_typing() {
        for text in ["\"Unclosed", "RPM@-", "abs(", "[", "ö + 1", ""] {
            let total: usize = tokens(text).iter().map(|(r, _)| r.len()).sum();
            assert_eq!(total, text.len(), "{text:?}");
        }
    }
}
