# UltraLog Integration with the OpenECU Spec Corpus

This document describes how UltraLog uses the OpenECU adapter and protocol specifications for channel normalization and metadata.

> **Stale sections (2026-11-22).** The OpenECU Alliance was retired: `oecua.org`, its REST
> API and the `OpenECUAlliance` / `OECUASpecs` repositories are gone or archived. The corpus
> is now vendored in this repository at `spec/openecu/` and UltraLog owns it — see CLAUDE.md
> "OpenECU spec corpus". The spec-source and "Adding New Specs" sections below are corrected.
> Every section describing the **API tier, background refresh or disk cache still describes
> code that is being removed**; treat it as history until that change lands.

## Implementation Status

**Adapters (Log File Parsing):**

- [x] Adapter specs embedded at compile time via `include_str!`
- [x] Spec-driven channel normalization from `source_names`
- [x] Integration with existing `normalize.rs` (spec as fallback)
- [x] Channel metadata lookup (min/max/precision/category)
- [x] 8 adapter specs integrated (Haltech, ECUMaster, Link, AiM, RomRaider, Speeduino, rusEFI, Emerald)
- [x] **API client for fetching specs from openecualliance.org**
- [x] **Local disk cache at `{app_data_dir}/UltraLog/oecua_specs/`**
- [x] **Background API refresh with fallback chain (cache → embedded → API)**
- [x] **24-hour cache staleness threshold**
- [ ] Runtime adapter loading from user directory
- [ ] Generic CSV/binary parser driven by specs
- [ ] Adapter marketplace integration

**Protocols (CAN Bus Real-Time Streaming):**

- [x] Protocol specs embedded at compile time via `include_str!`
- [x] Protocol type definitions (ProtocolSpec, MessageSpec, SignalSpec)
- [x] 9 protocol specs integrated (Haltech, ECUMaster, Speeduino, rusEFI, AEM, Megasquirt, MaxxECU, Syvecs, Emtron)
- [x] Protocol registry API (get_protocols, get_protocol_by_id, find_protocols_by_vendor)
- [x] **API client for fetching protocol specs**
- [x] **Local disk cache for protocols**
- [x] **Background API refresh for protocols**
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
│  └─────────────────────┘         │  - emerald-lg           │   │
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
│   ├── registry.rs      # Spec loading, normalization maps, metadata lookup, protocol registry
│   ├── api.rs           # API client for fetching specs from openecualliance.org
│   └── cache.rs         # Local disk cache at {app_data_dir}/UltraLog/oecua_specs/
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

Cache locations:
- Linux: ~/.local/share/UltraLog/oecua_specs/
- macOS: ~/Library/Application Support/UltraLog/oecua_specs/
- Windows: %APPDATA%\UltraLog\oecua_specs\
```

## How It Works

### Spec Loading Architecture: Multi-Tier Fallback

UltraLog uses a **three-tier fallback system** for loading adapter and protocol specifications, optimizing for fast startup while keeping specs up-to-date:

```
┌─────────────────────────────────────────────────────────┐
│                    Application Startup                   │
├─────────────────────────────────────────────────────────┤
│  1. Load specs via registry.rs (FAST, non-blocking)     │
│     ┌───────────────────────────────────────────────┐   │
│     │ Check cache freshness (< 24 hours?)          │   │
│     │   ├── YES → Load from disk cache (fastest)   │   │
│     │   └── NO → Load embedded YAML specs          │   │
│     └───────────────────────────────────────────────┘   │
│                                                          │
│  2. Spawn background thread (non-blocking)              │
│     ┌───────────────────────────────────────────────┐   │
│     │ Fetch from openecualliance.org API            │   │
│     │   ├── Success → Update cache & registry       │   │
│     │   └── Failure → Continue with cache/embedded  │   │
│     └───────────────────────────────────────────────┘   │
│                                                          │
│  3. User experience                                     │
│     - App starts immediately with cached/embedded specs │
│     - Background refresh happens silently               │
│     - Next startup gets latest specs from cache         │
└─────────────────────────────────────────────────────────┘
```

**Key Design Principles:**

- **Zero blocking** - App never waits for API calls
- **Always available** - Embedded specs ensure offline functionality
- **Auto-updating** - Latest specs fetched in background
- **Cache efficiency** - 24-hour staleness threshold reduces API load

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

### 2. Runtime Spec Loading with Fallback

Adapter specs are loaded at runtime via a fallback chain:

```rust
// src/adapters/registry.rs

// Embedded YAML as compile-time fallback
const HALTECH_NSP_YAML: &str =
    include_str!("../../spec/openecu/adapters/haltech/haltech-nsp.adapter.yaml");
// ... 9 more adapters

// Load with cache -> embedded fallback
fn load_adapters_with_fallback() -> Vec<AdapterSpec> {
    // 1. Try cache first (if fresh)
    if !cache::is_cache_stale() {
        if let Some(cached) = cache::load_cached_adapters() {
            return cached; // Fast path: use cache
        }
    }

    // 2. Fall back to embedded specs
    parse_embedded_adapters()
}

// Dynamic registry with RwLock for API updates
static ADAPTER_SPECS: LazyLock<RwLock<Vec<AdapterSpec>>> =
    LazyLock::new(|| RwLock::new(load_adapters_with_fallback()));
```

### 3. Background API Refresh

On app startup, a background thread fetches latest specs from the API:

```rust
// src/app.rs
thread::spawn(|| match adapters::refresh_specs_from_api() {
    RefreshResult::Success { adapters_count, protocols_count } => {
        tracing::info!("Refreshed {} adapters, {} protocols",
                      adapters_count, protocols_count);
    }
    RefreshResult::Failed(e) => {
        tracing::debug!("API refresh failed (using cache/embedded): {}", e);
    }
    RefreshResult::AlreadyRefreshed => {}
});
```

The refresh process:

1. Fetches from `https://openecualliance.org/api/adapters` and `/api/protocols`
2. Saves to local disk cache (`{app_data_dir}/UltraLog/oecua_specs/`)
3. Updates in-memory registry (`ADAPTER_SPECS` and `PROTOCOL_SPECS`)
4. Rebuilds normalization and metadata maps
5. Next startup loads fresh specs from cache

### 4. Normalization Map Building

A reverse lookup map is built from all adapter `source_names` (dynamically updated on API refresh):

```rust
// src/adapters/registry.rs

// RwLock allows dynamic updates when API refresh completes
static SPEC_NORMALIZATION_MAP: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(build_normalization_map(&ADAPTER_SPECS.read().unwrap())));

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

When API refresh completes, the map is rebuilt:

```rust
// After successful API fetch
if let Ok(mut norm_lock) = SPEC_NORMALIZATION_MAP.write() {
    *norm_lock = build_normalization_map(&new_adapters);
}
```

### 5. Normalization Integration

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

### 6. Channel Metadata Access

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

## API Client and Caching

### OpenECU Alliance API

The API client (`src/adapters/api.rs`) fetches specs from the OpenECU Alliance website:

**Base URL:** `https://openecualliance.org`

**Endpoints:**

| Endpoint | Method | Description | Response |
|----------|--------|-------------|----------|
| `/api/adapters` | GET | List all adapters (summary) | `{ data: [AdapterSummary] }` |
| `/api/adapters/{vendor}/{id}` | GET | Get full adapter spec | `AdapterSpec` |
| `/api/protocols` | GET | List all protocols (summary) | `{ data: [ProtocolSummary] }` |
| `/api/protocols/{vendor}/{id}` | GET | Get full protocol spec | `ProtocolSpec` |

**Fetch Strategy:**

```rust
// Fetch all adapters (list + individual fetches)
pub fn fetch_all_adapters() -> Result<Vec<AdapterSpec>, ApiError> {
    let summaries = fetch_adapter_list()?;  // GET /api/adapters

    let mut adapters = Vec::new();
    for summary in summaries {
        // GET /api/adapters/{vendor}/{id} for each
        if let Ok(adapter) = fetch_adapter(&summary.vendor, &summary.id) {
            adapters.push(adapter);
        }
    }
    Ok(adapters)
}
```

**Error Handling:**

- Network failures → Fall back to cache or embedded specs
- API errors (4xx/5xx) → Log warning, continue with fallback
- Parse errors → Log warning, skip that spec
- Non-blocking: API failures don't prevent app startup

### Local Disk Cache

The cache module (`src/adapters/cache.rs`) manages persistent storage of fetched specs:

**Cache Structure:**

```
{app_data_dir}/UltraLog/oecua_specs/
├── metadata.json          # Cache metadata (timestamp, counts)
├── adapters/
│   ├── haltech-haltech-nsp.json
│   ├── ecumaster-ecumaster-emu-csv.json
│   └── ... (8 adapter files)
└── protocols/
    ├── haltech-haltech-elite-broadcast.json
    ├── ecumaster-ecumaster-emu-broadcast.json
    └── ... (9 protocol files)
```

**Cache Metadata:**

```json
{
  "last_fetch_timestamp": 1706123456,
  "cache_version": 1,
  "adapter_count": 8,
  "protocol_count": 9
}
```

**Cache Functions:**

```rust
// Check if cache is stale (> 24 hours old)
pub fn is_cache_stale() -> bool

// Load cached adapters/protocols
pub fn load_cached_adapters() -> Option<Vec<AdapterSpec>>
pub fn load_cached_protocols() -> Option<Vec<ProtocolSpec>>

// Save fetched specs to cache
pub fn save_adapters_to_cache(adapters: &[AdapterSpec]) -> Result<()>
pub fn save_protocols_to_cache(protocols: &[ProtocolSpec]) -> Result<()>

// Utility functions
pub fn get_cache_age() -> Option<Duration>
pub fn clear_cache() -> Result<()>
```

**Staleness Threshold:**

- Default: 24 hours
- Configurable via `is_cache_stale_with_max_age(secs)`
- Prevents excessive API requests
- Balance between freshness and performance

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

    // API and cache
    refresh_specs_from_api,   // Trigger background refresh (returns RefreshResult)
    specs_refreshed,          // Check if specs have been refreshed from API
    get_spec_source,          // Get current spec source ("API", "Cache", "Embedded")

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

    // Result types
    RefreshResult,            // Success/Failed/AlreadyRefreshed
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

**Note:** The OpenECU Alliance API integration (implemented) provides runtime spec updates from the official repository. User directory loading would be for custom/experimental adapters not yet in the official specs.

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

Use `source_unit` and `conversion` fields for automatic conversion:

```yaml
channels:
  - id: coolant_temp
    unit: celsius
    source_unit: fahrenheit
    conversion: "(x - 32) * 5/9"
```

## Open Questions

1. **Binary formats**: How much can be spec-driven?
   - Current approach: Specs provide metadata, parsers handle binary format details

2. **User adapter validation**: How to validate user-created adapters?
   - Use JSON Schema validation before loading

3. **Adapter updates**: How to handle spec version changes?
   - Include adapter version in metadata, warn on breaking changes
