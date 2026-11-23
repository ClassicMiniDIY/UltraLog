# UltraLog Integration with the OpenECU Spec Corpus

This document describes how UltraLog uses the OpenECU adapter and protocol specifications for channel normalization and metadata.

> **The OpenECU Alliance is retired** (2026-11-22). `oecua.org`, its REST API and the
> `OpenECUAlliance` / `OECUASpecs` repositories are gone or archived. The corpus is vendored in
> this repository at `spec/openecu/` and UltraLog owns it — see CLAUDE.md "OpenECU spec corpus".
> The API client, the disk cache and the background refresh were removed on 2026-11-23, and the
> sections that described them are gone from this document rather than left to mislead.

## Implementation Status

**Adapters (Log File Parsing):**

- [x] Adapter specs embedded at compile time via `include_str!`
- [x] Spec-driven channel normalization from `source_names`
- [x] Integration with existing `normalize.rs` (spec as fallback)
- [x] Channel metadata lookup (min/max/precision/category)
- [x] 10 adapter specs integrated (Haltech, ECUMaster, Link, AiM, RomRaider, Speeduino, rusEFI, Emerald, MegaSquirt, BMW MS4x)
- [x] Corpus vendored in this repository; a clean clone builds offline
- [x] `cargo test --test spec_corpus` validates the corpus against the channel registry
- [ ] Unit conversion applied from `to_canonical` (declared on 30 channels, read by nothing)
- [ ] Per-adapter normalization namespace (see CLAUDE.md, "Source-name collisions")
- [ ] Runtime adapter loading from user directory
- [ ] Generic CSV/binary parser driven by specs
- [ ] Adapter marketplace integration

**Protocols (CAN Bus Real-Time Streaming):**

- [x] Protocol specs embedded at compile time via `include_str!`
- [x] Protocol type definitions (ProtocolSpec, MessageSpec, SignalSpec)
- [x] 12 protocol specs integrated (Haltech, ECUMaster, Speeduino, rusEFI, AEM, MegaSquirt, MaxxECU, Syvecs, Emtron, BMW MS42, BMW MS43, BMW E8x/E9x)
- [x] Protocol registry API (get_protocols, get_protocol_by_id, find_protocols_by_vendor)
- [ ] CAN bus message encoder/decoder
- [ ] Real-time CAN streaming support
- [ ] DBC file export from protocol specs

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        UltraLog App                             │
├─────────────────────────────────────────────────────────────────┤
│  src/parsers/                    src/adapters/                  │
│  ┌─────────────────────┐         ┌─────────────────────────┐   │
│  │  Format-Specific    │         │  Adapter Registry       │   │
│  │  Parsers (existing) │         │  (embedded YAML specs)  │   │
│  │  - haltech.rs       │         │  - haltech-nsp          │   │
│  │  - ecumaster.rs     │ ◄─────► │  - ecumaster-emu-csv    │   │
│  │  - link.rs          │  uses   │  - link-llg             │   │
│  │  - aim.rs           │  for    │  - aim-xrk              │   │
│  │  - romraider.rs     │ metadata│  - romraider-csv        │   │
│  │  - speeduino.rs     │         │  - speeduino-mlg        │   │
│  │  - emerald.rs       │         │  - rusefi-mlg           │   │
│  │  - megasquirt.rs    │         │  - emerald-lg           │   │
│  │  - bmw.rs           │         │  - megasquirt-tunerst.  │   │
│  └─────────────────────┘         │  - bmw-ms4x-romraider   │   │
│                                  └─────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│  src/normalize.rs                                               │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  Normalization Priority:                                    ││
│  │  1. Custom user mappings (highest)                          ││
│  │  2. Built-in hard-coded mappings                            ││
│  │  3. OpenECU Alliance spec source_names (fallback)           ││
│  └─────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│  src/parsers/types.rs - Channel                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  Channel metadata methods:                                  ││
│  │  - display_min() → parser value OR spec min                 ││
│  │  - display_max() → parser value OR spec max                 ││
│  │  - precision() → spec precision (decimal places)            ││
│  │  - category() → spec category (Engine, Fuel, etc.)          ││
│  │  - spec_metadata() → full ChannelMetadata struct            ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── adapters/
│   ├── mod.rs           # Module exports and re-exports
│   ├── types.rs         # AdapterSpec, ProtocolSpec, ChannelSpec, MessageSpec, etc.
│   └── registry.rs      # Spec loading, normalization maps, metadata lookup, protocol registry
├── normalize.rs         # Field normalization (uses adapters for fallback)
└── parsers/
    └── types.rs         # Channel type enhanced with spec metadata methods

spec/openecu/            # Vendored corpus, tracked in this repo (no upstream)
├── adapters/            # Log file format adapters (10 specs)
├── protocols/           # CAN bus protocol definitions (12 specs)
├── channels.yaml        # Canonical channel registry (466 channels) — source of truth
├── SPECIFICATION.md     # Generated from channels.yaml — never hand-edit
└── assets/              # Vendor logos, icons and banners referenced by `branding`
build.rs                 # Downloads nothing; just a rerun-if-changed on spec/openecu
tests/spec_corpus.rs     # Validates the corpus against the registry and the serde types
```

Older builds cached specs under `{app_data_dir}/UltraLog/oecua_specs/`. Nothing reads or
writes that directory now, and no code removes it, so an existing install keeps a few hundred
KB of orphaned JSON there.

## How It Works

### Spec Loading Architecture: One Tier

The specs are compiled into the binary and parsed once on first access. There is no network
call, no disk cache and no refresh, so there is nothing to fall back from:

```text
spec/openecu/**/*.yaml
        |  include_str!  (compile time)
        v
EMBEDDED_ADAPTERS / EMBEDDED_PROTOCOLS
        |  parse once    (LazyLock, first access)
        v
ADAPTER_SPECS, PROTOCOL_SPECS
        |
        +--> SPEC_NORMALIZATION_MAP   source_name -> display name
        +--> CHANNEL_METADATA_MAP     source_name -> ChannelMetadata
```

**Key design principles:**

- **Nothing blocks** — no I/O of any kind on the spec path
- **Always available** — the corpus ships in the binary; offline is the only mode
- **Deterministic** — nothing can replace the specs at runtime, so `EMBEDDED_ADAPTERS` order
  fully determines how a contested `source_names` entry resolves (CLAUDE.md,
  "Source-name collisions")
- **Immutable** — no locks, no generation counter, no cache invalidation hooks

This replaced a three-tier chain (background API fetch → 24-hour disk cache → embedded). It
went when the Alliance did: a lapsed domain would have let whoever registered the name next
put a `Vec<AdapterSpec>` into the disk cache and shadow the vendored corpus for 24 hours on
every start.

### 1. Spec Source: Vendored in This Repository

The corpus lives at `spec/openecu/` and is tracked in git. A plain clone builds offline:

```bash
git clone https://github.com/ClassicMiniDIY/UltraLog.git
cargo build
```

It was never a git submodule. Until 2026-11-22 `build.rs` downloaded 18 files from
`raw.githubusercontent.com/ClassicMiniDIY/OECUASpecs` into a **gitignored**
`spec/OECUASpecs/`, so a clean clone could not build without network access to a repository
that is now archived. `build.rs` no longer downloads anything.

### 2. Spec Loading

```rust
// src/adapters/registry.rs

const HALTECH_NSP_YAML: &str =
    include_str!("../../spec/openecu/adapters/haltech/haltech-nsp.adapter.yaml");
// ... 9 more adapters, 12 protocols

// Order is load-bearing: last wins for a contested source name. BMW sits first so that
// adding it changed no existing mapping. See CLAUDE.md, "Source-name collisions".
static EMBEDDED_ADAPTERS: &[&str] = &[BMW_MS4X_ADAPTER_YAML, HALTECH_NSP_YAML, /* ... */];

static ADAPTER_SPECS: LazyLock<Vec<AdapterSpec>> = LazyLock::new(parse_embedded_adapters);
```

`parse_embedded_adapters` is deliberately lenient at runtime — it logs and skips a spec that
fails to deserialize rather than panicking in a user's session. `cargo test --test spec_corpus`
is what refuses to let a bad spec reach a release.

### 3. Normalization Map Building

A reverse lookup map is built once from all adapter `source_names`:

```rust
// src/adapters/registry.rs

static SPEC_NORMALIZATION_MAP: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| build_normalization_map(&ADAPTER_SPECS));

fn build_normalization_map(adapters: &[AdapterSpec]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for adapter in adapters {
        for channel in &adapter.channels {
            for source_name in &channel.source_names {
                map.insert(source_name.to_lowercase(), channel.name.clone());
            }
        }
    }
    map
}
```

`map.insert` means **last adapter wins** for a source name two adapters both claim, so
`EMBEDDED_ADAPTERS` order decides it. 54 names collide and 43 resolve to different canonical
ids — see CLAUDE.md, "Source-name collisions". Nothing rebuilds this map; there is no writer
in the crate.

### 4. Normalization Integration

The `normalize.rs` module uses spec normalization as a fallback:

```rust
// src/normalize.rs
pub fn normalize_channel_name_with_custom(
    name: &str,
    custom_mappings: Option<&HashMap<String, String>>,
) -> String {
    // 1. Check custom mappings first (highest priority)
    // 2. Check built-in hard-coded mappings
    // 3. Fall back to OpenECU Alliance spec-based normalization
    if let Some(normalized) = adapters::normalize_from_spec(name) {
        return normalized;
    }
    // 4. Return original if no mapping found
    name.to_string()
}
```

### 5. Channel Metadata Access

The `Channel` type provides methods that fall back to spec metadata:

```rust
// src/parsers/types.rs
impl Channel {
    /// Get minimum display value - falls back to spec if parser doesn't provide
    pub fn display_min(&self) -> Option<f64> {
        let parser_min = match self { /* ... */ };
        parser_min.or_else(|| self.spec_metadata().and_then(|m| m.min))
    }

    /// Get precision (decimal places) from spec
    pub fn precision(&self) -> Option<u32> {
        self.spec_metadata().and_then(|m| m.precision)
    }

    /// Get channel category from spec
    pub fn category(&self) -> Option<ChannelCategory> {
        self.spec_metadata().map(|m| m.category)
    }
}
```

## CAN Bus Protocol Support

In addition to adapter specs (for parsing log files), UltraLog also integrates with OpenECU Alliance protocol specifications. These define CAN bus message structures for real-time ECU data streaming.

### Protocol vs Adapter Specs

| Aspect | **Adapters** | **Protocols** |
|--------|-------------|--------------|
| Purpose | Parse saved log files | Real-time CAN bus streaming |
| Format | CSV or binary files | CAN bus messages |
| Use Case | Offline analysis | Live monitoring, dashboards |
| Structure | Columns, headers, timestamps | Messages, signals, bit fields |
| Examples | .csv, .llg, .xrk files | CAN 11-bit/29-bit messages |

### Protocol Specs

UltraLog embeds 9 CAN protocol specifications at compile time:

| Vendor | Protocol ID | Baudrate | Messages | Extended ID |
|--------|-------------|----------|----------|-------------|
| Haltech | haltech-elite-broadcast | 1 Mbps | 50+ | No (11-bit) |
| ECUMaster | ecumaster-emu-broadcast | 1 Mbps | 30+ | No (11-bit) |
| Speeduino | speeduino-broadcast | 500 kbps | 12 | No (11-bit) |
| rusEFI | rusefi-broadcast | 500 kbps | 20+ | No (11-bit) |
| AEM Infinity | aem-infinity-broadcast | 500 kbps | 40+ | Yes (29-bit) |
| Megasquirt | megasquirt-broadcast | 500 kbps | 25+ | Mixed |
| MaxxECU | maxxecu-default | 1 Mbps | 35+ | No (11-bit) |
| Syvecs S7 | syvecs-s7-broadcast | 1 Mbps | 30+ | No (11-bit) |
| Emtron | emtron-broadcast | 1 Mbps | 25+ | No (11-bit) |

### Protocol Structure

Each protocol spec defines:

- **CAN Configuration**: Baudrate, identifier type (11-bit/29-bit), byte order
- **Messages**: CAN message IDs, lengths, broadcast intervals
- **Signals**: Bit-level definitions with start_bit, length, scale, offset
- **Enumerations**: Discrete value mappings (e.g., gear positions)
- **Metadata**: Compatible tools, tested ECU models, known issues

Example protocol signal definition:

```yaml
signals:
  - name: "RPM"
    description: "Engine rotational speed"
    start_bit: 0
    length: 16
    byte_order: big_endian
    data_type: unsigned
    scale: 1.0
    offset: 0
    unit: "rpm"
    min: 0
    max: 65535
```

### Protocol API

```rust
use ultralog::adapters::{
    // Protocol access
    get_protocols,            // Get all loaded protocol specs
    get_protocol_by_id,       // Get specific protocol by ID
    find_protocols_by_vendor, // Get protocols for a vendor

    // Protocol types
    ProtocolSpec,             // Top-level protocol definition
    ProtocolInfo,             // CAN configuration (baudrate, ID type)
    MessageSpec,              // CAN message definition
    SignalSpec,               // Signal within a message
    EnumSpec,                 // Enumeration for discrete values

    // Enums
    ProtocolType,             // can, canfd, lin, k-line
    ByteOrder,                // little_endian, big_endian
    SignalDataType,           // unsigned, signed, float, double
};
```

Example usage:

```rust
// Get all Haltech protocols
let haltech_protos = ultralog::adapters::find_protocols_by_vendor("haltech");
for proto in haltech_protos {
    println!("{}: {} @ {} baud", proto.id, proto.name, proto.protocol.baudrate);
    for msg in &proto.messages {
        println!("  Message 0x{:X}: {}", msg.id, msg.name);
        for signal in &msg.signals {
            println!("    {}: {} {}", signal.name, signal.unit.as_deref().unwrap_or(""), signal.description.as_deref().unwrap_or(""));
        }
    }
}
```

## API Reference

### adapters module

```rust
use ultralog::adapters::{
    // Normalization
    normalize_from_spec,      // Normalize channel name using specs
    has_spec_normalization,   // Check if name has spec normalization
    get_spec_normalizations,  // Iterator over all (source, display) pairs

    // Metadata
    get_channel_metadata,     // Get full ChannelMetadata for a name
    ChannelMetadata,          // Struct with id, name, category, unit, min, max, precision

    // Adapter access
    get_adapters,             // Get all loaded adapters
    get_adapter_by_id,        // Get specific adapter by ID
    get_adapters_by_vendor,   // Get adapters for a vendor
    find_adapters_by_extension, // Find adapters supporting file extension

    // Protocol access
    get_protocols,            // Get all loaded protocol specs
    get_protocol_by_id,       // Get specific protocol by ID
    find_protocols_by_vendor, // Get protocols for a vendor

    // Categories
    get_all_categories,       // Get all unique ChannelCategory values
    get_channels_by_category, // Get all channels for a category

    // Adapter types
    AdapterSpec,
    ChannelSpec,
    ChannelCategory,
    DataType,
    FileFormatSpec,

    // Protocol types
    ProtocolSpec,
    ProtocolInfo,
    ProtocolType,
    MessageSpec,
    SignalSpec,
    ByteOrder,
    SignalDataType,
    EnumSpec,
    UnitConversion,          // to_canonical's {scale, offset} — deserialized, never applied
};
```

### normalize module

```rust
use ultralog::normalize::{
    normalize_channel_name,               // Basic normalization
    normalize_channel_name_with_custom,   // With custom user mappings
    get_display_name,                     // "Normalized (Original)" format
    get_spec_metadata,                    // Get spec metadata for channel
    has_normalization,                    // Check if name can be normalized
    get_builtin_mappings,                 // Get hard-coded mappings
    sort_channels_by_priority,            // Sort with normalized first
};
```

## Adding New Specs

1. Add the adapter YAML under `spec/openecu/adapters/<vendor>/` in this repo
2. Add its channel IDs to `spec/openecu/channels.yaml` if any are new, keeping category
   and canonical unit consistent — `cargo test --test spec_corpus` fails otherwise
3. Add `include_str!` in `src/adapters/registry.rs`
4. Add it to the **front** of the `EMBEDDED_ADAPTERS` array, unless you have checked its
   `source_names` for collisions with the adapters already there: the array's order decides
   which adapter wins a contested name (CLAUDE.md "Source-name collisions")
5. Run `cargo test --test spec_corpus` — it fails if the file is not embedded, and
   validates the spec against the registry
6. Rebuild UltraLog

`build.rs` needs no change; there is no download list any more.

Example adding a new adapter:

```rust
// src/adapters/registry.rs
const NEW_ADAPTER_YAML: &str =
    include_str!("../../spec/openecu/adapters/newvendor/newformat.adapter.yaml");

static EMBEDDED_ADAPTERS: &[&str] = &[
    NEW_ADAPTER_YAML,  // Add at the FRONT: see CLAUDE.md "Source-name collisions"
    BMW_MS4X_ADAPTER_YAML,
    HALTECH_NSP_YAML,
    // ... existing adapters ...
];
```

## Benefits

1. **Zero Runtime I/O**: Specs are compiled into the binary
2. **Fast Startup**: LazyLock defers parsing until first use
3. **Type Safety**: Rust types match YAML schema
4. **Comprehensive Metadata**: min/max/precision/category available for all spec channels
5. **Backwards Compatible**: Existing parsers unchanged, spec is additive
6. **Cross-Tool**: Same specs work for any OpenECU Alliance-compatible tool

## Future Enhancements

### Runtime Adapter Loading (User Directory)

Load user-created adapters from local directory (not yet implemented):

```rust
impl AdapterRegistry {
    pub fn load_user_adapters(path: &Path) -> Result<Vec<AdapterSpec>> {
        // Read YAML files from ~/.ultralog/adapters/
        // Merge with embedded/cached adapters
        // Allow users to test custom adapters without rebuilding
    }
}
```

**Note:** there is no longer any runtime spec update path — the corpus is compiled in and
this repository is its only source. That removes the original reason for deferring this: a
user directory is now the *only* way to try an adapter without a rebuild, which makes it a
stronger candidate than it was, not a weaker one.

### Generic CSV Parser

Use adapter specs to drive parsing without format-specific code:

```rust
pub struct GenericCsvParser {
    adapter: AdapterSpec,
}

impl Parseable for GenericCsvParser {
    fn parse(&self, data: &str) -> Result<Log, Box<dyn Error>> {
        // Use adapter.file_format for delimiter, header row, etc.
        // Use adapter.channels for column mapping
        // Apply conversion expressions
    }
}
```

### Unit Conversion from Specs

The corpus already declares this and nothing reads it. A channel whose log unit differs from
the canonical one in `channels.yaml` carries `to_canonical`, and `ChannelSpec::to_canonical`
deserializes it:

```yaml
channels:
  - id: fuel_flow
    unit: cc/min # what the log actually contains — never rewritten to look canonical
    to_canonical: # canonical = raw * scale + offset
      scale: 0.06
      offset: 0
```

30 adapter channels and 5 BMW protocol signals declare it. `conversion` and `source_unit`
exist on `ChannelSpec` too and have never been read either, so UltraLog displays raw log
units throughout. Applying it changes displayed values across nine adapters and wants
checking against real logs.

## Open Questions

1. **Binary formats**: How much can be spec-driven?
   - Current approach: Specs provide metadata, parsers handle binary format details

2. **User adapter validation**: How to validate user-created adapters?
   - Use JSON Schema validation before loading

3. **Adapter updates**: How to handle spec version changes?
   - Include adapter version in metadata, warn on breaking changes
