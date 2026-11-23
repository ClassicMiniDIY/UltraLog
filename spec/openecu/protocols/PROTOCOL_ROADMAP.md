# ECU CAN Protocol Roadmap

This document tracks CAN protocol additions to the OpenECU Alliance specification.

## Completed Protocols

### Priority 1: Well-Documented Protocols

#### Haltech Elite (Reference Implementation)

- **File**: `protocols/haltech/haltech-elite-broadcast.protocol.yaml`
- **Source**: https://support.haltech.com/portal/en/kb/articles/haltech-can-protocol-specification
- **Baudrate**: 1Mbit/s
- **Format**: 11-bit standard frames
- **Status**: ✅ Complete

#### MaxxECU

- **File**: `protocols/maxxecu/maxxecu-default.protocol.yaml`
- **Source**: https://www.maxxecu.com/webhelp/can-default_maxxecu_protocol.html
- **DBC Available**: Yes, at https://www.maxxecu.com/downloads
- **Baudrate**: 1Mbit/s (configurable)
- **Format**: 11-bit standard frames, little-endian
- **Status**: ✅ Complete

#### Megasquirt

- **File**: `protocols/megasquirt/megasquirt-broadcast.protocol.yaml`
- **Source**: http://www.megamanual.com/com/CAN.htm
- **Additional**: https://wiki.autosportlabs.com/Megasquirt_CAN
- **Baudrate**: 500kbit/s
- **Format**: 11-bit and 29-bit frames, big-endian
- **Status**: ✅ Complete

#### AEM Infinity

- **File**: `protocols/aem/aem-infinity-broadcast.protocol.yaml`
- **Source**: DBC files in AEM DashDesign software
- **Reference**: https://www.scribd.com/document/413840009/Aemnet-Can-Bus-Protocol
- **Baudrate**: 500kbit/s
- **Format**: 29-bit extended frames, little-endian (AEMNET)
- **Status**: ✅ Complete

### Priority 2: Moderate Documentation

#### Speeduino

- **File**: `protocols/speeduino/speeduino-broadcast.protocol.yaml`
- **Source**: https://wiki.speeduino.com/en/Canbus_Support2
- **Baudrate**: 500kbit/s
- **Format**: 11-bit, Haltech-compatible broadcast
- **Status**: ✅ Complete

#### ECUMaster EMU

- **File**: `protocols/ecumaster/ecumaster-emu-broadcast.protocol.yaml`
- **Source**: https://www.ecumaster.com/downloads/ (ADU CAN Stream docs)
- **Baudrate**: 1Mbit/s
- **Format**: 11-bit standard frames, little-endian
- **Status**: ✅ Complete

#### rusEFI

- **File**: `protocols/rusefi/rusefi-broadcast.protocol.yaml`
- **Source**: https://github.com/rusefi/rusefi/blob/master/firmware/controllers/can/can_verbose.cpp
- **Baudrate**: 500kbit/s
- **Format**: 11-bit standard frames, little-endian
- **Status**: ✅ Complete

### Priority 3: Community/Configurable

#### Syvecs S6/S7

- **File**: `protocols/syvecs/syvecs-s7-broadcast.protocol.yaml`
- **Source**: https://www.syvecs.com/forum/viewtopic.php?t=536
- **DBC Discussion**: https://www.syvecs.com/forum/viewtopic.php?t=1121
- **Baudrate**: 1Mbit/s
- **Format**: 11-bit standard frames, little-endian
- **Status**: ✅ Complete

#### Emtron

- **File**: `protocols/emtron/emtron-broadcast.protocol.yaml`
- **Source**: https://wiki.autosportlabs.com/Emtron
- **Setup Guide**: https://gaugeart.com/instructions/emtron-ecu-can-telemetry-setup/
- **Baudrate**: 1Mbit/s
- **Format**: 11-bit standard frames, little-endian
- **Notes**: Default "Emtron Display Tx Set 1" configuration
- **Status**: ✅ Complete

---

## Remaining Protocols

### Link ECU G4+/G4X

- **Discussion**: https://forums.linkecu.com/topic/23547-dbc-for-can/
- **Notes**: Fully user-configurable, no standard protocol
- **Status**: ⏳ Would need to document common configurations

---

## Not Currently Feasible

### FuelTech FT Series

- Proprietary FTCAN protocol
- Limited public documentation
- Would require reverse engineering

### MoTeC M-Series

- Fully configurable in ECU
- No standard broadcast protocol
- ECUMaster has app notes for common configs

---

## Implementation Steps

For each protocol to add:

1. **Research**: Gather all available documentation
2. **Convert to YAML**: Follow `protocols/haltech/haltech-elite-broadcast.protocol.yaml` format
3. **Validate**: Check against schema at `schema/protocol.schema.json`
4. **Test**: Verify signal definitions against real data if possible
5. **Document**: Add branding assets and update README

---

## Resources

### Official Documentation

- Haltech: https://support.haltech.com/portal/en/kb/articles/haltech-can-protocol-specification
- MaxxECU: https://www.maxxecu.com/webhelp/can-default_maxxecu_protocol.html
- Megasquirt: http://www.megamanual.com/com/CAN.htm
- Speeduino: https://wiki.speeduino.com/en/Canbus_Support2
- ECUMaster: https://www.ecumaster.com/downloads/
- Emtron: https://wiki.autosportlabs.com/Emtron

### Community DBC Collections

- https://github.com/jamiejones85/DBC-files
- https://github.com/commaai/opendbc
- https://github.com/iDoka/awesome-automotive-can-id

### Tools

- DBC Editor: https://www.csselectronics.com/pages/dbc-editor-can-bus-database
- MegaCAN Arduino Library: https://github.com/mantonakakis1/MegaCAN
