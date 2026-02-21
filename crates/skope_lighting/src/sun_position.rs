// SKOPE Engine — NOAA Solar Position Calculator
// Ported from UE5 FSunPosition::GetSunPosition() + NOAA Solar Calculator
//
// Reference: NOAA Solar Calculator spreadsheet
// https://gml.noaa.gov/grad/solcalc/

use glam::Vec3;

/// Computed sun position data
#[derive(Debug, Clone, Copy)]
pub struct SunPositionData {
    /// Solar elevation angle in degrees (-90 to 90)
    pub elevation: f32,
    /// Atmospheric refraction-corrected elevation
    pub corrected_elevation: f32,
    /// Solar azimuth in degrees (0-360, North=0, clockwise)
    pub azimuth: f32,
    /// Sunrise time (hours, 0-24)
    pub sunrise: f32,
    /// Sunset time (hours, 0-24)
    pub sunset: f32,
    /// Solar noon time (hours, 0-24)
    pub solar_noon: f32,
}

/// Calculate sun position using NOAA Solar Position Algorithm.
///
/// This implements the same algorithm as UE5's `FSunPosition::GetSunPosition()`,
/// which is based on the NOAA Solar Calculator.
///
/// # Arguments
/// * `latitude` — Degrees north (negative for south), range -90 to 90
/// * `longitude` — Degrees east (negative for west), range -180 to 180
/// * `timezone` — UTC offset in hours (e.g. 9.0 for KST, -5.0 for EST)
/// * `year` — Calendar year
/// * `month` — Calendar month (1-12)
/// * `day` — Calendar day (1-31)
/// * `hours` — Hours (0-23)
/// * `minutes` — Minutes (0-59)
/// * `seconds` — Seconds (0-59)
pub fn calculate_sun_position(
    latitude: f64,
    longitude: f64,
    timezone: f64,
    year: i32,
    month: u32,
    day: u32,
    hours: f64,
    minutes: f64,
    seconds: f64,
) -> SunPositionData {
    let lat_rad = latitude.to_radians();

    // Julian Day Number
    let jdn = julian_day_number(year, month, day);
    let time_fraction = (hours + minutes / 60.0 + seconds / 3600.0 - timezone) / 24.0;
    let jd = jdn as f64 + time_fraction;

    // Julian Century
    let jc = (jd - 2451545.0) / 36525.0;

    // Geometric Mean Longitude of the Sun (degrees) — normalize to [0, 360)
    let geom_mean_long_raw = 280.46646 + jc * (36000.76983 + 0.0003032 * jc);
    let geom_mean_long = ((geom_mean_long_raw % 360.0) + 360.0) % 360.0;

    // Geometric Mean Anomaly of the Sun (degrees)
    let geom_mean_anom = 357.52911 + jc * (35999.05029 - 0.0001537 * jc);
    let geom_mean_anom_rad = geom_mean_anom.to_radians();

    // Eccentricity of Earth's Orbit
    let ecc = 0.016708634 - jc * (0.000042037 + 0.0000001267 * jc);

    // Sun's Equation of the Center
    let sun_eq_center = (1.914602 - jc * (0.004817 + 0.000014 * jc)) * geom_mean_anom_rad.sin()
        + (0.019993 - 0.000101 * jc) * (2.0 * geom_mean_anom_rad).sin()
        + 0.000289 * (3.0 * geom_mean_anom_rad).sin();

    // Sun's True Longitude
    let sun_true_long = geom_mean_long + sun_eq_center;

    // Sun's True Anomaly
    let sun_true_anom = geom_mean_anom + sun_eq_center;
    let sun_true_anom_rad = sun_true_anom.to_radians();

    // Sun's Radius Vector (AU)
    let _sun_rad_vector = (1.000001018 * (1.0 - ecc * ecc))
        / (1.0 + ecc * sun_true_anom_rad.cos());

    // Sun's Apparent Longitude
    let omega = 125.04 - 1934.136 * jc;
    let omega_rad = omega.to_radians();
    let sun_apparent_long = sun_true_long - 0.00569 - 0.00478 * omega_rad.sin();
    let sun_apparent_long_rad = sun_apparent_long.to_radians();

    // Mean Obliquity of the Ecliptic
    let mean_obliq = 23.0 + (26.0 + (21.448 - jc * (46.815 + jc * (0.00059 - jc * 0.001813))) / 60.0) / 60.0;

    // Corrected Obliquity
    let obliq_corr = mean_obliq + 0.00256 * omega_rad.cos();
    let obliq_corr_rad = obliq_corr.to_radians();

    // Sun's Declination
    let sin_decl = obliq_corr_rad.sin() * sun_apparent_long_rad.sin();
    let declination = sin_decl.asin();

    // Equation of Time (minutes)
    let y = (obliq_corr_rad / 2.0).tan().powi(2);
    let geom_mean_long_rad = geom_mean_long.to_radians();
    let eq_of_time = 4.0 * (
        y * (2.0 * geom_mean_long_rad).sin()
        - 2.0 * ecc * geom_mean_anom_rad.sin()
        + 4.0 * ecc * y * geom_mean_anom_rad.sin() * (2.0 * geom_mean_long_rad).cos()
        - 0.5 * y * y * (4.0 * geom_mean_long_rad).sin()
        - 1.25 * ecc * ecc * (2.0 * geom_mean_anom_rad).sin()
    ).to_degrees();

    // Solar Noon (LST, in minutes from midnight)
    let solar_noon_min = 720.0 - 4.0 * longitude - eq_of_time + timezone * 60.0;

    // Hour Angle for sunrise/sunset
    let cos_ha = (-0.8333f64).to_radians().cos() / (lat_rad.cos() * declination.cos())
        - lat_rad.tan() * declination.tan();

    let (sunrise_time, sunset_time) = if cos_ha.abs() <= 1.0 {
        let ha_sunrise = cos_ha.acos().to_degrees();
        let sunrise = solar_noon_min - ha_sunrise * 4.0;
        let sunset = solar_noon_min + ha_sunrise * 4.0;
        (sunrise / 60.0, sunset / 60.0)
    } else if latitude * declination.to_degrees() > 0.0 {
        // Midnight sun
        (0.0, 24.0)
    } else {
        // Polar night
        (0.0, 0.0)
    };

    // True Solar Time (minutes)
    let time_in_minutes = hours * 60.0 + minutes + seconds / 60.0;
    let true_solar_time_raw = time_in_minutes + eq_of_time + 4.0 * longitude - 60.0 * timezone;
    // Normalize to [0, 1440) — handle negative values from Rust's sign-preserving %
    let true_solar_time = ((true_solar_time_raw % 1440.0) + 1440.0) % 1440.0;

    // Hour Angle (degrees): NOAA formula = TST / 4 - 180
    let hour_angle = true_solar_time / 4.0 - 180.0;
    let hour_angle_rad = hour_angle.to_radians();

    // Solar Zenith Angle
    let cos_zenith = lat_rad.sin() * declination.sin()
        + lat_rad.cos() * declination.cos() * hour_angle_rad.cos();
    let zenith = cos_zenith.acos();
    let zenith_deg = zenith.to_degrees();

    // Solar Elevation
    let elevation = 90.0 - zenith_deg;

    // Atmospheric Refraction correction (arc-seconds → degrees)
    let refraction = if elevation > 85.0 {
        0.0
    } else if elevation > 5.0 {
        let e_rad = elevation.to_radians();
        let tan_e = e_rad.tan();
        (58.1 / tan_e - 0.07 / tan_e.powi(3) + 0.000086 / tan_e.powi(5)) / 3600.0
    } else if elevation > -0.575 {
        (1735.0 + elevation * (-518.2 + elevation * (103.4 + elevation * (-12.79 + elevation * 0.711)))) / 3600.0
    } else {
        (-20.772 / elevation.to_radians().tan()) / 3600.0
    };

    let corrected_elevation = elevation + refraction;

    // Solar Azimuth (guard against division by zero when sun is exactly at zenith)
    let sin_zenith = zenith.sin();
    let azimuth = if sin_zenith.abs() < 1e-10 {
        // Sun exactly at zenith — azimuth is undefined, default to south
        180.0
    } else if hour_angle > 0.0 {
        let cos_az = (lat_rad.sin() * cos_zenith - declination.sin())
            / (lat_rad.cos() * sin_zenith);
        (cos_az.clamp(-1.0, 1.0).acos().to_degrees() + 180.0) % 360.0
    } else {
        let cos_az = (lat_rad.sin() * cos_zenith - declination.sin())
            / (lat_rad.cos() * sin_zenith);
        (540.0 - cos_az.clamp(-1.0, 1.0).acos().to_degrees()) % 360.0
    };

    SunPositionData {
        elevation: elevation as f32,
        corrected_elevation: corrected_elevation as f32,
        azimuth: azimuth as f32,
        sunrise: sunrise_time as f32,
        sunset: sunset_time as f32,
        solar_noon: (solar_noon_min / 60.0) as f32,
    }
}

/// Convert sun elevation and azimuth angles to a world-space direction vector (Y-up).
///
/// Returns the direction the light is shining (i.e., from sun toward ground),
/// suitable for use as a directional light direction.
///
/// # Arguments
/// * `elevation_deg` — Solar elevation in degrees (positive = above horizon)
/// * `azimuth_deg` — Solar azimuth in degrees (0=North, 90=East, clockwise)
pub fn sun_angles_to_direction(elevation_deg: f32, azimuth_deg: f32) -> Vec3 {
    let elev = elevation_deg.to_radians();
    let azim = azimuth_deg.to_radians();

    // Direction FROM the sun (toward ground) — negate for light direction
    // In Y-up coordinate system:
    //   North = -Z, East = +X
    //   azimuth 0 = North = -Z
    //   elevation positive = above horizon = +Y component
    let cos_elev = elev.cos();
    // Sun position in sky (direction TO the sun)
    let to_sun = Vec3::new(
        azim.sin() * cos_elev,   // East component
        elev.sin(),               // Up component
        -azim.cos() * cos_elev,  // North component (negative Z)
    );

    // Light direction = from sun toward scene = negative of to_sun
    (-to_sun).normalize()
}

/// Decompose fractional hours into (hours, minutes, seconds)
pub fn decompose_time(time_of_day: f32) -> (f64, f64, f64) {
    let total_seconds = (time_of_day * 3600.0) as f64;
    let hours = (total_seconds / 3600.0).floor();
    let remaining = total_seconds - hours * 3600.0;
    let minutes = (remaining / 60.0).floor();
    let seconds = remaining - minutes * 60.0;
    (hours, minutes, seconds)
}

/// Convert day-of-year (1-365) to (month, day) for a given year.
pub fn day_of_year_to_month_day(doy: u32, year: i32) -> (u32, u32) {
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_months: [u32; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut remaining = doy.min(if is_leap { 366 } else { 365 });
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining <= days {
            return ((i + 1) as u32, remaining);
        }
        remaining -= days;
    }
    (12, 31) // fallback
}

/// Compute Julian Day Number from calendar date
fn julian_day_number(year: i32, month: u32, day: u32) -> i64 {
    // Using the standard astronomical Julian Day Number formula
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;

    let a = (14 - m) / 12;
    let y2 = y + 4800 - a;
    let m2 = m + 12 * a - 3;

    // Gregorian calendar
    d + (153 * m2 + 2) / 5 + 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 - 32045
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_julian_day_number() {
        // J2000.0 epoch: January 1.5, 2000 = JD 2451545.0
        assert_eq!(julian_day_number(2000, 1, 1), 2451545);
    }

    #[test]
    fn test_decompose_time() {
        let (h, m, s) = decompose_time(12.5);
        assert_eq!(h, 12.0);
        assert_eq!(m, 30.0);
        assert!((s - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_day_of_year_to_month_day() {
        assert_eq!(day_of_year_to_month_day(1, 2024), (1, 1));   // Jan 1
        assert_eq!(day_of_year_to_month_day(32, 2024), (2, 1));  // Feb 1
        assert_eq!(day_of_year_to_month_day(60, 2024), (2, 29)); // Feb 29 (leap)
        assert_eq!(day_of_year_to_month_day(60, 2023), (3, 1));  // Mar 1 (non-leap)
        assert_eq!(day_of_year_to_month_day(172, 2024), (6, 20)); // ~June 20
    }

    #[test]
    fn test_sun_position_seoul_noon() {
        // Seoul, June 21, 12:00 KST — sun should be high in the sky
        let pos = calculate_sun_position(
            37.5665, 126.978, 9.0,
            2024, 6, 21,
            12.0, 0.0, 0.0,
        );
        // Solar noon elevation in Seoul at summer solstice ~ 76 degrees
        assert!(pos.corrected_elevation > 60.0, "Elevation: {}", pos.corrected_elevation);
        assert!(pos.corrected_elevation < 85.0, "Elevation: {}", pos.corrected_elevation);
        // Azimuth should be roughly south (~180)
        assert!(pos.azimuth > 150.0 && pos.azimuth < 210.0, "Azimuth: {}", pos.azimuth);
    }

    #[test]
    fn test_sun_angles_to_direction() {
        // Sun directly overhead (90° elevation) → direction should be straight down
        let dir = sun_angles_to_direction(90.0, 0.0);
        assert!((dir.y - (-1.0)).abs() < 0.01, "Dir: {:?}", dir);

        // Sun at horizon (0° elevation, due south 180°)
        // Sun is at +Z (south), light direction is from sun = -Z (toward north)
        let dir = sun_angles_to_direction(0.0, 180.0);
        assert!(dir.z < -0.9, "Dir: {:?}", dir);
        assert!(dir.y.abs() < 0.01, "Dir: {:?}", dir);

        // Sun at 45° elevation, due east (90°)
        // Sun position: (cos45*sin90, sin45, -cos45*cos90) = (0.707, 0.707, 0)
        // Light dir = negated = (-0.707, -0.707, 0)
        let dir = sun_angles_to_direction(45.0, 90.0);
        assert!(dir.x < -0.5, "Dir: {:?}", dir);
        assert!(dir.y < -0.5, "Dir: {:?}", dir);
    }
}
