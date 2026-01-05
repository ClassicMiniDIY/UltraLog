# UltraLog Integration with OpenECU Alliance Specs

This document describes how UltraLog integrates with OpenECU Alliance adapter specifications for channel normalization and metadata.

## Implementation Status

**Completed:**

- [x] Adapter specs embedded at compile time via `include_str!`
- [x] Spec-driven channel normalization from `source_names`
- [x] Integration with existing `normalize.rs` (spec as fallback)
- [x] Channel metadata lookup (min/max/precision/category)
- [x] 8 adapter specs integrated (Haltech, ECUMaster, Link, AiM, RomRaider, Speeduino, rusEFI, Emerald)

**Not Yet Implemented:**

- [ ] Runtime adapter loading from user directory
- [ ] Generic CSV/binary parser driven by specs
- [ ] Adapter marketplace integration
- [ ] Build-time code generation

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
│   ├── types.rs         # AdapterSpec, ChannelSpec, ChannelCategory, etc.
│   └── registry.rs      # Spec loading, normalization maps, metadata lookup
├── normalize.rs         # Field normalization (uses adapters for fallback)
└── parsers/
    └── types.rs         # Channel type enhanced with spec metadata methods

spec/OECUASpecs/         # Git submodule: github.com/ClassicMiniDIY/OECUASpecs
build.rs                 # Auto-downloads specs if submodule missing
```

## How It Works

### 1. Spec Source: GitHub Submodule

Adapter specs come from the [OECUASpecs](https://github.com/ClassicMiniDIY/OECUASpecs) repository as a git submodule:

```bash
# Clone with submodules
git clone --recursive https://github.com/ClassicMiniDIY/UltraLog.git

# Or initialize after cloning
git submodule update --init
```

The `build.rs` script ensures specs are available:

1. Checks if submodule is initialized
2. If not, attempts `git submodule update --init`
3. If git fails, downloads specs directly from GitHub raw content

### 2. Compile-Time Spec Embedding

Adapter YAML files are embedded at compile time using `include_str!`:

```rust
// src/adapters/registry.rs
const HALTECH_NSP_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/haltech/haltech-nsp.adapter.yaml");
// ... 7 more adapters

static ADAPTER_SPECS: LazyLock<Vec<AdapterSpec>> = LazyLock::new(|| {
    EMBEDDED_ADAPTERS.iter()
        .filter_map(|yaml| serde_yaml::from_str(yaml).ok())
        .collect()
});
```

### 2. Normalization Map Building

A reverse lookup map is built from all adapter `source_names`:

```rust
// src/adapters/registry.rs
static SPEC_NORMALIZATION_MAP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for adapter in ADAPTER_SPECS.iter() {
        for channel in &adapter.channels {
            for source_name in &channel.source_names {
                map.insert(source_name.to_lowercase(), channel.name.clone());
            }
        }
    }
    map
});
```

### 3. Normalization Integration

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

### 4. Channel Metadata Access

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

    // Categories
    get_all_categories,       // Get all unique ChannelCategory values
    get_channels_by_category, // Get all channels for a category

    // Types
    AdapterSpec,
    ChannelSpec,
    ChannelCategory,
    DataType,
    FileFormatSpec,
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

1. Add adapter YAML to [OECUASpecs](https://github.com/ClassicMiniDIY/OECUASpecs) repo
2. Update submodule: `git submodule update --remote`
3. Add `include_str!` in `src/adapters/registry.rs`
4. Add to `EMBEDDED_ADAPTERS` array
5. Add download URL to `build.rs` `ADAPTER_SPECS` array
6. Rebuild UltraLog

Example adding a new adapter:

```rust
// src/adapters/registry.rs
const NEW_ADAPTER_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/newvendor/newformat.adapter.yaml");

static EMBEDDED_ADAPTERS: &[&str] = &[
    HALTECH_NSP_YAML,
    // ... existing adapters ...
    NEW_ADAPTER_YAML,  // Add here
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

### Runtime Adapter Loading

Load user adapters from `~/.ultralog/adapters/`:

```rust
impl AdapterRegistry {
    pub fn load_user_adapters(path: &Path) -> Result<Vec<AdapterSpec>> {
        // Read YAML files from user directory
        // Merge with embedded adapters
    }
}
```

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
