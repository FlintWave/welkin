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
}
