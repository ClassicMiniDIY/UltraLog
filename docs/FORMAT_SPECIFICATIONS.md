# ECU Log File Format Specifications

**Document Version:** 1.0
**Publication Date:** December 2024
**Purpose:** Defensive publication and interoperability documentation

---

## Introduction

This document describes the file formats parsed by UltraLog for the purpose of enabling interoperability between ECU (Engine Control Unit) data logging systems. These specifications are published to:

1. **Enable data portability** - Allow users to analyze their own vehicle telemetry data
2. **Establish prior art** - Document reverse-engineered format details for defensive purposes
3. **Support the community** - Provide technical reference for open-source developers

All format details were derived through legitimate reverse engineering of user-exported data files, following the principles established in *Sega Enterprises, Ltd. v. Accolade, Inc.* (1992) regarding interoperability.

---

## Table of Contents

1. [Haltech NSP CSV Export](#haltech-nsp-csv-export)
2. [ECUMaster EMU Pro CSV Export](#ecumaster-emu-pro-csv-export)
3. [RomRaider CSV Export](#romraider-csv-export)
4. [MegaLogViewer Binary Format (MLG)](#megalogviewer-binary-format-mlg)
5. [AiM XRK/DRK Binary Format](#aim-xrkdrk-binary-format)
6. [Link ECU LLG Binary Format](#link-ecu-llg-binary-format)

---

## Haltech NSP CSV Export

### Overview

Haltech ECUs use NSP (Haltech Software Platform) to export log data as CSV files. These files contain timestamped channel data with standardized type annotations.

### File Identification

- **Extension:** `.csv`, `.log`
- **Header Pattern:** File begins with `%DataLog%` marker

### Format Structure

```
%DataLog%
<blank lines>
<header row with channel names and types>
<data rows>
```

### Header Row Format

Each column header follows the pattern:
```
<ChannelName>(<ChannelType>:<Unit>)
```

Example:
```
Time(Time_ms:ms),RPM(EngineSpeed:rpm),MAP(AbsPressure:kPa)
```

### Timestamp Format

First column contains timestamps in `HH:MM:SS.mmm` format:
- Hours: 1-2 digits
- Minutes: 2 digits
- Seconds: 2 digits
- Milliseconds: variable precision

### Channel Types

| Type | Description | Conversion |
|------|-------------|------------|
| `EngineSpeed` | Engine RPM | Direct (no conversion) |
| `AbsPressure` | Absolute pressure (kPa) | raw / 10 |
| `Pressure` | Gauge pressure (kPa) | raw / 10 - 101.3 |
| `Percentage` | 0-100% values | raw / 10 |
| `Angle` | Degrees | raw / 10 |
| `Temperature` | Temperature | raw / 10 - 273.15 (K to C) |
| `BatteryVoltage` | Voltage | raw / 1000 |
| `Speed` | Vehicle speed | raw / 10 |
| `AFR` | Air/Fuel Ratio | raw / 1000 |
| `Flow` | Flow rate | raw / 100 |
| `Raw` | Unprocessed values | Direct |

### Data Encoding

- **Delimiter:** Comma (`,`)
- **Decimal separator:** Period (`.`)
- **Character encoding:** UTF-8 or ASCII

### Reference

Based on Haltech CAN ECU Broadcast Protocol documentation.

---

## ECUMaster EMU Pro CSV Export

### Overview

ECUMaster EMU Pro ECUs export log data as CSV files with hierarchical channel naming and support for multiple regional formats.

### File Identification

- **Extension:** `.csv`, `.log`
- **Pattern:** Contains `TIME` column, semicolon or tab delimited

### Header Row Format

Channel names use hierarchical paths:
```
engine/rpm
engine/map
engine/coolant/temperature
```

The last path segment is the display name.

### Data Encoding

- **Primary delimiter:** Semicolon (`;`)
- **Alternative delimiter:** Tab (`\t`)
- **European locale:** Semicolon delimiter with comma decimal separator
- **US locale:** Tab delimiter with period decimal separator

### Time Column

The `TIME` column contains elapsed time in seconds since log start.

### Unit Inference

Units are inferred from channel paths:
- Paths containing `temp` → `°C`
- Paths containing `pressure`, `map`, `baro` → `kPa`
- Paths containing `rpm` → `RPM`
- Paths containing `tps`, `throttle` → `%`

---

## RomRaider CSV Export

### Overview

RomRaider is open-source ECU logging software primarily used with Subaru ECUs. It exports standard CSV files with units embedded in column headers.

### File Identification

- **Extension:** `.csv`
- **Pattern:** First data column is `Time` (case-sensitive)

### Header Row Format

Columns follow the pattern:
```
Channel Name (unit)
```

Example:
```
Time,Engine Speed (rpm),Manifold Pressure (psi),Coolant Temp (F)
```

### Regional Formats

- **US locale:** Comma delimiter, period decimal
- **European locale:** Semicolon delimiter, comma decimal

### Time Column

Time values in seconds from log start.

### Reference

RomRaider source: https://github.com/RomRaider/RomRaider

---

## MegaLogViewer Binary Format (MLG)

### Overview

The MLG format is used by Speeduino, rusEFI, and MegaSquirt ecosystems. It's a compact binary format optimized for high-frequency data logging.

### File Identification

- **Extension:** `.mlg`
- **Magic bytes:** `MLVLG` (bytes 0-4)

### Header Structure

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 5 | Magic: `MLVLG` |
| 5 | 1 | Reserved |
| 6 | 2 | Format version (big-endian int16) |
| 8 | 4 | Timestamp (big-endian int32) |
| 12 | 2/4 | Info data start (int16 v1, int32 v2) |
| var | 4 | Data begin index (big-endian int32) |
| var | 2 | Record length (big-endian int16) |
| var | 2 | Number of fields (big-endian int16) |

### Format Versions

- **Version 1:** 55-byte field definitions
- **Version 2:** 89-byte field definitions

### Field Definition Structure (v1)

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 1 | Field type |
| 1 | 34 | Field name (null-terminated ASCII) |
| 35 | 10 | Unit string (null-terminated ASCII) |
| 45 | 1 | Digits (decimal places) |
| 46 | 4 | Scale factor (big-endian float32) |
| 50 | 4 | Transform (big-endian float32) |
| 54 | 1 | Reserved |

### Field Types

| Value | Type | Size (bytes) |
|-------|------|--------------|
| 0 | U08 | 1 |
| 1 | S08 | 1 |
| 2 | U16 | 2 |
| 3 | S16 | 2 |
| 4 | U32 | 4 |
| 5 | S32 | 4 |
| 6 | S64 | 8 |
| 7 | F32 | 4 |
| `0x10` | U08 Bitfield | 1 |
| `0x11` | U16 Bitfield | 2 |
| `0x12` | U32 Bitfield | 4 |

### Data Records

Records follow field definitions:
- **Block type byte:** Indicates record type
- **Timestamp:** Record timestamp
- **Field values:** Packed according to field definitions

### Value Conversion

```
display_value = (raw_value + transform) * scale
```

---

## AiM XRK/DRK Binary Format

### Overview

AiM Technologies data acquisition devices (MXP, MXG, MXL2, EVO5, MyChron5) use the XRK/DRK binary format for storing logged telemetry.

### File Identification

- **Extensions:** `.xrk`, `.drk`
- **Magic bytes:** `<hCNF` (bytes 0-4)

### File Structure

The file consists of tagged sections:

```
<hCNF> - Configuration header
<hCHS> - Channel section headers
<hFOT> - Footer with metadata
```

### Configuration Header

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 6 | Magic: `<hCNF\x00` |
| 6 | 4 | Section length (little-endian uint32) |
| 10 | 2 | Version |
| 12 | var | Configuration data |

### Channel Section (`<hCHS>`)

Each channel section contains:

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 6 | Tag: `<hCHS\x00` |
| 6 | 4 | Section length (little-endian uint32) |
| 10 | 2 | Version |
| 12 | 4 | Channel index |
| var | var | Metadata fields |
| ~30 | 8 | Short name (null-padded ASCII) |
| ~38 | 24 | Long name (null-padded ASCII) |
| ~62 | 8 | Unit string (null-padded ASCII) |
| var | var | Sample rate and data info |

### Footer Section (`<hFOT>`)

Contains session metadata:
- Vehicle name
- Racer name
- Track name
- Championship
- Venue type
- Session datetime

### Data Storage

Channel data is stored as time-value pairs using floating-point representation.

---

## Link ECU LLG Binary Format

### Overview

Link Engine Management ECUs (G4, G4+, G4X series) use the LLG binary format for data logging.

### File Identification

- **Extension:** `.llg`
- **Magic bytes:** `lf3` at offset 4

### Header Structure

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 4 | Header size (little-endian uint32) |
| 4 | 3 | Magic: `lf3` |
| 7 | var | Version and flags |

Total header: ~215 bytes

### Metadata Locations

Metadata is stored at fixed offsets using UTF-16 LE encoding:

| Offset | Field | Max Length |
|--------|-------|------------|
| 0x336 | ECU Model | 32 chars |
| 0x1786 | Log Date | 16 chars |
| 0x184e | Log Time | 16 chars |
| 0x1916 | Software Version | 20 chars |
| 0x1aa6 | Source | 20 chars |

### Channel Block Structure

Channel blocks begin around offset 0x2000:

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 4 | Zero padding |
| 4 | 4 | Channel ID (little-endian uint32) |
| 8 | 200 | Channel name (UTF-16 LE) |
| 208 | 200 | Unit string (UTF-16 LE) |
| 408 | var | Data records |

### Data Records

Data is stored as (value, time) pairs:
- **Value:** float32 (little-endian)
- **Time:** float32 (little-endian) in seconds

### String Encoding

All strings use UTF-16 Little Endian encoding, null-terminated.

---

## Legal Notice

This document is published for interoperability purposes under fair use principles. All trademarks mentioned are property of their respective owners.

The format specifications described herein were derived through analysis of user-accessible exported data files. No copy protection mechanisms were circumvented, and no proprietary source code was accessed.

This documentation is provided "as is" without warranty. Implementation of parsers based on these specifications is at the developer's own risk.

---

## Publication for Defensive Purposes

This document is intended for publication to Technical Disclosure Commons (TDCommons) or similar defensive publication repositories to establish prior art and prevent patenting of these interoperability techniques.

**Submitter:** Cole Gentry
**Project:** UltraLog (https://github.com/ClassicMiniDIY/UltraLog)
**License:** AGPL-3.0

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | December 2024 | Initial publication |
