# EEP (Extended Environment Protocol) Tasks

43 tasks across 6 phases. Each task is atomic: one function, one struct, or one cap handler.

## Phase 1: Data types

- [ ] Define `SkyData` struct with sun position/direction, moon position/direction, ambient/diffuse/specular colors, cloud coverage/density/scale/glow, and horizon colors. Acceptance: struct compiles, is public, derives Debug/Clone/Default.
- [ ] Define `WaterData` struct with water color, fog color/density, wave scale, wave direction, and Fresnel/normal-scale fields. Acceptance: struct compiles, is public, derives Debug/Clone/Default.
- [ ] Define `DayCycleTrack` entries mapping a time-of-day fraction to sky and water keyframe UUIDs. Acceptance: entries sort by time fraction and compile as part of DayCycle.
- [ ] Define `DayCycle` struct containing day length/offset track metadata and ordered sky+water keyframe tracks. Acceptance: struct compiles, is public, derives Debug/Clone/Default.
- [ ] Define `ViewerEnvironment` struct aggregating SkyData, WaterData, DayCycle, and optional environment asset UUID. Acceptance: struct compiles, is public, derives Debug/Clone/Default.
- [ ] Implement `SkyData::to_llsd` serializing all fields in OpenSim-compatible LLSD map keys. Acceptance: output contains keys sun_position, moon_position, ambient, colors, clouds.
- [ ] Implement `WaterData::to_llsd` serializing all water fields in OpenSim-compatible LLSD map keys. Acceptance: output contains keys water_color, fog_color, fog_density, wave_scale.
- [ ] Implement `DayCycle::to_llsd` serializing track entries as time-ordered LLSD arrays. Acceptance: output contains a tracks array with time, sky, and water keys.
- [ ] Implement `ViewerEnvironment::to_llsd` composing sky, water, day cycle, and asset UUID LLSD maps. Acceptance: round-trip serialization compiles and includes sky, water, and day_cycle maps.
- [ ] Implement `SkyData::from_llsd` validating and parsing all fields with defaults for missing keys. Acceptance: missing keys do not panic and produce Default values.
- [ ] Implement `WaterData::from_llsd` validating and parsing all fields with defaults for missing keys. Acceptance: missing keys do not panic and produce Default values.
- [ ] Implement `DayCycle::from_llsd` parsing ordered track entries and rejecting invalid time fractions. Acceptance: invalid input returns an error and valid input round-trips.
- [ ] Implement `ViewerEnvironment::from_llsd` composing component parsers and default asset fallback. Acceptance: malformed nested maps return an error rather than panicking.
- [ ] Define default asset UUID constants: DEFAULT_SKY_ASSET_UUID = 3ae23978-ac82-bcf3-a9cb-ba6e52dcb9ad, DEFAULT_WATER_ASSET_UUID = 59d1a851-47e7-0e5f-1ed7-6b715154f41a, DEFAULT_DAY_CYCLE_ASSET_UUID = 5646d39e-d3d7-6aff-ed71-30fc87d64a92. Acceptance: constants parse as valid UUIDs and match exactly.
- [ ] Implement `ViewerEnvironment::default_region` constructing the default sky, water, and day cycle using the default asset UUIDs. Acceptance: returned environment references all three required UUIDs.

## Phase 2: Storage

- [ ] Define `RegionEnvironmentStore` trait with load, save, and delete operations for region environments. Acceptance: trait is object-safe and compiles.
- [ ] Implement region environment load function returning ViewerEnvironment, falling back to default_region when no row exists. Acceptance: empty storage returns the default environment.
- [ ] Implement region environment save function persisting serialized ViewerEnvironment keyed by region ID. Acceptance: saved environment can be loaded unchanged.
- [ ] Implement per-parcel environment load function keyed by region ID and parcel local ID. Acceptance: missing parcel override returns None.
- [ ] Implement per-parcel environment save function keyed by region ID and parcel local ID. Acceptance: saved override can be loaded unchanged.
- [ ] Implement per-parcel environment delete function removing the override row. Acceptance: deleted override subsequently loads as None.
- [ ] Add `allow_environment_override: bool` to region settings with default false. Acceptance: default settings have override disabled.

## Phase 3: Cap handlers

- [ ] Implement EnvironmentSettings GET handler returning real region environment converted to legacy WindLight LLSD. Acceptance: response includes non-placeholder sky and water values.
- [ ] Implement EnvironmentSettings POST handler parsing legacy WindLight LLSD into ViewerEnvironment and saving it for the region. Acceptance: accepted POST updates subsequent GET output.
- [ ] Implement ExtEnvironment GET handler returning region or parcel environment LLSD, or default when unspecified. Acceptance: GET for missing name returns default asset UUIDs.
- [ ] Implement ExtEnvironment GET parcel-path handling using parcel local ID and AllowEnvironmentOverride. Acceptance: override is ignored when disabled and returned when enabled.
- [ ] Implement ExtEnvironment PUT handler validating and storing a new or updated environment. Acceptance: PUT followed by GET returns equivalent serialized environment.
- [ ] Implement ExtEnvironment POST handler applying an existing environment name to the region or a parcel override when permitted. Acceptance: POST with valid name updates active environment selection.
- [ ] Implement ExtEnvironment DELETE handler removing a parcel override or named environment. Acceptance: DELETE makes subsequent GET fall back to region/default environment.

## Phase 4: Day/night cycle

- [ ] Implement `compute_day_fraction(unix_time: i64, day_length_seconds: u32, day_offset_seconds: i32) -> f64`. Acceptance: zero-length is rejected, one full period maps to 1.0.
- [ ] Implement `compute_sun_direction(day_fraction: f64, altitude: f64, azimuth_offset: f64) -> Vector3`. Acceptance: fraction 0.0 and 1.0 produce consistent opposite-hemisphere results.
- [ ] Implement `compute_moon_direction(day_fraction: f64, altitude: f64, azimuth_offset: f64) -> Vector3`. Acceptance: moon direction differs from sun direction for the same fraction.
- [ ] Implement UpdateEnvTime tick computing current day fraction and updating region environment time state. Acceptance: repeated ticks advance fraction according to elapsed wall time.
- [ ] Implement SendViewerTime function broadcasting computed day fraction and sun/moon directions to connected viewers. Acceptance: generated message contains day fraction and both direction vectors.

## Phase 5: LSL wiring

- [ ] Implement `llGetRegionDayLength` builtin returning region DayLength as integer seconds. Acceptance: returns configured length for the invoking region.
- [ ] Implement `llGetRegionDayOffset` builtin returning region DayOffset as integer seconds. Acceptance: returns configured offset for the invoking region.
- [ ] Implement `llGetSunDirection` builtin returning computed sun direction vector. Acceptance: vector magnitude is within 1e-6 of 1.0.
- [ ] Implement `llGetMoonDirection` builtin returning computed moon direction vector. Acceptance: vector magnitude is within 1e-6 of 1.0.
- [ ] Implement environment refresh helper sending updated WindLight/EEP data after environment changes. Acceptance: helper serializes current ViewerEnvironment and calls viewer broadcast once.

## Phase 6: Integration test

- [ ] Add integration test creating a region, setting an environment, and verifying EnvironmentSettings GET returns saved non-default values. Acceptance: test passes without network dependencies.
- [ ] Add integration test exercising ExtEnvironment PUT/GET/POST/DELETE and per-parcel override fallback. Acceptance: test verifies default UUID 3ae23978-ac82-bcf3-a9cb-ba6e52dcb9ad after deletion.
- [ ] Add integration test invoking LSL llGetRegionDayLength, llGetRegionDayOffset, llGetSunDirection, and llGetMoonDirection builtins. Acceptance: test validates numeric ranges and unit vectors.
