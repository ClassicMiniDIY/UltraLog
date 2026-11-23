//! Validates the vendored OpenECU spec corpus in `spec/openecu` against the canonical
//! channel registry (`spec/openecu/channels.yaml`) and against the serde types that
//! actually consume it.
//!
//! This is a port of `scripts/validate-specs.ts` from the retired OpenECUAlliance
//! repository, which gated that repo's deploy. It is not decoration: on its first run
//! there it found two real bugs nothing else had — a signal overflowing the rusEFI
//! status frame and an overlapping pair in Haltech `0x477`. The corpus had drifted for
//! 19 months with nobody noticing that two protocols shipped an unusable baud rate, one
//! shipped message IDs that match no real bus, and eight canonical IDs carried mutually
//! incompatible units. Every one of those is mechanically detectable.
//!
//! Upstream ran two layers: the zod schemas in `server/schemas/{adapter,protocol}.ts`,
//! and the hand-written registry checks. Deserializing into `AdapterSpec` / `ProtocolSpec`
//! stands in for most of the schema layer, but the serde types are deliberately permissive
//! — `#[serde(other)]` enum fallbacks, `#[serde(default)]`, plain `Vec` and `String` where
//! zod had bounds and enums — so the bounds zod enforced are re-stated explicitly below:
//! non-empty collections, the `csv`/`binary` and `little`/`big` enums, CAN frame and signal
//! length ranges, and the signal fields zod made mandatory. Without them an adapter with
//! `extensions: []` (dead: matches no file) or `source_names: []` (dead: normalises
//! nothing) passed silently, and an out-of-range `message.length` widened the frame enough
//! to defeat the overflow check below it.
//!
//! Still unenforced, because the Rust types make them unreachable rather than merely
//! unchecked: `type: literal('protocol')` and the `website` URL format. `byte_order` and
//! `magic_bytes` are enforced more strictly here than upstream — `ByteOrder` has no
//! `#[serde(other)]`, and `Option<Vec<u8>>` rejects a byte above 255.
//!
//! NOT ported: the version-bump check. Upstream it compared each spec against
//! `origin/main` and required a version bump plus a matching changelog entry, because
//! external consumers pinned versions over an API. There is no API and no external
//! consumer now — the corpus and the only code that reads it change in the same commit.
//!
//! Errors fail the test. Warnings print and pass: a warning marks something a human
//! recorded on purpose, most importantly `disputed:` (see below).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_norway::Value;
use ultralog::adapters::types::{AdapterSpec, ProtocolSpec};

/// CAN rates that exist on a real bus. Upstream shipped protocols with values outside
/// this set, which is how a spec that could never have been captured from hardware got
/// published.
const PLAUSIBLE_BAUD: &[u64] = &[125_000, 250_000, 500_000, 800_000, 1_000_000];

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/openecu")
}

#[derive(Default)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report {
    fn err(&mut self, file: &str, msg: impl AsRef<str>) {
        self.errors.push(format!("{file}: {}", msg.as_ref()));
    }
    fn warn(&mut self, file: &str, msg: impl AsRef<str>) {
        self.warnings.push(format!("{file}: {}", msg.as_ref()));
    }
}

/// Every `*.yaml` under `dir`, recursively, sorted so failures are reproducible.
fn walk_yaml(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {}: {e}", d.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Corpus-relative path, always with `/` separators. `Path::display()` yields `\` on
/// Windows, and `every_corpus_file_is_embedded` substring-matches this against the
/// forward-slash paths in `registry.rs`'s `include_str!` calls — so without the
/// normalisation that test reports every spec as unembedded on a Windows checkout. CI
/// only runs tests on Linux, so it would have failed for a contributor, not for us.
fn rel(path: &Path) -> String {
    path.strip_prefix(corpus_root())
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn load(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_norway::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|x| !x.is_null())
}

fn as_str(v: Option<&Value>) -> Option<&str> {
    v.and_then(|x| x.as_str())
}

fn seq<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    get(v, key)
        .and_then(|x| x.as_sequence())
        .map_or(&[], |s| s.as_slice())
}

/// A canonical channel, as the registry declares it.
struct Canonical {
    category: String,
    unit: String,
}

struct Registry {
    canonical: HashMap<String, Canonical>,
    /// alias -> canonical id. 20 of these exist; they resolve but are deprecated.
    alias_of: HashMap<String, String>,
    /// Raw unit spelling -> normalised spelling. Matched exactly, not case-folded,
    /// because that is what the registry's keys do (`kPa` and `km/h` are mixed case).
    spellings: HashMap<String, String>,
    /// `from->to` pairs the registry knows how to convert, for a better error message.
    conversions: HashSet<String>,
}

impl Registry {
    fn load() -> Self {
        let raw = load(&corpus_root().join("channels.yaml"));
        let mut canonical = HashMap::new();
        let mut alias_of = HashMap::new();
        for ch in seq(&raw, "channels") {
            let Some(id) = as_str(get(ch, "id")) else {
                continue;
            };
            canonical.insert(
                id.to_string(),
                Canonical {
                    category: as_str(get(ch, "category")).unwrap_or_default().to_string(),
                    unit: as_str(get(ch, "unit")).unwrap_or_default().to_string(),
                },
            );
            for alias in seq(ch, "aliases") {
                if let Some(a) = alias.as_str() {
                    alias_of.insert(a.to_string(), id.to_string());
                }
            }
        }

        let spellings = get(&raw, "unit_spellings")
            .and_then(|v| v.as_mapping())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let conversions = seq(&raw, "conversions")
            .iter()
            .filter_map(|c| {
                Some(format!(
                    "{}->{}",
                    as_str(get(c, "from"))?,
                    as_str(get(c, "to"))?
                ))
            })
            .collect();

        assert!(
            !canonical.is_empty(),
            "channels.yaml declared no channels — the registry failed to load"
        );
        Self {
            canonical,
            alias_of,
            spellings,
            conversions,
        }
    }

    /// The canonical id for `id`, whether `id` is canonical or a deprecated alias.
    fn resolve(&self, id: &str) -> Option<String> {
        if self.canonical.contains_key(id) {
            Some(id.to_string())
        } else {
            self.alias_of.get(id).cloned()
        }
    }

    fn norm_unit(&self, unit: &str) -> String {
        let trimmed = unit.trim();
        self.spellings
            .get(trimmed)
            .cloned()
            .unwrap_or_else(|| trimmed.to_string())
    }
}

/// Registry conformance for one adapter's channels, read off the raw YAML rather than
/// the typed struct: the serde types fall back to `Unknown` and drop fields they do not
/// model, so a check against them cannot see a category that is well-formed but wrong.
fn check_adapter(reg: &Registry, file: &str, raw: &Value, rep: &mut Report) {
    // Bounds zod enforced that the serde types do not (see the module comment).
    if seq(raw, "channels").is_empty() {
        rep.err(file, "declares no channels");
    }
    match get(raw, "file_format") {
        None => rep.err(file, "has no 'file_format'"),
        Some(ff) => {
            if seq(ff, "extensions").is_empty() {
                rep.err(
                    file,
                    "file_format.extensions is empty — this adapter can never match a file",
                );
            }
            match as_str(get(ff, "type")) {
                Some("csv") | Some("binary") => {}
                other => rep.err(
                    file,
                    format!(
                        "file_format.type is {:?}, must be 'csv' or 'binary'",
                        other.unwrap_or("<missing>")
                    ),
                ),
            }
            if let Some(e) = as_str(get(ff, "endianness"))
                && e != "little"
                && e != "big"
            {
                rep.err(
                    file,
                    format!("file_format.endianness is '{e}', must be 'little' or 'big'"),
                );
            }
        }
    }

    let mut seen = HashSet::new();
    for ch in seq(raw, "channels") {
        let Some(id) = as_str(get(ch, "id")) else {
            rep.err(file, "a channel has no 'id'");
            continue;
        };
        if !seen.insert(id.to_string()) {
            rep.err(file, format!("duplicate channel id '{id}'"));
        }

        if seq(ch, "source_names").is_empty() {
            rep.err(
                file,
                format!(
                    "channel '{id}' has no source_names — it can never normalise \
                     a column, so it is dead weight"
                ),
            );
        }

        let Some(canon_id) = reg.resolve(id) else {
            rep.err(
                file,
                format!(
                    "channel '{id}' is not in the canonical registry \
                     (add it to channels.yaml or use an existing id)"
                ),
            );
            continue;
        };
        if canon_id != id {
            rep.warn(
                file,
                format!("channel '{id}' is a deprecated alias for '{canon_id}'"),
            );
        }
        let canon = &reg.canonical[&canon_id];

        let category = as_str(get(ch, "category")).unwrap_or_default();
        if category != canon.category {
            rep.err(
                file,
                format!(
                    "channel '{id}' has category '{category}', registry says '{}'",
                    canon.category
                ),
            );
        }

        let declared = as_str(get(ch, "unit"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let unit = reg.norm_unit(&declared);
        if unit != declared {
            rep.warn(
                file,
                format!("channel '{id}' unit '{declared}' should be spelled '{unit}'"),
            );
        }

        // The log unit is allowed to differ from the canonical one — it describes what
        // the file actually contains. What is not allowed is differing in silence: the
        // channel must carry the conversion so a consumer can normalise without
        // knowing the vendor.
        let conv = get(ch, "to_canonical");
        let matches_canonical = unit == canon.unit;

        if let Some(conv) = conv {
            if matches_canonical {
                rep.warn(
                    file,
                    format!(
                        "channel '{id}' declares 'to_canonical' but already uses the canonical unit"
                    ),
                );
            } else if get(conv, "scale").and_then(Value::as_f64).is_none()
                || get(conv, "offset").and_then(Value::as_f64).is_none()
            {
                rep.err(
                    file,
                    format!(
                        "channel '{id}' has a malformed 'to_canonical' \
                         (needs numeric scale and offset)"
                    ),
                );
            }
        } else if !matches_canonical {
            let hint = if reg.conversions.contains(&format!("{unit}->{}", canon.unit)) {
                format!(
                    " (a {unit}->{} conversion exists in the registry)",
                    canon.unit
                )
            } else {
                String::new()
            };
            rep.err(
                file,
                format!(
                    "channel '{id}' is in '{unit}' but canonical is '{}' \
                     and no 'to_canonical' is declared{hint}",
                    canon.unit
                ),
            );
        }
    }
}

/// CAN plausibility and bit-layout integrity for one protocol.
fn check_protocol(reg: &Registry, file: &str, raw: &Value, rep: &mut Report) {
    let baud = get(raw, "protocol")
        .and_then(|p| get(p, "baudrate"))
        .and_then(Value::as_u64);
    match baud {
        Some(b) if PLAUSIBLE_BAUD.contains(&b) => {}
        Some(b) => rep.err(file, format!("implausible CAN baudrate {b}")),
        None => rep.err(file, "protocol.baudrate is missing or not a number"),
    }

    let extended = get(raw, "protocol")
        .and_then(|p| get(p, "extended_id").or_else(|| get(p, "extendedId")))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_id: u64 = if extended { 0x1FFF_FFFF } else { 0x7FF };
    let id_bits = if extended { 29 } else { 11 };

    // A CAN frame carries at most 8 data bytes; CAN FD raises that to 64. `frame_bits`
    // below is derived from `length`, so an out-of-range value does not just misdescribe
    // the frame — it widens it enough that the overflow check can no longer fire.
    let fd = get(raw, "protocol")
        .and_then(|p| get(p, "fd_enabled").or_else(|| get(p, "fdEnabled")))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || get(raw, "protocol")
            .and_then(|p| get(p, "type"))
            .and_then(Value::as_str)
            == Some("canfd");
    let max_frame_bytes: u64 = if fd { 64 } else { 8 };

    if seq(raw, "messages").is_empty() {
        rep.err(file, "declares no messages");
    }

    let mut seen_ids = HashSet::new();
    for msg in seq(raw, "messages") {
        let Some(id) = message_id(get(msg, "id")) else {
            rep.err(file, "a message has a missing or unparseable 'id'");
            continue;
        };
        let hex = format!("0x{id:X}");
        if !seen_ids.insert(id) {
            rep.err(file, format!("duplicate message id {hex}"));
        }
        if id > max_id {
            rep.err(
                file,
                format!("message {hex} exceeds the {id_bits}-bit id range"),
            );
        }

        let declared_len = get(msg, "length").and_then(Value::as_u64);
        if let Some(len) = declared_len
            && len > max_frame_bytes
        {
            rep.err(
                file,
                format!(
                    "message {hex} declares length {len}, above the {max_frame_bytes}-byte \
                     maximum for this bus — a frame this wide also disables the \
                     signal-overflow check"
                ),
            );
        }
        // Clamp, so one bad length cannot mask overflow in the signals below it.
        let frame_bits = declared_len.unwrap_or(8).min(max_frame_bytes) * 8;

        if seq(msg, "signals").is_empty() {
            rep.warn(file, format!("message {hex} declares no signals"));
        }

        // (start, end_inclusive, name, disputed_reason)
        let mut spans: Vec<(u64, u64, String, Option<String>)> = Vec::new();
        for sig in seq(msg, "signals") {
            let name = as_str(get(sig, "name")).unwrap_or("<unnamed>").to_string();
            let start = get(sig, "start_bit")
                .or_else(|| get(sig, "startBit"))
                .and_then(Value::as_u64);
            let len = get(sig, "length").and_then(Value::as_u64);
            let (Some(start), Some(len)) = (start, len) else {
                rep.err(
                    file,
                    format!("signal '{name}' in {hex} is missing start_bit/length"),
                );
                continue;
            };
            // zod bounded this at 1..=64; nothing in the Rust types does, and `len == 0`
            // would underflow the span end below.
            if len == 0 || len > 64 {
                rep.err(
                    file,
                    format!("signal '{name}' in {hex} has length {len}, must be 1..=64"),
                );
                continue;
            }
            if start + len > frame_bits {
                rep.err(file, format!("signal '{name}' overflows frame {hex}"));
            }
            // zod made these mandatory; serde defaults scale to 1.0 and offset to 0.0, so
            // a spec that omits them silently gets an identity scaling.
            for field in ["unit", "scale", "offset"] {
                if get(sig, field).is_none() {
                    rep.err(
                        file,
                        format!("signal '{name}' in {hex} is missing required '{field}'"),
                    );
                }
            }
            if get(sig, "byte_order").is_none() && get(sig, "byteOrder").is_none() {
                rep.err(
                    file,
                    format!("signal '{name}' in {hex} is missing required 'byte_order'"),
                );
            }
            let disputed = as_str(get(sig, "disputed")).map(str::to_string);
            spans.push((start, start + len - 1, name.clone(), disputed));

            // Every signal must be accounted for: mapped to a canonical id, marked as
            // padding, or explicitly declared vendor-specific. Silence is the failure
            // mode being removed — an unmarked signal is one nobody has looked at.
            let reserved = get(sig, "reserved")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let vendor_specific = get(sig, "vendor_specific")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            match as_str(get(sig, "id")) {
                Some(sig_id) => {
                    if reg.resolve(sig_id).is_none() {
                        rep.err(
                            file,
                            format!(
                                "signal '{name}' has id '{sig_id}' which is not in the registry"
                            ),
                        );
                    } else if reserved || vendor_specific {
                        rep.err(
                            file,
                            format!(
                                "signal '{name}' has a canonical id and is also marked \
                                 reserved/vendor_specific"
                            ),
                        );
                    }
                }
                None if !reserved && !vendor_specific => rep.warn(
                    file,
                    format!(
                        "signal '{name}' in {hex} has no canonical id and is not marked \
                         reserved or vendor_specific"
                    ),
                ),
                None => {}
            }
        }

        spans.sort_by_key(|s| s.0);
        for pair in spans.windows(2) {
            let (prev, cur) = (&pair[0], &pair[1]);
            if cur.0 <= prev.1 {
                let msg = format!("signals '{}' and '{}' overlap in {hex}", prev.2, cur.2);
                // A signal may carry `disputed` when the vendor's own documentation is
                // self-contradictory. That records the conflict instead of hiding it
                // behind a guess, so it degrades to a warning rather than failing.
                // Use it to record a conflict, never to silence one you could resolve.
                match cur.3.as_deref().or(prev.3.as_deref()) {
                    Some(reason) => rep.warn(file, format!("{msg} — disputed: {reason}")),
                    None => rep.err(file, msg),
                }
            }
        }
    }
}

/// Message ids appear as decimal integers or as hex strings like `"0x360"`.
fn message_id(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    let s = v.as_str()?.trim();
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    match hex {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => s.parse().ok().or_else(|| u64::from_str_radix(s, 16).ok()),
    }
}

#[test]
fn corpus_is_valid() {
    let root = corpus_root();
    let reg = Registry::load();
    let mut rep = Report::default();

    let adapters = walk_yaml(&root.join("adapters"));
    for path in &adapters {
        let file = rel(path);
        let raw = load(path);

        // The serde types are the real consumer contract: if the corpus does not
        // deserialize into them, the embedded tier is broken whatever the YAML says.
        match serde_norway::from_value::<AdapterSpec>(raw.clone()) {
            Ok(spec) => {
                for field in spec.unknown_fields() {
                    rep.err(&file, format!("unknown enum value at {field}"));
                }
            }
            Err(e) => rep.err(&file, format!("does not deserialize as AdapterSpec: {e}")),
        }

        check_adapter(&reg, &file, &raw, &mut rep);
    }

    let protocols = walk_yaml(&root.join("protocols"));
    for path in &protocols {
        let file = rel(path);
        let raw = load(path);

        match serde_norway::from_value::<ProtocolSpec>(raw.clone()) {
            Ok(spec) => {
                for field in spec.unknown_fields() {
                    rep.err(&file, format!("unknown enum value at {field}"));
                }
            }
            Err(e) => rep.err(&file, format!("does not deserialize as ProtocolSpec: {e}")),
        }

        check_protocol(&reg, &file, &raw, &mut rep);
    }

    for w in &rep.warnings {
        println!("warn  {w}");
    }
    println!(
        "{} adapters, {} protocols, {} canonical channels — {} errors, {} warnings",
        adapters.len(),
        protocols.len(),
        reg.canonical.len(),
        rep.errors.len(),
        rep.warnings.len()
    );

    assert!(
        rep.errors.is_empty(),
        "spec corpus has {} error(s):\n{}",
        rep.errors.len(),
        rep.errors.join("\n")
    );
}

/// The corpus the binary embeds must be the corpus on disk. `registry.rs` lists every
/// file by hand in `include_str!`, so a file added to `spec/openecu` without a matching
/// entry is invisible at runtime — exactly how UltraLog ended up a release behind
/// upstream, missing one adapter and three protocols.
#[test]
fn every_corpus_file_is_embedded() {
    let root = corpus_root();
    let registry_src =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapters/registry.rs"))
            .expect("read registry.rs");

    let mut missing = Vec::new();
    for path in walk_yaml(&root.join("adapters"))
        .into_iter()
        .chain(walk_yaml(&root.join("protocols")))
    {
        let needle = rel(&path);
        if !registry_src.contains(&needle) {
            missing.push(needle);
        }
    }

    assert!(
        missing.is_empty(),
        "these spec files are not embedded by src/adapters/registry.rs:\n{}",
        missing.join("\n")
    );
}

/// `SPECIFICATION.md` is generated from `channels.yaml` and the generator
/// (`generate-spec-doc.ts`) did not survive the OpenECU Alliance retirement — it was Bun
/// and this is a Rust repo. So the document can no longer be regenerated, only kept in
/// step by hand, which means it can silently go stale.
///
/// This is the guard: the registry's own headline numbers must match what the document
/// claims, and every canonical id must appear in it. It does not prove the document is
/// correct, only that nobody added or removed a channel without touching it.
#[test]
fn specification_doc_is_not_stale() {
    let root = corpus_root();
    let reg = Registry::load();
    let doc = fs::read_to_string(root.join("SPECIFICATION.md")).expect("read SPECIFICATION.md");

    let raw = load(&root.join("channels.yaml"));
    let categories: HashSet<&str> = seq(&raw, "channels")
        .iter()
        .filter_map(|c| as_str(get(c, "category")))
        .collect();

    let claim = format!(
        "{} canonical channels across {} categories",
        reg.canonical.len(),
        categories.len()
    );
    assert!(
        doc.contains(&claim),
        "SPECIFICATION.md does not claim \"{claim}\" — channels.yaml has \
         {} channels in {} categories, so the document is out of step. There is no \
         generator any more: update the document by hand.",
        reg.canonical.len(),
        categories.len()
    );

    let undocumented: Vec<&str> = reg
        .canonical
        .keys()
        .map(String::as_str)
        .filter(|id| !doc.contains(&format!("`{id}`")))
        .collect();
    assert!(
        undocumented.is_empty(),
        "{} canonical channel id(s) are in channels.yaml but absent from \
         SPECIFICATION.md: {}",
        undocumented.len(),
        undocumented.join(", ")
    );
}

/// `to_canonical` carries the unit-conversion mandate and must survive deserialization.
/// It was dropped silently until it was added to `ChannelSpec`; nothing applies it yet,
/// so this test guards the data, not the behaviour.
#[test]
fn to_canonical_survives_deserialization() {
    let mut declared_in_yaml = 0usize;
    let mut parsed_into_type = 0usize;

    for path in walk_yaml(&corpus_root().join("adapters")) {
        let raw = load(&path);
        declared_in_yaml += seq(&raw, "channels")
            .iter()
            .filter(|ch| get(ch, "to_canonical").is_some())
            .count();

        let spec: AdapterSpec =
            serde_norway::from_value(raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        parsed_into_type += spec
            .channels
            .iter()
            .filter(|ch| ch.to_canonical.is_some())
            .count();
    }

    assert!(
        declared_in_yaml > 0,
        "no channel declares 'to_canonical' — the corpus or this test is wrong"
    );
    assert_eq!(
        declared_in_yaml, parsed_into_type,
        "{declared_in_yaml} channels declare 'to_canonical' but only {parsed_into_type} \
         reached ChannelSpec — the field is being dropped again"
    );

    // Protocol signals carry it too, which the first pass at this missed entirely.
    let mut sig_declared = 0usize;
    let mut sig_parsed = 0usize;
    for path in walk_yaml(&corpus_root().join("protocols")) {
        let raw = load(&path);
        sig_declared += seq(&raw, "messages")
            .iter()
            .flat_map(|m| seq(m, "signals"))
            .filter(|s| get(s, "to_canonical").is_some())
            .count();

        let spec: ProtocolSpec =
            serde_norway::from_value(raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        sig_parsed += spec
            .messages
            .iter()
            .flat_map(|m| &m.signals)
            .filter(|s| s.to_canonical.is_some())
            .count();
    }

    assert!(
        sig_declared > 0,
        "no protocol signal declares 'to_canonical' — the corpus or this test is wrong"
    );
    assert_eq!(
        sig_declared, sig_parsed,
        "{sig_declared} protocol signals declare 'to_canonical' but only {sig_parsed} \
         reached SignalSpec — the field is being dropped again"
    );
}
