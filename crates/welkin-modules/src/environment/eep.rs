use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Llsd {
    Undefined,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    String(String),
    Uuid(String),
    Date(String),
    Uri(String),
    Binary(Vec<u8>),
    Array(Vec<Llsd>),
    Map(BTreeMap<String, Llsd>),
}

#[allow(dead_code)]
impl Llsd {
    pub fn map() -> Self {
        Self::Map(BTreeMap::new())
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Llsd) {
        if let Self::Map(map) = self {
            map.insert(key.into(), value);
        }
    }

    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, Llsd>> {
        match self {
            Self::Map(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(value) => Some(*value),
            Self::Integer(value) => Some(*value as f64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Uuid(value) | Self::Date(value) | Self::Uri(value) => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Llsd]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }
}

#[allow(dead_code)]
impl From<&Llsd> for Llsd {
    fn from(value: &Llsd) -> Self {
        value.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentError(pub String);

#[allow(dead_code)]
impl std::fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[allow(dead_code)]
impl std::error::Error for EnvironmentError {}

pub const EEP_PHASE: u32 = 1;
pub const PHASE: u32 = 1;
pub const DEFAULT_DAY_LENGTH: f64 = 14400.0;
pub const DEFAULT_TRACK_TIME: f64 = 0.0;
pub const DEFAULT_REGION_NAME: &str = "Default Region";

#[derive(Debug, Clone, PartialEq)]
pub struct SkyData {
    pub name: String,
    pub ambient: [f32; 3],
    pub blue_density: [f32; 3],
    pub blue_horizon: [f32; 3],
    pub cloud_color: [f32; 3],
    pub cloud_pos_density_1: [f32; 2],
    pub cloud_pos_density_2: [f32; 2],
    pub cloud_scale: f32,
    pub cloud_shadow: f32,
    pub cloud_sun: [f32; 3],
    pub haze_density: f32,
    pub haze_horizon: f32,
    pub light_color: [f32; 3],
    pub light_direction: [f32; 3],
    pub max_y: f32,
    pub star_brightness: f32,
    pub sun_color: [f32; 3],
    pub sunlight_color: [f32; 3],
}

#[allow(dead_code)]
impl Default for SkyData {
    fn default() -> Self {
        Self {
            name: "Default Sky".to_string(),
            ambient: [0.35, 0.35, 0.35],
            blue_density: [0.15, 0.35, 0.75],
            blue_horizon: [0.35, 0.55, 0.85],
            cloud_color: [1.0, 1.0, 1.0],
            cloud_pos_density_1: [0.5, 0.5],
            cloud_pos_density_2: [0.6, 0.6],
            cloud_scale: 0.42,
            cloud_shadow: 0.35,
            cloud_sun: [1.0, 0.95, 0.85],
            haze_density: 0.25,
            haze_horizon: 0.2,
            light_color: [0.8, 0.85, 0.9],
            light_direction: [0.3, -0.6, 0.2],
            max_y: 64.0,
            star_brightness: 0.0,
            sun_color: [1.0, 0.9, 0.7],
            sunlight_color: [0.9, 0.9, 0.8],
        }
    }
}

#[allow(dead_code)]
impl SkyData {
    pub fn to_llsd(&self) -> Llsd {
        let mut map = BTreeMap::new();
        map.insert("name".to_string(), Llsd::String(self.name.clone()));
        map.insert(
            "ambient".to_string(),
            Llsd::Array(self.ambient.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "blue_density".to_string(),
            Llsd::Array(self.blue_density.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "blue_horizon".to_string(),
            Llsd::Array(self.blue_horizon.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "cloud_color".to_string(),
            Llsd::Array(self.cloud_color.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "cloud_pos_density_1".to_string(),
            Llsd::Array(
                self.cloud_pos_density_1
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        map.insert(
            "cloud_pos_density_2".to_string(),
            Llsd::Array(
                self.cloud_pos_density_2
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        map.insert("cloud_scale".to_string(), Llsd::Real(self.cloud_scale as f64));
        map.insert("cloud_shadow".to_string(), Llsd::Real(self.cloud_shadow as f64));
        map.insert(
            "cloud_sun".to_string(),
            Llsd::Array(self.cloud_sun.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert("haze_density".to_string(), Llsd::Real(self.haze_density as f64));
        map.insert("haze_horizon".to_string(), Llsd::Real(self.haze_horizon as f64));
        map.insert(
            "light_color".to_string(),
            Llsd::Array(self.light_color.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "light_direction".to_string(),
            Llsd::Array(
                self.light_direction
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        map.insert("max_y".to_string(), Llsd::Real(self.max_y as f64));
        map.insert(
            "star_brightness".to_string(),
            Llsd::Real(self.star_brightness as f64),
        );
        map.insert(
            "sun_color".to_string(),
            Llsd::Array(self.sun_color.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert(
            "sunlight_color".to_string(),
            Llsd::Array(
                self.sunlight_color
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        Llsd::Map(map)
    }

    pub fn from_llsd(input: impl Into<Llsd>) -> Result<Self, EnvironmentError> {
        let mut sky = Self::default();
        let value = input.into();
        let map = value
            .as_map()
            .ok_or_else(|| EnvironmentError("expected LLSD map for SkyData".to_string()))?;

        if let Some(name) = map.get("name").and_then(Llsd::as_string) {
            sky.name = name.to_string();
        }
        if let Some(values) = map.get("ambient").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.ambient);
        }
        if let Some(values) = map.get("blue_density").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.blue_density);
        }
        if let Some(values) = map.get("blue_horizon").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.blue_horizon);
        }
        if let Some(values) = map.get("cloud_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.cloud_color);
        }
        if let Some(values) = map.get("cloud_pos_density_1").and_then(Llsd::as_array) {
            copy_real_array_2(values, &mut sky.cloud_pos_density_1);
        }
        if let Some(values) = map.get("cloud_pos_density_2").and_then(Llsd::as_array) {
            copy_real_array_2(values, &mut sky.cloud_pos_density_2);
        }
        if let Some(value) = map.get("cloud_scale").and_then(Llsd::as_f64) {
            sky.cloud_scale = value as f32;
        }
        if let Some(value) = map.get("cloud_shadow").and_then(Llsd::as_f64) {
            sky.cloud_shadow = value as f32;
        }
        if let Some(values) = map.get("cloud_sun").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.cloud_sun);
        }
        if let Some(value) = map.get("haze_density").and_then(Llsd::as_f64) {
            sky.haze_density = value as f32;
        }
        if let Some(value) = map.get("haze_horizon").and_then(Llsd::as_f64) {
            sky.haze_horizon = value as f32;
        }
        if let Some(values) = map.get("light_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.light_color);
        }
        if let Some(values) = map.get("light_direction").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.light_direction);
        }
        if let Some(value) = map.get("max_y").and_then(Llsd::as_f64) {
            sky.max_y = value as f32;
        }
        if let Some(value) = map.get("star_brightness").and_then(Llsd::as_f64) {
            sky.star_brightness = value as f32;
        }
        if let Some(values) = map.get("sun_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.sun_color);
        }
        if let Some(values) = map.get("sunlight_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut sky.sunlight_color);
        }

        Ok(sky)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaterData {
    pub name: String,
    pub water_color: [f32; 3],
    pub water_fog_density: f32,
    pub water_fog_color: [f32; 3],
    pub under_water_visibility: f32,
    pub reflection_wavelet_scale: [f32; 2],
    pub fresnel_scale: f32,
    pub fresnel_offset: f32,
    pub surface_height: f32,
    pub large_wave_speed: f32,
    pub small_wave_speed: f32,
}

#[allow(dead_code)]
impl Default for WaterData {
    fn default() -> Self {
        Self {
            name: "Default Water".to_string(),
            water_color: [0.12, 0.35, 0.45],
            water_fog_density: 2.0,
            water_fog_color: [0.15, 0.4, 0.45],
            under_water_visibility: 20.0,
            reflection_wavelet_scale: [2.0, 2.0],
            fresnel_scale: 1.33,
            fresnel_offset: 0.1,
            surface_height: 0.0,
            large_wave_speed: 1.0,
            small_wave_speed: 1.3,
        }
    }
}

#[allow(dead_code)]
impl WaterData {
    pub fn to_llsd(&self) -> Llsd {
        let mut map = BTreeMap::new();
        map.insert("name".to_string(), Llsd::String(self.name.clone()));
        map.insert(
            "water_color".to_string(),
            Llsd::Array(self.water_color.iter().map(|v| Llsd::Real(*v as f64)).collect()),
        );
        map.insert("water_fog_density".to_string(), Llsd::Real(self.water_fog_density as f64));
        map.insert(
            "water_fog_color".to_string(),
            Llsd::Array(
                self.water_fog_color
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        map.insert(
            "under_water_visibility".to_string(),
            Llsd::Real(self.under_water_visibility as f64),
        );
        map.insert(
            "reflection_wavelet_scale".to_string(),
            Llsd::Array(
                self.reflection_wavelet_scale
                    .iter()
                    .map(|v| Llsd::Real(*v as f64))
                    .collect(),
            ),
        );
        map.insert("fresnel_scale".to_string(), Llsd::Real(self.fresnel_scale as f64));
        map.insert("fresnel_offset".to_string(), Llsd::Real(self.fresnel_offset as f64));
        map.insert("surface_height".to_string(), Llsd::Real(self.surface_height as f64));
        map.insert("large_wave_speed".to_string(), Llsd::Real(self.large_wave_speed as f64));
        map.insert("small_wave_speed".to_string(), Llsd::Real(self.small_wave_speed as f64));
        Llsd::Map(map)
    }

    pub fn from_llsd(input: impl Into<Llsd>) -> Result<Self, EnvironmentError> {
        let mut water = Self::default();
        let value = input.into();
        let map = value
            .as_map()
            .ok_or_else(|| EnvironmentError("expected LLSD map for WaterData".to_string()))?;

        if let Some(name) = map.get("name").and_then(Llsd::as_string) {
            water.name = name.to_string();
        }
        if let Some(values) = map.get("water_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut water.water_color);
        }
        if let Some(value) = map.get("water_fog_density").and_then(Llsd::as_f64) {
            water.water_fog_density = value as f32;
        }
        if let Some(values) = map.get("water_fog_color").and_then(Llsd::as_array) {
            copy_real_array_3(values, &mut water.water_fog_color);
        }
        if let Some(value) = map.get("under_water_visibility").and_then(Llsd::as_f64) {
            water.under_water_visibility = value as f32;
        }
        if let Some(values) = map.get("reflection_wavelet_scale").and_then(Llsd::as_array) {
            copy_real_array_2(values, &mut water.reflection_wavelet_scale);
        }
        if let Some(value) = map.get("fresnel_scale").and_then(Llsd::as_f64) {
            water.fresnel_scale = value as f32;
        }
        if let Some(value) = map.get("fresnel_offset").and_then(Llsd::as_f64) {
            water.fresnel_offset = value as f32;
        }
        if let Some(value) = map.get("surface_height").and_then(Llsd::as_f64) {
            water.surface_height = value as f32;
        }
        if let Some(value) = map.get("large_wave_speed").and_then(Llsd::as_f64) {
            water.large_wave_speed = value as f32;
        }
        if let Some(value) = map.get("small_wave_speed").and_then(Llsd::as_f64) {
            water.small_wave_speed = value as f32;
        }

        Ok(water)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayCycleTrack {
    pub time: f64,
    pub sky: SkyData,
    pub water: WaterData,
}

#[allow(dead_code)]
impl Default for DayCycleTrack {
    fn default() -> Self {
        Self {
            time: DEFAULT_TRACK_TIME,
            sky: SkyData::default(),
            water: WaterData::default(),
        }
    }
}

#[allow(dead_code)]
impl DayCycleTrack {
    pub fn to_llsd(&self) -> Llsd {
        let mut map = BTreeMap::new();
        map.insert("time".to_string(), Llsd::Real(self.time));
        map.insert("sky".to_string(), self.sky.to_llsd());
        map.insert("water".to_string(), self.water.to_llsd());
        Llsd::Map(map)
    }

    pub fn from_llsd(input: impl Into<Llsd>) -> Result<Self, EnvironmentError> {
        let mut track = Self::default();
        let value = input.into();
        let map = value
            .as_map()
            .ok_or_else(|| EnvironmentError("expected LLSD map for DayCycleTrack".to_string()))?;
        if let Some(value) = map.get("time").and_then(Llsd::as_f64) {
            track.time = value;
        }
        if let Some(sky) = map.get("sky") {
            track.sky = SkyData::from_llsd(sky)?;
        }
        if let Some(water) = map.get("water") {
            track.water = WaterData::from_llsd(water)?;
        }
        Ok(track)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayCycle {
    pub tracks: Vec<DayCycleTrack>,
    pub day_length: f64,
}

#[allow(dead_code)]
impl Default for DayCycle {
    fn default() -> Self {
        Self {
            tracks: vec![DayCycleTrack::default()],
            day_length: DEFAULT_DAY_LENGTH,
        }
    }
}

#[allow(dead_code)]
impl DayCycle {
    pub fn compute_day_fraction(&self, time_of_day_seconds: f64) -> f64 {
        compute_day_fraction(time_of_day_seconds, self.day_length)
    }

    pub fn to_llsd(&self) -> Llsd {
        let mut map = BTreeMap::new();
        map.insert(
            "day_length".to_string(),
            Llsd::Real(self.day_length),
        );
        map.insert(
            "tracks".to_string(),
            Llsd::Array(self.tracks.iter().map(DayCycleTrack::to_llsd).collect()),
        );
        Llsd::Map(map)
    }

    pub fn from_llsd(input: impl Into<Llsd>) -> Result<Self, EnvironmentError> {
        let mut day_cycle = Self::default();
        let value = input.into();
        let map = value
            .as_map()
            .ok_or_else(|| EnvironmentError("expected LLSD map for DayCycle".to_string()))?;
        if let Some(value) = map.get("day_length").and_then(Llsd::as_f64) {
            day_cycle.day_length = value;
        }
        if let Some(values) = map.get("tracks").and_then(Llsd::as_array) {
            day_cycle.tracks = values
                .iter()
                .map(DayCycleTrack::from_llsd)
                .collect::<Result<Vec<_>, _>>()?;
        }
        if day_cycle.tracks.is_empty() {
            day_cycle.tracks.push(DayCycleTrack::default());
        }
        Ok(day_cycle)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerEnvironment {
    pub region_id: String,
    pub sky: SkyData,
    pub water: WaterData,
    pub day_cycle: DayCycle,
    pub use_day_cycle: bool,
}

#[allow(dead_code)]
impl Default for ViewerEnvironment {
    fn default() -> Self {
        Self {
            region_id: "00000000-0000-0000-0000-000000000000".to_string(),
            sky: SkyData::default(),
            water: WaterData::default(),
            day_cycle: DayCycle::default(),
            use_day_cycle: true,
        }
    }
}

#[allow(dead_code)]
impl ViewerEnvironment {
    pub fn default_region() -> Self {
        Self {
            region_id: DEFAULT_REGION_NAME.to_string(),
            ..Self::default()
        }
    }

    pub fn to_llsd(&self) -> Llsd {
        let mut map = BTreeMap::new();
        map.insert("region_id".to_string(), Llsd::String(self.region_id.clone()));
        map.insert("sky".to_string(), self.sky.to_llsd());
        map.insert("water".to_string(), self.water.to_llsd());
        map.insert("day_cycle".to_string(), self.day_cycle.to_llsd());
        map.insert("use_day_cycle".to_string(), Llsd::Boolean(self.use_day_cycle));
        Llsd::Map(map)
    }

    pub fn from_llsd(input: impl Into<Llsd>) -> Result<Self, EnvironmentError> {
        let mut environment = Self::default();
        let value = input.into();
        let map = value
            .as_map()
            .ok_or_else(|| EnvironmentError("expected LLSD map for ViewerEnvironment".to_string()))?;
        if let Some(region_id) = map.get("region_id").and_then(Llsd::as_string) {
            environment.region_id = region_id.to_string();
        }
        if let Some(sky) = map.get("sky") {
            environment.sky = SkyData::from_llsd(sky)?;
        }
        if let Some(water) = map.get("water") {
            environment.water = WaterData::from_llsd(water)?;
        }
        if let Some(day_cycle) = map.get("day_cycle") {
            environment.day_cycle = DayCycle::from_llsd(day_cycle)?;
        }
        if let Some(use_day_cycle) = map.get("use_day_cycle").and_then(Llsd::as_bool) {
            environment.use_day_cycle = use_day_cycle;
        }
        Ok(environment)
    }
}

pub fn compute_day_fraction(time_of_day_seconds: f64, day_length_seconds: f64) -> f64 {
    if !time_of_day_seconds.is_finite() || !day_length_seconds.is_finite() || day_length_seconds <= 0.0 {
        return 0.0;
    }
    (time_of_day_seconds / day_length_seconds).rem_euclid(1.0)
}

fn copy_real_array_2(values: &[Llsd], target: &mut [f32; 2]) {
    for (index, value) in values.iter().take(2).enumerate() {
        if let Some(number) = value.as_f64() {
            target[index] = number as f32;
        }
    }
}

fn copy_real_array_3(values: &[Llsd], target: &mut [f32; 3]) {
    for (index, value) in values.iter().take(3).enumerate() {
        if let Some(number) = value.as_f64() {
            target[index] = number as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_well_formed() {
        let environment = ViewerEnvironment::default_region();
        assert_eq!(environment.region_id, DEFAULT_REGION_NAME);
        assert!(!environment.day_cycle.tracks.is_empty());
        assert!(environment.day_cycle.day_length > 0.0);
        assert_eq!(EEP_PHASE, PHASE);
    }

    #[test]
    fn day_cycle_round_trips_through_llsd() {
        let original = DayCycle {
            tracks: vec![
                DayCycleTrack {
                    time: 0.25,
                    sky: SkyData::default(),
                    water: WaterData::default(),
                },
                DayCycleTrack {
                    time: 0.75,
                    sky: SkyData::default(),
                    water: WaterData::default(),
                },
            ],
            day_length: 7200.0,
        };
        let restored = DayCycle::from_llsd(original.to_llsd()).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn compute_day_fraction_wraps() {
        assert!((compute_day_fraction(3600.0, 7200.0) - 0.5).abs() < f64::EPSILON);
        assert!((compute_day_fraction(9000.0, 7200.0) - 0.25).abs() < f64::EPSILON);
        assert!((ViewerEnvironment::default()
            .day_cycle
            .compute_day_fraction(7200.0)
            - 0.5)
            .abs()
            < f64::EPSILON);
    }
}
