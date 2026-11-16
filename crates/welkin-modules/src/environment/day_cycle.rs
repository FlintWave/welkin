// SPDX-License-Identifier: LGPL-3.0-or-later

//! Pure helpers for converting sun hours and day fractions to directions.

use std::f64::consts::PI;

/// Day fraction for a fixed sun hour, matching OpenSim's modulus convention.
///
/// `sun_hour` is in hours (0..24) and `day_length` is the virtual day in
/// seconds.  A zero or negative day length yields `0.0`.
pub fn day_fraction(sun_hour: f64, day_length: f64) -> f64 {
    if day_length <= 0.0 {
        return 0.0;
    }
    let seconds = sun_hour * 3600.0;
    (seconds.rem_euclid(day_length)) / day_length
}

/// Sun direction unit vector from a day fraction (0..1).
pub fn sun_direction(day_fraction: f64) -> [f64; 3] {
    let angle = 2.0 * PI * day_fraction;
    [angle.cos(), 0.0, angle.sin()]
}

/// Moon direction unit vector from a day fraction (0..1).
pub fn moon_direction(day_fraction: f64) -> [f64; 3] {
    let angle = 2.0 * PI * day_fraction + PI;
    [angle.cos(), 0.0, angle.sin()]
}

// Aliases matching alternate naming used by callers/OpenSpec.
pub fn day_fraction_from_sun_hour(sun_hour: f64, day_length: f64) -> f64 {
    day_fraction(sun_hour, day_length)
}

pub fn compute_sun_direction(day_fraction: f64) -> [f64; 3] {
    sun_direction(day_fraction)
}

pub fn compute_moon_direction(day_fraction: f64) -> [f64; 3] {
    moon_direction(day_fraction)
}

/// Day-cycle state helper tracking wall-clock progression of a region's day.
#[derive(Debug, Clone)]
pub struct DayCycleState {
    /// Virtual day length in seconds.
    pub day_length_seconds: u32,
    /// Offset applied to wall time in seconds.
    pub day_offset_seconds: i32,
    /// Current wall-clock unix time tracked by the state.
    pub current_unix_time: i64,
}

impl Default for DayCycleState {
    fn default() -> Self {
        Self {
            day_length_seconds: 86400,
            day_offset_seconds: 0,
            current_unix_time: 0,
        }
    }
}

impl DayCycleState {
    pub fn new(day_length_seconds: u32, day_offset_seconds: i32) -> Self {
        Self {
            day_length_seconds,
            day_offset_seconds,
            current_unix_time: 0,
        }
    }

    pub fn new_at(
        day_length_seconds: u32,
        day_offset_seconds: i32,
        unix_time: i64,
    ) -> Self {
        Self {
            day_length_seconds,
            day_offset_seconds,
            current_unix_time: unix_time,
        }
    }

    /// Advance the tracked wall-clock time to `now_unix_time`.
    pub fn advance(&mut self, now_unix_time: i64) {
        self.current_unix_time = now_unix_time;
    }

    /// Advance the tracked wall-clock time by `seconds`.
    pub fn advance_by(&mut self, seconds: f64) {
        self.current_unix_time += seconds as i64;
    }

    pub fn advance_by_seconds(&mut self, seconds: f64) {
        self.advance_by(seconds);
    }

    pub fn advance_elapsed(&mut self, seconds: f64) {
        self.advance_by(seconds);
    }

    pub fn tick(&mut self, now_unix_time: i64) {
        self.advance(now_unix_time);
    }

    pub fn current_day_fraction(&self) -> f64 {
        compute_day_fraction(
            self.current_unix_time,
            self.day_length_seconds,
            self.day_offset_seconds,
        )
    }

    pub fn sun_direction(&self) -> [f64; 3] {
        sun_direction(self.current_day_fraction())
    }

    pub fn moon_direction(&self) -> [f64; 3] {
        moon_direction(self.current_day_fraction())
    }

    pub fn viewer_time_llsd(&self) -> String {
        send_viewer_time(self.current_day_fraction())
    }
}

/// Compute the day fraction from a unix timestamp.
pub fn compute_day_fraction(
    unix_time: i64,
    day_length_seconds: u32,
    day_offset_seconds: i32,
) -> f64 {
    if day_length_seconds == 0 {
        return 0.0;
    }
    let day = day_length_seconds as i64;
    let num = unix_time as i64 + day_offset_seconds as i64;
    let mut frac = (num as f64 / day as f64).rem_euclid(1.0);
    if frac == 0.0 && num != 0 {
        frac = 1.0;
    }
    frac
}

/// LLSD message for "send viewer time".
pub fn send_viewer_time(day_fraction: f64) -> String {
    let sun = sun_direction(day_fraction);
    let moon = moon_direction(day_fraction);
    format!(
        "<llsd><map>\
         <key>day_fraction</key><real>{}</real>\
         <key>sun_direction</key><array><real>{}</real><real>{}</real><real>{}</real></array>\
         <key>moon_direction</key><array><real>{}</real><real>{}</real><real>{}</real></array>\
         </map></llsd>",
        day_fraction, sun[0], sun[1], sun[2], moon[0], moon[1], moon[2]
    )
}

/// Aliases for send viewer time LLSD builder.
pub fn build_viewer_time_llsd(day_fraction: f64) -> String {
    send_viewer_time(day_fraction)
}
pub fn encode_viewer_time(day_fraction: f64) -> String {
    send_viewer_time(day_fraction)
}
pub fn viewer_time_llsd(day_fraction: f64) -> String {
    send_viewer_time(day_fraction)
}
pub fn send_viewer_time_llsd(day_fraction: f64) -> String {
    send_viewer_time(day_fraction)
}

/// State method-style free function aliases.
pub fn send_viewer_time_from_state(state: &DayCycleState) -> String {
    state.viewer_time_llsd()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_fraction_constants() {
        assert!((day_fraction(0.0, 14400.0) - 0.0).abs() < 1e-12);
        assert!((day_fraction(12.0, 86400.0) - 0.5).abs() < 1e-12);
        assert!((day_fraction(24.0, 86400.0) - 0.0).abs() < 1e-12);
        assert_eq!(day_fraction(1.0, 0.0), 0.0);
    }

    #[test]
    fn directions_unit() {
        for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let s = sun_direction(f);
            let m = moon_direction(f);
            let smag = (s[0]*s[0] + s[1]*s[1] + s[2]*s[2]).sqrt();
            let mmag = (m[0]*m[0] + m[1]*m[1] + m[2]*m[2]).sqrt();
            assert!((smag - 1.0).abs() < 1e-12);
            assert!((mmag - 1.0).abs() < 1e-12);
            assert!(s != m);
        }
    }

    #[test]
    fn day_cycle_state_advance_and_fraction() {
        let mut state = DayCycleState::new(100, 0);
        assert_eq!(state.current_day_fraction(), 0.0);
        state.advance(50);
        assert!((state.current_day_fraction() - 0.5).abs() < 1e-9);
        state.advance_by(25.0);
        assert!((state.current_day_fraction() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn compute_day_fraction_basic() {
        assert!((compute_day_fraction(0, 100, 0) - 0.0).abs() < 1e-9);
        assert!((compute_day_fraction(100, 100, 0) - 1.0).abs() < 1e-9);
        assert_eq!(compute_day_fraction(1, 0, 0), 0.0);
    }

    #[test]
    fn send_viewer_time_contains() {
        let xml = send_viewer_time(0.25);
        assert!(xml.contains("day_fraction"));
        assert!(xml.contains("sun_direction"));
        assert!(xml.contains("moon_direction"));
    }
}
