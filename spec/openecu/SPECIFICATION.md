# OpenECU Spec — Channel Reference

**Spec version 1.0 · registry 1.0.0 · updated 2026-08-24**

This is the canonical channel vocabulary for the OpenECU Spec. It is generated
from [`channels.yaml`](channels.yaml), which is the machine-readable source of
truth — **edit that file, not this one.**

Regenerate with:

```bash
bun run generate:spec-doc
```

---

## Why this file exists

An adapter maps a vendor's own column names onto a canonical channel ID, so an
application can read `rpm` from a Haltech log and a Link log without knowing
anything about either vendor. That only works if every adapter agrees on what
`rpm` means — the same ID, the same category, the same unit.

Until 2026-08-24 this file was referenced by the contributor docs but did not
exist, and the vocabulary was defined implicitly by whatever the nine adapters
happened to use. Eight IDs had drifted onto incompatible units: `map` was kPa
in seven adapters, psi in one and mbar in another. A consumer that trusted the
ID alone got readings off by an order of magnitude.

---

## The rules

1. **Every adapter channel `id` and every protocol signal `id` must appear in
   this registry**, either as a canonical ID or as a listed alias.
2. **`unit` on an adapter channel describes the log file, not the ideal.** If a
   vendor writes psi, the adapter says `psi`. Never rewrite it to look canonical
   — that makes the spec lie about the file.
3. **When the source unit differs from the canonical unit, declare the
   conversion** on the channel:

   ```yaml
   - id: map
     unit: psi
     to_canonical:
       scale: 6.89475729
       offset: 0
   ```

   A consumer computes `canonical = raw * scale + offset`. Adapters already in
   the canonical unit omit `to_canonical` entirely.
4. **Category must match the registry.** It drives grouping on the site.
5. **Aliases still resolve** but are deprecated. New adapters should use the
   canonical ID.
6. **A protocol signal that is not a measurement** carries `reserved: true`
   (padding, transport framing). One with no canonical equivalent carries
   `vendor_specific: true`. Both are deliberate declarations — an unmarked
   signal without an `id` is treated as an oversight.
7. **Adding a channel?** Add it to `channels.yaml` first, then use it. Run
   `bun run validate:specs` before opening a PR — CI runs it too.

---

## Units

One spelling per unit. The validator rejects anything else.

| quantity | canonical unit |
| --- | --- |
| pressure | `kpa` |
| temperature | `celsius` |
| speed | `kph` |
| angle | `degrees` |
| angular rate | `deg/s` |
| proportion | `percent` |
| air-fuel | `lambda`, `afr` |
| mass flow | `g/s` |
| volume flow | `l/h` |
| voltage | `volts` |
| length | `meters`, `millimeters` |
| acceleration | `g` |
| frequency | `hz` |
| time | `seconds`, `milliseconds` |
| torque | `nm` |
| engine speed | `rpm` |

### Accepted spellings

These normalise with no numeric change:

| written | canonical |
| --- | --- |
| `%` | `percent` |
| `amps` | `amps` |
| `b btdc` | `degrees` |
| `b c` | `celsius` |
| `b` | `degrees` |
| `c` | `celsius` |
| `deg` | `degrees` |
| `deg/sec` | `deg/s` |
| `kPa` | `kpa` |
| `km/h` | `kph` |
| `kph` | `kph` |
| `ma` | `ma` |
| `mpa` | `mpa` |
| `ms` | `milliseconds` |
| `ohms` | `ohms` |
| `pct` | `percent` |
| `psi` | `psi` |
| `s` | `seconds` |
| `v` | `volts` |
| `°c` | `celsius` |

### Conversions

`canonical = raw * scale + offset`

| from | to | scale | offset |
| --- | --- | --- | --- |
| `psi` | `kpa` | 6.89475729 | 0 |
| `bar` | `kpa` | 100 | 0 |
| `mbar` | `kpa` | 0.1 | 0 |
| `mph` | `kph` | 1.609344 | 0 |
| `milliseconds` | `seconds` | 0.001 | 0 |
| `cc/min` | `l/h` | 0.06 | 0 |
| `fahrenheit` | `celsius` | 0.5555555556 | -17.7777778 |
| `kilometers` | `meters` | 1000 | 0 |
| `liters` | `l/h` | 1 | 0 |
| `mpa` | `bar` | 10 | 0 |
| `ma` | `amps` | 0.001 | 0 |

---

## Channels

466 canonical channels across 20 categories. **Used by**
counts how many of the 10 adapters currently map the channel.

### acceleration

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `g_lateral` | Lateral G | `g` | float | 2 | Lateral acceleration (positive = right) |
| `g_longitudinal` | Longitudinal G | `g` | float | 2 | Longitudinal acceleration (positive = acceleration) |
| `g_vertical` | Vertical G | `g` | float | 2 | Vertical acceleration |
| `vehicle_acceleration` | Vehicle Acceleration | `m/s2` | float | 1 | Longitudinal acceleration derived from vehicle speed. |

### correction

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `accel_enrichment` | Acceleration Enrichment | `percent` | float | 3 | Transient fuel enrichment |
| `af_learning` | A/F Learning | `percent` | float | 1 | Long-term fuel trim / A/F Learning value |
| `afr_correction` | A/F Correction | `percent` | float | 1 | Short-term fuel trim from A/F sensor |
| `barometric_correction` | Barometric Correction | `percent` | float | 2 | Altitude/barometric fuel correction |
| `clt_correction` | CLT Correction | `percent` | float | 2 | Coolant temperature fuel correction |
| `dam` | DAM | `multiplier` | float | 1 | Dynamic Advance Multiplier |
| `ect_fuel_trim` | ECT Fuel Trim | `percent` | float | 1 | Coolant temperature fuel trim |
| `ego_correction` | EGO Correction | `percent` | float | 3 | Closed-loop O2 correction |
| `ego_correction_1` | EGO Correction Bank 1 | `percent` | float | 1 | EGO correction for bank 1 |
| `ego_correction_2` | EGO Correction Bank 2 | `percent` | float | 1 | EGO correction for bank 2 |
| `feedback_knock` | Feedback Knock Correction | `degrees` | float | 1 | Real-time knock retard |
| `fine_knock_learning` | Fine Knock Learning | `degrees` | float | 1 | Fine knock learning value per cylinder |
| `fuel_correction` | Fuel Correction | `percent` | float | 2 | Active fuel correction percentage |
| `fuel_trim` | Closed Loop Trim | `percent` | float | 2 | Closed loop fuel trim |
| `gamma_enrichment` | Gamma Enrichment | `percent` | float | 1 | Total fuel correction multiplier |
| `iam` | IAM | `multiplier` | float | 1 | Ignition Advance Multiplier (Subaru learning value) |
| `iat_correction` | IAT Correction | `percent` | float | 3 | Intake air temperature fuel correction |
| `iat_fuel_trim` | IAT Fuel Trim | `percent` | float | 1 | Intake air temperature fuel trim |
| `knock_retard` | Knock Retard | `degrees` | float | 7 | Timing retard from knock detection |
| `warmup_enrichment` | Warmup Enrichment | `percent` | float | 3 | Cold engine fuel enrichment |

### custom

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `accel_x` | Acceleration X | `g` | float | 1 | Lateral acceleration |
| `accel_y` | Acceleration Y | `g` | float | 1 | Longitudinal acceleration |
| `aux_input_1` | Auxiliary Input 1 | `—` | float | 1 | Generic auxiliary input channel |
| `aux_input_2` | Auxiliary Input 2 | `—` | float | 1 | Generic auxiliary input channel |

### diagnostics

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `idle_valve_step` | Idle Valve Step | `count` | float | 1 | Idle air control valve step position. |
| `mil` | MIL | `—` | bool | 1 | Malfunction indicator lamp status (ON/OFF) |
| `roughness_cyl_1` | Roughness Monitor Cylinder 1 | `count` | float | 1 | Combustion roughness for cylinder 1. |
| `roughness_cyl_2` | Roughness Monitor Cylinder 2 | `count` | float | 1 | Combustion roughness for cylinder 2. |
| `roughness_cyl_3` | Roughness Monitor Cylinder 3 | `count` | float | 1 | Combustion roughness for cylinder 3. |
| `roughness_cyl_4` | Roughness Monitor Cylinder 4 | `count` | float | 1 | Combustion roughness for cylinder 4. |

### driver_input

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `steering_angle` | Steering Angle | `degrees` | float | 1 | Steering wheel angle |

### drivetrain

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `desired_gear` | Desired Gear | `gear` | int | 1 | Gear the transmission controller is commanding. |
| `gear` | Gear Position | `gear` | int | 10 | Current gear |
| `gear_ratio` | Gear Ratio | `ratio` | float | 1 | Current calculated gear ratio. |
| `gear_selector_position` | Gear Selector Position | `gear` | int | 0 | Position of the gear selector. |

### electrical

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `analog_d14` | Analog D14 | `volts` | float | 1 | Analog input D14 voltage |
| `analog_input_1` | Analog Voltage 1<br>_aliases: `an_volt_1`_ | `volts` | float | 1 | Analog input 1 voltage |
| `analog_input_10` | Analog Input 10 | `volts` | float | 0 | Generic analog input 10. |
| `analog_input_11` | Analog Input 11 | `volts` | float | 0 | Generic analog input 11. |
| `analog_input_12` | Analog Input 12 | `volts` | float | 0 | Generic analog input 12. |
| `analog_input_13` | Analog Input 13 | `volts` | float | 0 | Generic analog input 13. |
| `analog_input_14` | Analog Input 14 | `volts` | float | 0 | Generic analog input 14. |
| `analog_input_15` | Analog Input 15 | `volts` | float | 0 | Generic analog input 15. |
| `analog_input_16` | Analog Input 16 | `volts` | float | 0 | Generic analog input 16. |
| `analog_input_2` | Analog Voltage 2<br>_aliases: `an_volt_2`_ | `volts` | float | 1 | Analog input 2 voltage |
| `analog_input_3` | Analog Voltage 3<br>_aliases: `an_volt_3`_ | `volts` | float | 1 | Analog input 3 voltage |
| `analog_input_4` | Analog Voltage 4<br>_aliases: `an_volt_4`_ | `volts` | float | 1 | Analog input 4 voltage |
| `analog_input_5` | Analog Input 5 | `volts` | float | 0 | Generic analog input 5. |
| `analog_input_6` | Analog Input 6 | `volts` | float | 0 | Generic analog input 6. |
| `analog_input_7` | Analog Input 7 | `volts` | float | 0 | Generic analog input 7. |
| `analog_input_8` | Analog Input 8 | `volts` | float | 0 | Generic analog input 8. |
| `analog_input_9` | Analog Input 9 | `volts` | float | 0 | Generic analog input 9. |
| `ap_main_voltage` | Main Accelerator Sensor Voltage | `volts` | float | 2 | Main accelerator pedal sensor voltage. |
| `ap_sub_voltage` | Sub Accelerator Sensor Voltage | `volts` | float | 2 | Secondary accelerator pedal sensor voltage. |
| `battery_voltage` | Battery Voltage | `volts` | float | 10 | System voltage |
| `eld_voltage` | ELD Voltage | `volts` | float | 1 | Electrical load detector voltage |
| `fuel_level_voltage` | Fuel Level Voltage | `volts` | float | 1 | Raw fuel level sender voltage. |
| `ignition_voltage` | Ignition Voltage | `volts` | float | 1 | Voltage at the ignition switch input. |
| `maf_voltage` | MAF Voltage | `volts` | float | 2 | Mass airflow sensor voltage |
| `map_voltage` | MAP Voltage | `volts` | float | 1 | Raw MAP sensor voltage |
| `o2_sensor_current` | O2 Sensor Current | `amps` | float | 1 | Wideband oxygen sensor pump current. |
| `o2_sensor_resistance` | O2 Sensor Resistance | `ohms` | float | 1 | Wideband oxygen sensor internal resistance. |
| `o2_voltage_front_1` | Front O2 Voltage 1 | `volts` | float | 2 | Narrowband front oxygen sensor 1 voltage. |
| `o2_voltage_front_2` | Front O2 Voltage 2 | `volts` | float | 2 | Narrowband front oxygen sensor 2 voltage. |
| `o2_voltage_rear` | Rear O2 Voltage | `volts` | float | 2 | Narrowband rear oxygen sensor voltage. |
| `ocv_current_exhaust` | Exhaust OCV Current | `amps` | float | 1 | Exhaust oil control valve current. |
| `ocv_current_intake` | Intake OCV Current | `amps` | float | 1 | Intake oil control valve current. |
| `tgv_voltage_left` | Tumble Valve Sensor Voltage Left | `volts` | float | 1 | Left tumble generator valve position sensor voltage. |
| `tgv_voltage_right` | Tumble Valve Sensor Voltage Right | `volts` | float | 1 | Right tumble generator valve position sensor voltage. |
| `throttle_motor_voltage` | Throttle Motor Voltage | `volts` | float | 1 | Drive-by-wire throttle motor voltage. |
| `tps_main_voltage` | Main Throttle Sensor Voltage | `volts` | float | 1 | Main throttle position sensor voltage. |
| `tps_sub_voltage` | Sub Throttle Sensor Voltage | `volts` | float | 2 | Secondary throttle position sensor voltage. |
| `tps_voltage` | TPS Voltage | `volts` | float | 3 | Raw throttle position sensor voltage |
| `user_channel_1` | User Channel 1 | `volts` | float | 0 | User-assignable channel 1, source configured in the tuning software. |
| `user_channel_10` | User Channel 10 | `volts` | float | 0 | User-assignable channel 10, source configured in the tuning software. |
| `user_channel_11` | User Channel 11 | `volts` | float | 0 | User-assignable channel 11, source configured in the tuning software. |
| `user_channel_12` | User Channel 12 | `volts` | float | 0 | User-assignable channel 12, source configured in the tuning software. |
| `user_channel_13` | User Channel 13 | `volts` | float | 0 | User-assignable channel 13, source configured in the tuning software. |
| `user_channel_14` | User Channel 14 | `volts` | float | 0 | User-assignable channel 14, source configured in the tuning software. |
| `user_channel_15` | User Channel 15 | `volts` | float | 0 | User-assignable channel 15, source configured in the tuning software. |
| `user_channel_16` | User Channel 16 | `volts` | float | 0 | User-assignable channel 16, source configured in the tuning software. |
| `user_channel_2` | User Channel 2 | `volts` | float | 0 | User-assignable channel 2, source configured in the tuning software. |
| `user_channel_3` | User Channel 3 | `volts` | float | 0 | User-assignable channel 3, source configured in the tuning software. |
| `user_channel_4` | User Channel 4 | `volts` | float | 0 | User-assignable channel 4, source configured in the tuning software. |
| `user_channel_5` | User Channel 5 | `volts` | float | 0 | User-assignable channel 5, source configured in the tuning software. |
| `user_channel_6` | User Channel 6 | `volts` | float | 0 | User-assignable channel 6, source configured in the tuning software. |
| `user_channel_7` | User Channel 7 | `volts` | float | 0 | User-assignable channel 7, source configured in the tuning software. |
| `user_channel_8` | User Channel 8 | `volts` | float | 0 | User-assignable channel 8, source configured in the tuning software. |
| `user_channel_9` | User Channel 9 | `volts` | float | 0 | User-assignable channel 9, source configured in the tuning software. |

### engine

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `af_lean_correction` | A/F Lean Correction | `percent` | float | 1 | Lean-side fuel correction. |
| `af_learning_2` | A/F Learning 2 | `percent` | float | 1 | Long-term fuel trim, learning zone 2. |
| `af_learning_3` | A/F Learning 3 | `percent` | float | 1 | Long-term fuel trim, learning zone 3. |
| `afr_correction_2` | A/F Correction 2 | `percent` | float | 1 | Short-term fuel trim, zone 2. |
| `afr_correction_3` | A/F Correction 3 | `percent` | float | 1 | Short-term fuel trim, zone 3. |
| `alternator_duty` | Alternator Duty | `percent` | float | 1 | Alternator field duty cycle. |
| `antilag_fuel_adder` | Anti-Lag Fuel Adder | `percent` | float | 0 | Extra fuel added by anti-lag. |
| `ap_main` | Accelerator Position Main | `percent` | float | 2 | Main accelerator pedal position |
| `ap_sub` | Accelerator Position Sub | `percent` | float | 1 | Secondary accelerator pedal position |
| `ase_enrichment` | After-Start Enrichment | `percent` | float | 1 | Fuel enrichment applied for a period after starting. |
| `aux_output_1_duty` | Aux Output 1 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 1. |
| `aux_output_2_duty` | Aux Output 2 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 2. |
| `aux_output_3_duty` | Aux Output 3 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 3. |
| `aux_output_4_duty` | Aux Output 4 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 4. |
| `aux_output_5_duty` | Aux Output 5 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 5. |
| `aux_output_6_duty` | Aux Output 6 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 6. |
| `aux_output_7_duty` | Aux Output 7 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 7. |
| `aux_output_8_duty` | Aux Output 8 Duty | `percent` | float | 0 | Duty cycle of auxiliary output 8. |
| `aux_speed_1` | Aux Speed 1 | `rpm` | float | 1 | Auxiliary speed input 1. |
| `aux_speed_2` | Aux Speed 2 | `rpm` | float | 1 | Auxiliary speed input 2. |
| `clutch_position` | Clutch Position | `percent` | float | 0 | Clutch pedal or actuator position. |
| `coil_duty_cycle` | Coil Duty Cycle | `percent` | float | 1 | Ignition coil duty cycle. |
| `cut_percentage` | Cut Percentage | `percent` | float | 0 | Proportion of ignition or fuel events being cut. |
| `cylinder_air_mass` | Cylinder Air Mass | `mg` | float | 1 | Air mass trapped per cylinder per cycle. |
| `dbw_duty_cycle` | Throttle Motor Duty | `percent` | float | 2 | Drive-by-wire throttle motor duty cycle. |
| `driveshaft_rpm` | Driveshaft RPM | `rpm` | float | 0 | Driveshaft rotational speed. |
| `engine_load_mass` | Engine Load (Mass) | `g/s` | float | 1 | Engine load expressed as air mass per intake event. |
| `engine_protection_rpm` | Engine Protection RPM Limit | `rpm` | float | 1 | RPM limit currently imposed by engine protection. |
| `etb_error` | ETB Error | `percent` | float | 1 | Electronic throttle body position error |
| `etb_position` | Electronic Throttle Position<br>_aliases: `dbw_position`, `e_throttle_position`_ | `percent` | float | 2 | Measured electronic throttle body plate position. |
| `etb_target` | Electronic Throttle Target<br>_aliases: `dbw_target`, `e_throttle_target`_ | `percent` | float | 3 | Commanded electronic throttle body plate position. |
| `fan_duty` | Fan Duty | `percent` | float | 1 | Cooling fan duty cycle. |
| `flex_fuel_correction` | Flex Fuel Correction | `percent` | float | 1 | Fuel correction from the flex fuel sensor. |
| `fuel_mass` | Fuel Mass | `mg` | float | 1 | Fuel mass delivered per injection. |
| `fuel_mass_base` | Base Fuel Mass | `mg` | float | 1 | Fuel mass before corrections. |
| `fuel_pump_duty` | Fuel Pump Duty | `percent` | float | 1 | Primary fuel pump duty cycle. |
| `fuel_temp_correction` | Fuel Temperature Correction | `percent` | float | 1 | Fuel correction for fuel temperature. |
| `fuel_trim_bank_1` | Fuel Trim Bank 1 | `percent` | float | 0 | Closed-loop fuel trim for bank 1. |
| `fuel_trim_bank_2` | Fuel Trim Bank 2 | `percent` | float | 0 | Closed-loop fuel trim for bank 2. |
| `fuel_trim_long_bank_1` | Long Term Fuel Trim Bank 1 | `percent` | float | 2 | Long-term learned fuel trim, bank 1. |
| `fuel_trim_long_bank_2` | Long Term Fuel Trim Bank 2 | `percent` | float | 2 | Long-term learned fuel trim, bank 2. |
| `fuel_trim_short_bank_1` | Short Term Fuel Trim Bank 1 | `percent` | float | 2 | Short-term closed-loop fuel trim, bank 1. |
| `fuel_trim_short_bank_2` | Short Term Fuel Trim Bank 2 | `percent` | float | 2 | Short-term closed-loop fuel trim, bank 2. |
| `idle_base` | Idle Base Position | `percent` | float | 1 | Idle stepper or solenoid base position |
| `idle_load` | Idle Load | `percent` | float | 1 | Idle air control position |
| `idle_position` | Idle Position | `percent` | float | 1 | Idle air control position |
| `idle_rpm_delta` | Idle RPM Delta | `rpm` | float | 1 | Difference between idle target and actual RPM. |
| `idle_target_rpm` | Idle Target RPM<br>_aliases: `idle_target`_ | `rpm` | float | 3 | Target idle speed |
| `idle_valve` | Idle Valve Position | `percent` | float | 2 | Idle air control valve position |
| `ignition_load` | Ignition Load | `percent` | float | 1 | Load axis used for the ignition table. |
| `injector_duty_2` | Injector Duty Stage 2 | `percent` | float | 0 | Injector duty cycle for injection stage 2. |
| `injector_duty_3` | Injector Duty Stage 3 | `percent` | float | 0 | Injector duty cycle for injection stage 3. |
| `injector_duty_4` | Injector Duty Stage 4 | `percent` | float | 0 | Injector duty cycle for injection stage 4. |
| `injector_duty_5` | Injector Duty Stage 5 | `percent` | float | 0 | Injector duty cycle for injection stage 5. |
| `injector_duty_6` | Injector Duty Stage 6 | `percent` | float | 0 | Injector duty cycle for injection stage 6. |
| `injector_duty_7` | Injector Duty Stage 7 | `percent` | float | 0 | Injector duty cycle for injection stage 7. |
| `injector_duty_8` | Injector Duty Stage 8 | `percent` | float | 0 | Injector duty cycle for injection stage 8. |
| `injector_duty_staged` | Injector Duty (Staged) | `percent` | float | 2 | Duty cycle of the staged (secondary) injectors. |
| `input_shaft_speed` | Input Shaft Speed | `rpm` | float | 1 | Transmission input shaft speed. |
| `instant_rpm` | Instant RPM | `rpm` | float | 1 | Instantaneous engine speed, before smoothing. |
| `launch_rpm_target` | Launch RPM Target | `rpm` | float | 1 | Launch control target RPM |
| `load` | Engine Load | `percent` | float | 3 | Calculated engine load |
| `maf` | Mass Airflow | `g/s` | float | 3 | Mass airflow sensor reading |
| `nitrous_fuel_adder` | Nitrous Fuel Adder | `percent` | float | 0 | Extra fuel added while nitrous is flowing. |
| `o2_heater_duty` | O2 Heater Duty | `percent` | float | 1 | Wideband oxygen sensor heater duty cycle. |
| `ocv_duty_exhaust` | Exhaust OCV Duty | `percent` | float | 0 | Exhaust oil control valve duty cycle. |
| `ocv_duty_intake` | Intake OCV Duty | `percent` | float | 0 | Intake oil control valve duty cycle. |
| `pedal_position` | Accelerator Pedal | `percent` | float | 2 | Accelerator pedal position |
| `purge_valve_duty` | Purge Valve Duty | `percent` | float | 1 | Evaporative canister purge valve duty cycle. |
| `pwm_output_1` | PWM Output 1 | `percent` | float | 0 | Duty cycle of PWM output 1. |
| `pwm_output_2` | PWM Output 2 | `percent` | float | 0 | Duty cycle of PWM output 2. |
| `pwm_output_3` | PWM Output 3 | `percent` | float | 0 | Duty cycle of PWM output 3. |
| `pwm_output_4` | PWM Output 4 | `percent` | float | 0 | Duty cycle of PWM output 4. |
| `pwm_output_5` | PWM Output 5 | `percent` | float | 0 | Duty cycle of PWM output 5. |
| `pwm_output_6` | PWM Output 6 | `percent` | float | 0 | Duty cycle of PWM output 6. |
| `pwm_output_7` | PWM Output 7 | `percent` | float | 0 | Duty cycle of PWM output 7. |
| `pwm_output_8` | PWM Output 8 | `percent` | float | 0 | Duty cycle of PWM output 8. |
| `rev_limit_rpm` | Rev Limit | `rpm` | float | 0 | Currently enforced RPM limit. |
| `rpm` | Engine RPM | `rpm` | float | 10 | Engine speed |
| `rpm_rate` | RPM Rate | `rpm/s` | float | 2 | Rate of change of engine speed. |
| `tgv_position` | Tumble Valve Position | `percent` | float | 0 | Tumble generator valve position. |
| `tgv_position_right` | Tumble Valve Position Right | `percent` | float | 0 | Tumble generator valve position, right bank. |
| `torque_actual` | Engine Torque | `nm` | float | 0 | Delivered engine torque. |
| `torque_indicated` | Indicated Engine Torque | `nm` | float | 0 | Indicated engine torque before losses. |
| `torque_loss` | Engine Torque Loss | `nm` | float | 0 | Torque lost to friction and accessories. |
| `torque_requested` | Torque Requested | `nm` | float | 1 | Requested torque from torque management |
| `torque_theoretical` | Theoretical Engine Torque | `nm` | float | 0 | Theoretical maximum engine torque. |
| `tps` | Throttle Position | `percent` | float | 8 | Throttle position sensor |
| `tps_2` | Throttle Position 2 | `percent` | float | 3 | Secondary throttle position sensor |
| `tps_main` | Throttle Position Main | `percent` | float | 1 | Main throttle position sensor |
| `tps_rate` | Throttle Rate | `percent/s` | float | 1 | Rate of change of throttle position. |
| `tps_sub` | Throttle Position Sub | `percent` | float | 1 | Secondary throttle position sensor |
| `traction_slip_target` | Target Slip | `percent` | float | 0 | Wheel slip target used by traction control. |
| `turbo_speed` | Turbo Speed | `rpm` | float | 1 | Turbocharger shaft speed. |
| `ve_1` | VE Table 1 | `percent` | float | 1 | Volumetric efficiency from table 1. |
| `ve_2` | VE Table 2 | `percent` | float | 1 | Volumetric efficiency from table 2. |
| `vvt_exhaust` | VVT Exhaust Position<br>_aliases: `vvt_exhaust_actual`, `vvt_exhaust_bank1_actual`_ | `degrees` | float | 5 | Variable valve timing exhaust cam position |
| `vvt_exhaust_bank2_actual` | VVT Exhaust Bank 2 Actual | `degrees` | float | 2 | Measured exhaust cam position, bank 2. |
| `vvt_exhaust_bank2_duty` | VVT Exhaust Bank 2 Duty | `percent` | float | 1 | Exhaust cam solenoid duty cycle, bank 2. |
| `vvt_exhaust_bank2_target` | VVT Exhaust Bank 2 Target | `degrees` | float | 0 | Commanded exhaust cam position, bank 2. |
| `vvt_exhaust_duty` | VVT Exhaust Duty | `percent` | float | 1 | Exhaust cam solenoid duty cycle. |
| `vvt_exhaust_target` | VVT Exhaust Target<br>_aliases: `vvt_exhaust_bank1_target`_ | `degrees` | float | 1 | Target exhaust cam position |
| `vvt_intake` | VVT Intake Position<br>_aliases: `vvt_intake_actual`, `vvt_intake_bank1_actual`_ | `degrees` | float | 6 | Variable valve timing intake cam position |
| `vvt_intake_bank2_actual` | VVT Intake Bank 2 Actual | `degrees` | float | 3 | Measured intake cam position, bank 2. |
| `vvt_intake_bank2_duty` | VVT Intake Bank 2 Duty | `percent` | float | 2 | Intake cam solenoid duty cycle, bank 2. |
| `vvt_intake_bank2_target` | VVT Intake Bank 2 Target | `degrees` | float | 1 | Commanded intake cam position, bank 2. |
| `vvt_intake_duty` | VVT Intake Duty | `percent` | float | 3 | Intake cam solenoid duty cycle. |
| `vvt_intake_target` | VVT Intake Target<br>_aliases: `vvt_intake_bank1_target`_ | `degrees` | float | 3 | Target intake cam position |
| `wastegate_duty` | Wastegate Duty Cycle | `percent` | float | 2 | Wastegate solenoid duty cycle |
| `wastegate_duty_primary` | Primary Wastegate Duty | `percent` | float | 1 | Primary wastegate solenoid duty cycle. |
| `wastegate_duty_secondary` | Secondary Wastegate Duty | `percent` | float | 1 | Secondary wastegate solenoid duty cycle. |
| `wastegate_position` | Wastegate Position | `percent` | float | 1 | Wastegate actuator position. |
| `water_injection_duty` | Water Injection Duty | `percent` | float | 1 | Water/methanol injection solenoid duty cycle. |

### fuel

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `afr` | Air-Fuel Ratio | `afr` | float | 9 | Measured AFR from O2 sensor |
| `afr_1` | AFR Bank 1 | `afr` | float | 1 | Air-fuel ratio from bank 1 / sensor 1 |
| `afr_2` | AFR Bank 2 | `afr` | float | 3 | Air-fuel ratio from bank 2 / sensor 2 |
| `afr_external` | PLX AFR<br>_aliases: `plx_afr`_ | `afr` | float | 1 | PLX wideband AFR sensor |
| `afr_target` | AFR Target | `afr` | float | 4 | Target AFR from tune |
| `duty_cycle` | Injector Duty Cycle<br>_aliases: `injector_duty`, `injector_duty_1`_ | `percent` | float | 8 | Injector duty cycle |
| `dwell_actual` | Dwell (Measured) | `milliseconds` | float | 1 | Measured coil dwell, as opposed to commanded. |
| `engine_protection_afr` | Engine Protection AFR Limit | `afr` | float | 1 | AFR threshold used by engine protection. |
| `flex_ethanol` | Ethanol Content | `percent` | float | 2 | Ethanol content from the flex fuel sensor. |
| `fuel_cut` | Fuel Cut | `—` | bool | 1 | Fuel cut active flag |
| `fuel_economy` | Fuel Economy | `l/100km` | float | 0 | Average fuel economy. |
| `fuel_flow` | Fuel Flow Rate | `l/h` | float | 3 | Instantaneous fuel flow rate |
| `fuel_flow_mass` | Fuel Flow Rate (Mass) | `g/s` | float | 1 | Mass fuel flow. Not convertible to volumetric flow without fuel density. |
| `fuel_level` | Fuel Level | `percent` | float | 1 | Fuel remaining in the tank. |
| `fuel_load` | Fuel Load | `percent` | float | 1 | Load axis used for the fuel table. |
| `fuel_used` | Fuel Used | `liters` | float | 1 | Cumulative fuel consumed this session. |
| `fuel_value` | Fuel Value | `—` | float | 1 | Fuel enrichment / VE lookup value |
| `humidity` | Humidity | `percent` | float | 0 | Relative or absolute humidity. |
| `injector_open_time` | Injector Open Time | `milliseconds` | float | 2 | Injector dead time / opening delay. |
| `lambda` | Lambda<br>_aliases: `lambda_1`_ | `lambda` | float | 8 | Lambda value from wideband O2 |
| `lambda_10` | Lambda 10 | `lambda` | float | 0 | Wideband lambda sensor 10. |
| `lambda_11` | Lambda 11 | `lambda` | float | 0 | Wideband lambda sensor 11. |
| `lambda_12` | Lambda 12 | `lambda` | float | 0 | Wideband lambda sensor 12. |
| `lambda_2` | Lambda 2 | `lambda` | float | 5 | Lambda value from second wideband O2 |
| `lambda_3` | Lambda 3 | `lambda` | float | 0 | Wideband lambda sensor 3. |
| `lambda_4` | Lambda 4 | `lambda` | float | 0 | Wideband lambda sensor 4. |
| `lambda_5` | Lambda 5 | `lambda` | float | 0 | Wideband lambda sensor 5. |
| `lambda_6` | Lambda 6 | `lambda` | float | 0 | Wideband lambda sensor 6. |
| `lambda_7` | Lambda 7 | `lambda` | float | 0 | Wideband lambda sensor 7. |
| `lambda_8` | Lambda 8 | `lambda` | float | 0 | Wideband lambda sensor 8. |
| `lambda_9` | Lambda 9 | `lambda` | float | 0 | Wideband lambda sensor 9. |
| `lambda_target` | Lambda Target | `lambda` | float | 4 | Target lambda from ECU tuning |
| `pulse_width` | Injector Pulse Width<br>_aliases: `injector_pw`, `pulse_width_1`_ | `milliseconds` | float | 9 | Injector pulse width |
| `pulse_width_2` | Pulse Width 2 | `milliseconds` | float | 3 | Injector pulse width for stage or bank 2. |
| `pulse_width_3` | Pulse Width 3 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 3. |
| `pulse_width_4` | Pulse Width 4 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 4. |
| `pulse_width_5` | Pulse Width 5 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 5. |
| `pulse_width_6` | Pulse Width 6 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 6. |
| `pulse_width_7` | Pulse Width 7 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 7. |
| `pulse_width_8` | Pulse Width 8 | `milliseconds` | float | 1 | Injector pulse width for stage or bank 8. |
| `stoich_ratio` | Stoichiometric Ratio | `afr` | float | 1 | Stoichiometric air-fuel ratio for the current fuel. |
| `ve` | VE | `percent` | float | 3 | Volumetric Efficiency from VE table |

### ignition

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `antilag_timing_retard` | Anti-Lag Timing Retard | `degrees` | float | 1 | Ignition retard applied by anti-lag. |
| `dwell` | Dwell | `milliseconds` | float | 5 | Coil dwell time |
| `dwell_angle` | Dwell Angle | `degrees` | float | 1 | Coil dwell expressed as crank angle. |
| `ignition_advance` | Ignition Advance | `degrees` | float | 8 | Spark timing advance |
| `ignition_advance_cyl_1` | Ignition Advance Cylinder 1 | `degrees` | float | 2 | Per-cylinder ignition advance for cylinder 1. |
| `ignition_advance_cyl_10` | Ignition Advance Cylinder 10 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 10. |
| `ignition_advance_cyl_11` | Ignition Advance Cylinder 11 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 11. |
| `ignition_advance_cyl_12` | Ignition Advance Cylinder 12 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 12. |
| `ignition_advance_cyl_2` | Ignition Advance Cylinder 2 | `degrees` | float | 2 | Per-cylinder ignition advance for cylinder 2. |
| `ignition_advance_cyl_3` | Ignition Advance Cylinder 3 | `degrees` | float | 1 | Per-cylinder ignition advance for cylinder 3. |
| `ignition_advance_cyl_4` | Ignition Advance Cylinder 4 | `degrees` | float | 1 | Per-cylinder ignition advance for cylinder 4. |
| `ignition_advance_cyl_5` | Ignition Advance Cylinder 5 | `degrees` | float | 1 | Per-cylinder ignition advance for cylinder 5. |
| `ignition_advance_cyl_6` | Ignition Advance Cylinder 6 | `degrees` | float | 1 | Per-cylinder ignition advance for cylinder 6. |
| `ignition_advance_cyl_7` | Ignition Advance Cylinder 7 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 7. |
| `ignition_advance_cyl_8` | Ignition Advance Cylinder 8 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 8. |
| `ignition_advance_cyl_9` | Ignition Advance Cylinder 9 | `degrees` | float | 0 | Per-cylinder ignition advance for cylinder 9. |
| `ignition_advance_trailing` | Trailing Ignition Advance | `degrees` | float | 1 | Trailing spark advance on rotary and dual-plug engines. |
| `ignition_compensation` | Total Ignition Compensation | `degrees` | float | 0 | Total ignition advance adjustment from all corrections. |
| `ignition_correction_clt` | Ignition Correction (Coolant) | `degrees` | float | 1 | Ignition advance correction for coolant temperature. |
| `ignition_correction_iat` | Ignition Correction (IAT) | `degrees` | float | 1 | Ignition advance correction for intake air temperature. |
| `ignition_correction_pid` | Ignition Correction (Idle PID) | `degrees` | float | 1 | Ignition advance correction from the idle PID loop. |
| `ignition_table` | Ignition Table | `degrees` | float | 2 | Base ignition timing from table lookup |
| `ignition_timing_learned` | Learned Ignition Timing | `degrees` | float | 1 | Learned ignition timing correction. |
| `injection_timing` | Injection Timing | `degrees` | float | 1 | Crank angle at which injection occurs. |
| `knock_count` | Knock Count | `count` | int | 2 | Number of knock events detected |
| `knock_level` | Knock Level | `volts` | float | 2 | Knock sensor signal level |
| `knock_level_1` | Knock Level 1 | `volts` | float | 3 | Knock sensor 1 signal level |
| `knock_level_2` | Knock Level 2 | `volts` | float | 3 | Knock sensor 2 signal level |
| `knock_level_3` | Knock Level 3 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 3. |
| `knock_level_4` | Knock Level 4 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 4. |
| `knock_level_5` | Knock Level 5 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 5. |
| `knock_level_6` | Knock Level 6 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 6. |
| `knock_level_7` | Knock Level 7 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 7. |
| `knock_level_8` | Knock Level 8 | `volts` | float | 1 | Knock sensor level for cylinder or sensor 8. |
| `knock_level_db` | Knock Level (dB) | `decibels` | float | 1 | Knock level on a logarithmic dB scale. Not linearly convertible to the voltage form. |
| `launch_timing_retard` | Launch Timing Retard | `degrees` | float | 0 | Ignition retard applied by launch control. |
| `nitrous_timing_retard` | Nitrous Timing Retard | `degrees` | float | 0 | Ignition retard applied while nitrous is flowing. |
| `spark_cut` | Spark Cut | `—` | bool | 1 | Spark cut active flag |

### position

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `distance` | Distance | `meters` | float | 1 | Total distance traveled |
| `gps_altitude` | GPS Altitude | `meters` | float | 1 | GPS altitude above sea level |
| `gps_heading` | GPS Heading | `degrees` | float | 1 | GPS heading/course |
| `gps_latitude` | GPS Latitude | `degrees` | float | 1 | GPS latitude position |
| `gps_longitude` | GPS Longitude | `degrees` | float | 1 | GPS longitude position |
| `trip_distance` | Trip Distance | `kilometers` | float | 1 | Distance travelled since the trip meter was reset. |

### pressure

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `ac_pressure` | A/C Pressure | `kpa` | float | 1 | Air conditioning refrigerant pressure. |
| `barometric_pressure` | Barometric Pressure<br>_aliases: `baro`, `barometric`_ | `kpa` | float | 9 | Atmospheric pressure |
| `boost` | Boost Pressure | `kpa` | float | 5 | Boost pressure (gauge) |
| `boost_duty` | Boost Duty | `percent` | float | 4 | Boost solenoid duty cycle |
| `boost_error` | Boost Error | `kpa` | float | 0 | Difference between boost target and actual. |
| `boost_external` | PLX Boost<br>_aliases: `plx_boost`_ | `bar` | float | 1 | PLX boost/vacuum sensor |
| `boost_limit` | Boost Limit | `kpa` | float | 0 | Upper boost limit currently enforced. |
| `boost_target` | Boost Target<br>_aliases: `target_boost`_ | `kpa` | float | 6 | Target boost from boost control |
| `brake_booster_pressure` | Brake Booster Pressure | `kpa` | float | 1 | Vacuum pressure in the brake booster. |
| `brake_pressure_front` | Brake Pressure Front | `kpa` | float | 1 | Front brake line pressure |
| `brake_pressure_rear` | Brake Pressure Rear | `kpa` | float | 1 | Rear brake line pressure |
| `compressor_discharge_pressure` | Compressor Discharge Pressure | `kpa` | float | 1 | Pressure at the turbo compressor outlet. |
| `coolant_pressure` | Coolant Pressure | `kpa` | float | 0 | Coolant system pressure. |
| `crank_case_pressure` | Crankcase Pressure | `kpa` | float | 0 | Crankcase pressure. |
| `engine_protection_map` | Engine Protection MAP Limit | `kpa` | float | 1 | Manifold pressure limit imposed by engine protection. |
| `engine_protection_oil` | Engine Protection Oil Limit | `kpa` | float | 1 | Oil pressure threshold used by engine protection. |
| `exhaust_manifold_pressure` | Exhaust Manifold Pressure | `kpa` | float | 1 | Pressure in the exhaust manifold. |
| `flex_boost_correction` | Flex Boost Correction | `kpa` | float | 1 | Boost target adjustment from ethanol content. |
| `fuel_pressure` | Fuel Pressure | `kpa` | float | 7 | Fuel rail pressure |
| `fuel_pressure_high` | High Fuel Pressure | `bar` | float | 2 | Direct-injection high-pressure rail pressure. |
| `fuel_tank_pressure` | Fuel Tank Pressure | `kpa` | float | 1 | Evaporative system fuel tank pressure. |
| `gearbox_line_pressure` | Gearbox Line Pressure | `kpa` | float | 0 | Transmission hydraulic line pressure. |
| `manifold_relative_pressure` | Manifold Relative Pressure | `kpa` | float | 1 | Manifold pressure relative to atmosphere. |
| `map` | Manifold Pressure | `kpa` | float | 10 | Manifold absolute pressure |
| `map_rate` | MAP Rate | `kpa/s` | float | 1 | Rate of change of manifold pressure. |
| `mgp` | Manifold Gauge Pressure | `kpa` | float | 1 | Manifold gauge pressure (relative) |
| `nitrous_bottle_pressure` | Nitrous Bottle Pressure | `kpa` | float | 0 | Nitrous bottle pressure. |
| `oil_pressure` | Oil Pressure | `kpa` | float | 7 | Engine oil pressure |
| `throttle_inlet_pressure` | Throttle Inlet Pressure | `kpa` | float | 1 | Pressure upstream of the throttle body. |
| `tyre_pressure_fl` | Tyre Pressure Front Left | `kpa` | float | 0 | Front Left tyre pressure. |
| `tyre_pressure_fr` | Tyre Pressure Front Right | `kpa` | float | 0 | Front Right tyre pressure. |
| `tyre_pressure_rl` | Tyre Pressure Rear Left | `kpa` | float | 0 | Rear Left tyre pressure. |
| `tyre_pressure_rr` | Tyre Pressure Rear Right | `kpa` | float | 0 | Rear Right tyre pressure. |
| `wastegate_pressure` | Wastegate Pressure | `kpa` | float | 0 | Pressure at the wastegate actuator. |

### rotation

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `gyro_pitch` | Pitch Rate | `deg/s` | float | 1 | Pitch rotation rate |
| `gyro_roll` | Roll Rate | `deg/s` | float | 1 | Roll rotation rate |
| `gyro_yaw` | Yaw Rate | `deg/s` | float | 2 | Yaw rotation rate |

### speed

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `cruise_speed_error` | Cruise Speed Error | `kph` | float | 0 | Cruise control speed error. |
| `cruise_target_speed` | Cruise Target Speed | `kph` | float | 1 | Cruise control target speed. |
| `gps_speed` | GPS Speed | `kph` | float | 0 | Ground speed from GPS. |
| `vehicle_speed` | Vehicle Speed | `kph` | float | 10 | Vehicle ground speed |
| `wheel_speed_driven` | Driven Wheel Speed | `kph` | float | 0 | Average speed of the driven wheels. |
| `wheel_speed_fl` | Wheel Speed FL | `kph` | float | 2 | Front left wheel speed |
| `wheel_speed_fr` | Wheel Speed FR | `kph` | float | 2 | Front right wheel speed |
| `wheel_speed_rl` | Wheel Speed RL | `kph` | float | 2 | Rear left wheel speed |
| `wheel_speed_rr` | Wheel Speed RR | `kph` | float | 2 | Rear right wheel speed |
| `wheel_speed_undriven` | Undriven Wheel Speed | `kph` | float | 0 | Average speed of the undriven wheels. |

### status

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `ac_relay` | A/C Relay | `—` | bool | 2 | Air conditioning clutch relay state. |
| `ac_request` | A/C Request | `—` | bool | 2 | Air conditioning has been requested. |
| `antilag_active` | Anti-Lag Active | `—` | bool | 0 | Anti-lag system is active. |
| `boost_cut_active` | Boost Cut Active | `—` | bool | 1 | Overboost cut is active. |
| `brake_switch` | Brake Switch | `—` | bool | 2 | Brake pedal switch state. |
| `check_engine_light` | Check Engine Light | `—` | bool | 1 | MIL / check engine lamp is commanded on. |
| `closed_loop_active` | Closed Loop Active | `—` | bool | 1 | Closed-loop lambda control is active. |
| `clutch_switch` | Clutch Switch | `—` | bool | 3 | Clutch pedal switch state. |
| `digital_input_1` | Digital Input 1 | `—` | bool | 0 | Generic digital input 1. |
| `digital_input_10` | Digital Input 10 | `—` | bool | 0 | Generic digital input 10. |
| `digital_input_11` | Digital Input 11 | `—` | bool | 0 | Generic digital input 11. |
| `digital_input_12` | Digital Input 12 | `—` | bool | 0 | Generic digital input 12. |
| `digital_input_13` | Digital Input 13 | `—` | bool | 0 | Generic digital input 13. |
| `digital_input_14` | Digital Input 14 | `—` | bool | 0 | Generic digital input 14. |
| `digital_input_15` | Digital Input 15 | `—` | bool | 0 | Generic digital input 15. |
| `digital_input_16` | Digital Input 16 | `—` | bool | 0 | Generic digital input 16. |
| `digital_input_2` | Digital Input 2 | `—` | bool | 0 | Generic digital input 2. |
| `digital_input_3` | Digital Input 3 | `—` | bool | 0 | Generic digital input 3. |
| `digital_input_4` | Digital Input 4 | `—` | bool | 0 | Generic digital input 4. |
| `digital_input_5` | Digital Input 5 | `—` | bool | 0 | Generic digital input 5. |
| `digital_input_6` | Digital Input 6 | `—` | bool | 0 | Generic digital input 6. |
| `digital_input_7` | Digital Input 7 | `—` | bool | 0 | Generic digital input 7. |
| `digital_input_8` | Digital Input 8 | `—` | bool | 0 | Generic digital input 8. |
| `digital_input_9` | Digital Input 9 | `—` | bool | 0 | Generic digital input 9. |
| `engine_protection_active` | Engine Protection Active | `—` | bool | 0 | An engine protection strategy is intervening. |
| `engine_running` | Engine Running | `—` | bool | 0 | Engine is running. |
| `fan_relay_1` | Fan Relay 1 | `—` | bool | 2 | Cooling fan relay 1 state. |
| `fan_relay_2` | Fan Relay 2 | `—` | bool | 1 | Cooling fan relay 2 state. |
| `flat_shift_active` | Flat Shift Active | `—` | bool | 0 | Flat shift / shift cut is active. |
| `fuel_pump_relay` | Fuel Pump Relay | `—` | bool | 2 | Fuel pump relay state. |
| `fuel_staging_active` | Fuel Staging Active | `—` | bool | 1 | Staged (secondary) injectors are firing. |
| `head_light_state` | Head Light | `—` | bool | 0 | Head light output state. |
| `high_beam_light_state` | High Beam | `—` | bool | 0 | High beam output state. |
| `high_egt_warning` | High EGT Warning | `—` | bool | 0 | Exhaust gas temperature above the warning threshold. |
| `high_water_temp_warning` | High Coolant Temp Warning | `—` | bool | 0 | Coolant temperature above the warning threshold. |
| `idle_control_active` | Idle Control Active | `—` | bool | 2 | Closed-loop idle control is active. |
| `knock_detected` | Knock Detected | `—` | bool | 1 | Knock was detected on the most recent event. |
| `lambda_protect_active` | Lambda Protection Active | `—` | bool | 0 | Lean-protection strategy is intervening. |
| `launch_control_active` | Launch Control Active | `—` | bool | 1 | Launch control is engaged. |
| `launch_control_armed` | Launch Control Armed | `—` | bool | 0 | Launch control is armed but not yet cutting. |
| `left_indicator_state` | Left Indicator | `—` | bool | 0 | Left indicator output state. |
| `logging_active` | Logging Active | `—` | bool | 0 | ECU internal logging is running. |
| `low_oil_pressure_warning` | Low Oil Pressure Warning | `—` | bool | 0 | Oil pressure below the warning threshold. |
| `main_relay` | Main Relay | `—` | bool | 0 | ECU main relay state. |
| `neutral_switch` | Neutral Position Switch | `—` | bool | 1 | Transmission is in neutral. |
| `nitrous_armed` | Nitrous Armed | `—` | bool | 0 | Nitrous system is armed. |
| `nitrous_stage_1_active` | Nitrous Stage 1 Active | `—` | bool | 1 | Nitrous stage 1 is flowing. |
| `nitrous_stage_2_active` | Nitrous Stage 2 Active | `—` | bool | 0 | Nitrous stage 2 is flowing. |
| `o2_heater` | O2 Heater | `—` | bool | 0 | Oxygen sensor heater state. |
| `overheat_warning` | Overheat Warning | `—` | bool | 1 | Engine overheat warning is active. |
| `overrun_active` | Overrun Cut Active | `—` | bool | 2 | Deceleration fuel cut-off is active. |
| `park_light_state` | Park Light | `—` | bool | 0 | Park light output state. |
| `rev_limiter_active` | Rev Limiter Active | `—` | bool | 1 | RPM limiter is cutting. |
| `right_indicator_state` | Right Indicator | `—` | bool | 0 | Right indicator output state. |
| `shift_light` | Shift Light | `—` | bool | 0 | Shift light is commanded on. |
| `speed_limiter_active` | Speed Limiter Active | `—` | bool | 0 | Vehicle speed limiter is active. |
| `starter_signal` | Starter Signal | `—` | bool | 2 | Starter engaged. |
| `throttle_blip_active` | Throttle Blip Active | `—` | bool | 0 | Downshift throttle blip is active. |
| `traction_control_active` | Traction Control Active | `—` | bool | 1 | Traction control is intervening. |

### suspension

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `suspension_fl` | Suspension FL | `millimeters` | float | 1 | Front left suspension travel |
| `suspension_fr` | Suspension FR | `millimeters` | float | 1 | Front right suspension travel |
| `suspension_rl` | Suspension RL | `millimeters` | float | 1 | Rear left suspension travel |
| `suspension_rr` | Suspension RR | `millimeters` | float | 1 | Rear right suspension travel |

### system

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `active_boost_table` | Active Boost Table | `count` | int | 0 | Index of the boost table currently selected. |
| `active_tune` | Active Tune | `count` | int | 0 | Index of the tune or map currently selected. |
| `cpu_load` | CPU Load | `percent` | float | 1 | ECU CPU utilization |
| `ecu_temp` | ECU Temperature | `celsius` | float | 3 | ECU internal temperature |
| `engine_mode` | Engine Mode | `code` | int | 1 | Current engine operating mode. |
| `engine_protection_severity` | Engine Protection Severity | `count` | int | 0 | Severity level of the active engine protection strategy. |
| `engine_run_time` | Engine Run Time | `seconds` | float | 1 | Seconds the engine has been running. |
| `error_code` | Error Code | `code` | int | 3 | Active error/fault code |
| `error_count` | Active Error Count | `count` | int | 1 | Number of currently active error codes. |
| `firmware_version` | Firmware Version | `version` | string | 1 | ECU firmware version. |
| `free_ram` | Free RAM | `bytes` | float | 1 | Available memory |
| `fuel_cut_reason` | Fuel Cut Reason | `code` | int | 1 | Code identifying why fuel is being cut. |
| `gps_satellites` | GPS Satellites | `count` | int | 1 | Number of GPS satellites in view |
| `knock_cylinder` | Last Knock Cylinder | `count` | int | 0 | Cylinder on which knock was last detected. |
| `loops_per_second` | Loops Per Second | `hz` | float | 2 | ECU loop rate |
| `misfire_count` | Misfire Count | `count` | int | 1 | Total misfire events detected. |
| `spark_cut_reason` | Spark Cut Reason | `code` | int | 1 | Code identifying why spark is being cut. |
| `sync_level` | Sync Level | `count` | int | 1 | Crank/cam synchronisation level. |
| `sync_loss_count` | Sync Loss Count | `count` | int | 1 | Times the ECU lost engine position sync. |
| `time` | Time | `seconds` | float | 4 | Log timestamp in seconds |
| `time_since_cranking` | Time Since Cranking | `seconds` | float | 1 | Seconds since the engine last cranked. |
| `trigger_counter` | Trigger Counter | `count` | int | 0 | Trigger tooth counter. |
| `trigger_error_counter` | Trigger Errors | `count` | int | 1 | Trigger synchronization error count |
| `uptime` | ECU Uptime | `seconds` | float | 2 | Seconds since the ECU powered on. |
| `warning_count` | Warning Count | `count` | int | 1 | Running count of warnings raised by the ECU. |

### temperature

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `ambient_temp` | Ambient Temperature | `celsius` | float | 1 | Ambient air temperature. |
| `aux_temp_1` | Aux Temperature 1 | `celsius` | float | 1 | Auxiliary temperature sensor 1 |
| `aux_temp_2` | Aux Temperature 2 | `celsius` | float | 1 | Auxiliary temperature sensor 2 |
| `aux_temp_3` | Aux Temperature 3 | `celsius` | float | 0 | Auxiliary temperature sensor 3. |
| `aux_temp_4` | Aux Temperature 4 | `celsius` | float | 0 | Auxiliary temperature sensor 4. |
| `charge_temp` | Charge Temperature | `celsius` | float | 2 | Intake charge temperature estimate |
| `compressor_outlet_temp` | Compressor Outlet Temperature | `celsius` | float | 1 | Air temperature at the compressor outlet. |
| `coolant_temp` | Coolant Temperature | `celsius` | float | 10 | Engine coolant temperature |
| `differential_temp` | Differential Temperature | `celsius` | float | 0 | Differential oil temperature. |
| `egt_1` | EGT Cylinder 1<br>_aliases: `egt`, `exhaust_temp`_ | `celsius` | float | 6 | Exhaust gas temperature for cylinder 1 |
| `egt_10` | EGT Cylinder 10 | `celsius` | float | 0 | Exhaust gas temperature, cylinder or position 10. |
| `egt_11` | EGT Cylinder 11 | `celsius` | float | 0 | Exhaust gas temperature, cylinder or position 11. |
| `egt_12` | EGT Cylinder 12 | `celsius` | float | 0 | Exhaust gas temperature, cylinder or position 12. |
| `egt_2` | EGT Cylinder 2<br>_aliases: `exhaust_temp_2`_ | `celsius` | float | 3 | Exhaust gas temperature for cylinder 2 |
| `egt_3` | EGT Cylinder 3<br>_aliases: `exhaust_temp_3`_ | `celsius` | float | 2 | Exhaust gas temperature, cylinder or position 3. |
| `egt_4` | EGT Cylinder 4<br>_aliases: `exhaust_temp_4`_ | `celsius` | float | 2 | Exhaust gas temperature, cylinder or position 4. |
| `egt_5` | EGT Cylinder 5 | `celsius` | float | 1 | Exhaust gas temperature, cylinder or position 5. |
| `egt_6` | EGT Cylinder 6 | `celsius` | float | 1 | Exhaust gas temperature, cylinder or position 6. |
| `egt_7` | EGT Cylinder 7 | `celsius` | float | 1 | Exhaust gas temperature, cylinder or position 7. |
| `egt_8` | EGT Cylinder 8 | `celsius` | float | 1 | Exhaust gas temperature, cylinder or position 8. |
| `egt_9` | EGT Cylinder 9 | `celsius` | float | 0 | Exhaust gas temperature, cylinder or position 9. |
| `egt_external` | PLX EGT<br>_aliases: `plx_egt`_ | `celsius` | float | 1 | PLX exhaust gas temperature |
| `egt_highest` | EGT Highest | `celsius` | float | 0 | Highest reading across all EGT sensors. |
| `egt_spread` | EGT Spread | `celsius` | float | 0 | Difference between the highest and lowest EGT sensor. |
| `engine_protection_coolant` | Engine Protection Coolant Limit | `celsius` | float | 1 | Coolant temperature threshold used by engine protection. |
| `fuel_temp` | Fuel Temperature | `celsius` | float | 6 | Fuel temperature |
| `gearbox_temp` | Gearbox Temperature | `celsius` | float | 2 | Transmission oil temperature |
| `iat` | Intake Air Temperature | `celsius` | float | 10 | Intake air temperature |
| `oil_temp` | Oil Temperature | `celsius` | float | 7 | Engine oil temperature |
| `tyre_temp_fl` | Tyre Temperature Front Left | `celsius` | float | 0 | Front Left tyre temperature. |
| `tyre_temp_fr` | Tyre Temperature Front Right | `celsius` | float | 0 | Front Right tyre temperature. |
| `tyre_temp_rl` | Tyre Temperature Rear Left | `celsius` | float | 0 | Rear Left tyre temperature. |
| `tyre_temp_rr` | Tyre Temperature Rear Right | `celsius` | float | 0 | Rear Right tyre temperature. |
| `wideband_temp` | Wideband Sensor Temperature | `celsius` | float | 1 | Wideband oxygen sensor element temperature. |

### timing

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `best_lap_time` | Best Lap Time | `seconds` | float | 1 | Best lap time in session |
| `lap_number` | Lap Number | `lap` | int | 1 | Current lap number |
| `lap_time` | Lap Time | `seconds` | float | 1 | Current lap time |

### traction

| id | name | unit | type | used by | description |
| --- | --- | --- | --- | --- | --- |
| `traction_cut` | Traction Cut | `percent` | float | 2 | Traction control ignition cut percentage |
| `traction_slip` | Traction Slip | `percent` | float | 1 | Wheel slip percentage |

---

## Deprecated aliases

Recognised for backward compatibility. Do not use in new adapters.

| deprecated | use instead |
| --- | --- |
| `an_volt_1` | `analog_input_1` |
| `an_volt_2` | `analog_input_2` |
| `an_volt_3` | `analog_input_3` |
| `an_volt_4` | `analog_input_4` |
| `baro` | `barometric_pressure` |
| `barometric` | `barometric_pressure` |
| `dbw_position` | `etb_position` |
| `dbw_target` | `etb_target` |
| `e_throttle_position` | `etb_position` |
| `e_throttle_target` | `etb_target` |
| `egt` | `egt_1` |
| `exhaust_temp` | `egt_1` |
| `exhaust_temp_2` | `egt_2` |
| `exhaust_temp_3` | `egt_3` |
| `exhaust_temp_4` | `egt_4` |
| `idle_target` | `idle_target_rpm` |
| `injector_duty` | `duty_cycle` |
| `injector_duty_1` | `duty_cycle` |
| `injector_pw` | `pulse_width` |
| `lambda_1` | `lambda` |
| `plx_afr` | `afr_external` |
| `plx_boost` | `boost_external` |
| `plx_egt` | `egt_external` |
| `pulse_width_1` | `pulse_width` |
| `target_boost` | `boost_target` |
| `vvt_exhaust_actual` | `vvt_exhaust` |
| `vvt_exhaust_bank1_actual` | `vvt_exhaust` |
| `vvt_exhaust_bank1_target` | `vvt_exhaust_target` |
| `vvt_intake_actual` | `vvt_intake` |
| `vvt_intake_bank1_actual` | `vvt_intake` |
| `vvt_intake_bank1_target` | `vvt_intake_target` |

---

## Split channels

These were one ID carrying two physically different measurements. They are not
inter-convertible, so they became separate channels rather than a unit
conversion.

| id | split from | unit | why |
| --- | --- | --- | --- |
| `fuel_flow_mass` | `fuel_flow` | `g/s` | Mass fuel flow. Not convertible to volumetric flow without fuel density. |
| `knock_level_db` | `knock_level` | `decibels` | Knock level on a logarithmic dB scale. Not linearly convertible to the voltage form. |
