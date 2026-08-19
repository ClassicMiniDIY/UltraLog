pub mod aim;
pub mod bluedriver;
pub mod dynamicefi;
pub mod ecumaster;
pub mod emerald;
pub mod haltech;
pub mod link;
pub mod locomotive;
pub mod megasquirt;
pub mod mhd;
pub mod motorsport_electronics;
pub mod racechrono;
pub mod romraider;
pub mod speeduino;
pub mod types;
pub mod woolich;

pub use aim::Aim;
pub use bluedriver::BlueDriver;
pub use dynamicefi::DynamicEfi;
pub use ecumaster::EcuMaster;
pub use emerald::Emerald;
pub use haltech::Haltech;
pub use link::Link;
pub use locomotive::Locomotive;
pub use megasquirt::MegaSquirt;
pub use mhd::Mhd;
pub use motorsport_electronics::MotorsportElectronics;
pub use racechrono::RaceChrono;
pub use romraider::RomRaider;
pub use speeduino::Speeduino;
pub use types::{Channel, EcuType, Log, Parseable, Value};
pub use woolich::Woolich;

/// Drop a leading block of `#`-prefixed comment lines so the real CSV header
/// row is the first line format detection sees.
///
/// Several exporters (MHD Tuning, OBDLink and similar OBD-II apps) write a
/// metadata preamble ahead of the header row. Without this the header is
/// never found and the log loads with zero channels. Content that does not
/// start with a comment block is returned unchanged.
///
/// Note: a parser whose own fingerprint lives in that preamble must run its
/// detection against the raw text, before this is applied.
pub fn strip_leading_comment_lines(contents: &str) -> &str {
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        if line.trim_start().starts_with('#') {
            offset += line.len();
        } else {
            break;
        }
    }
    &contents[offset..]
}

#[cfg(test)]
mod tests {
    use super::strip_leading_comment_lines;

    #[test]
    fn strips_leading_comment_block() {
        let contents = "#Encoding: UTF-8\n#VIN: ABC\nTime,RPM\n0,1000\n";
        assert_eq!(strip_leading_comment_lines(contents), "Time,RPM\n0,1000\n");
    }

    #[test]
    fn leaves_content_without_comments_unchanged() {
        let contents = "Time,RPM\n0,1000\n";
        assert_eq!(strip_leading_comment_lines(contents), contents);
    }

    #[test]
    fn only_strips_the_leading_block() {
        // A '#' appearing after the header row is data, not a preamble.
        let contents = "Time,RPM\n#not a preamble\n0,1000\n";
        assert_eq!(strip_leading_comment_lines(contents), contents);
    }

    #[test]
    fn handles_all_comment_input() {
        assert_eq!(strip_leading_comment_lines("#only\n#comments\n"), "");
    }
}
