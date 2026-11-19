//! Onboard air system: tank runtime and compressor recovery.
//!
//! # Free-air volume (Boyle's law)
//!
//! A tank of volume `V` holding air at absolute pressure `P` contains the
//! same mass of air as `V * P / P_atm` of air at atmospheric pressure ("free
//! air"). Gauge pressures (PSI on a dial, "psig") are converted to absolute
//! by adding atmospheric pressure `P_atm` (14.696 psi at sea level). The free
//! air released while the tank drops from `P_start` to `P_end` (both gauge)
//! is:
//!
//! ```text
//! V_free = V_tank * (P_start - P_end) / P_atm
//! ```
//!
//! Horn consumption and compressor output are rated in CFM of free air
//! (cubic feet per minute at atmospheric pressure), so
//! `horn_seconds = V_free / horn_cfm * 60`.
//!
//! # Assumptions
//!
//! * Isothermal expansion (Boyle's law). Fast blasts cool the air and give a
//!   little less than this; treat results as an upper estimate.
//! * The horn consumes its rated CFM at every tank pressure above the
//!   cut-off, and the compressor is off while the horn sounds (worst case).
//! * Line, valve and leak losses are ignored.
//! * 1 US gallon = 231 cubic inches = 0.133680556 cubic feet.
//!
//! Pass [`ATMOSPHERIC_PSI`] for sea level, or a lower ambient pressure for
//! altitude (about 12.2 psi at 5,000 ft).

use crate::error::{non_negative, positive};
use crate::{InputError, Result};

/// Standard atmosphere at sea level in psi.
pub const ATMOSPHERIC_PSI: f64 = 14.696;

/// Cubic feet in one US liquid gallon (231 in³ / 1728 in³).
pub const CUBIC_FEET_PER_GALLON: f64 = 231.0 / 1728.0;

fn pressures(high_name: &str, high: f64, low_name: &str, low: f64) -> Result<(f64, f64)> {
    let high = non_negative(high_name, high)?;
    let low = non_negative(low_name, low)?;
    if high <= low {
        return Err(InputError::new(format!(
            "{high_name} ({high} psi) must be greater than {low_name} ({low} psi)"
        )));
    }
    Ok((high, low))
}

fn free_air(
    tank_gallons: f64,
    high_name: &str,
    high: f64,
    low_name: &str,
    low: f64,
    atmospheric_psi: f64,
) -> Result<f64> {
    let tank_gallons = positive("tank_gallons", tank_gallons)?;
    let (high, low) = pressures(high_name, high, low_name, low)?;
    let atm = positive("atmospheric_psi", atmospheric_psi)?;
    Ok(tank_gallons * CUBIC_FEET_PER_GALLON * (high - low) / atm)
}

/// Free air (cubic feet at atmospheric pressure) released between two gauge
/// pressures.
///
/// Formula: `V_free = tank_gallons * 0.13368 * (start_psi - end_psi) / atmospheric_psi`.
///
/// # Errors
///
/// [`InputError`] if `tank_gallons <= 0`, a pressure is negative,
/// `start_psi <= end_psi`, or `atmospheric_psi <= 0`.
///
/// # Example
///
/// ```
/// use trainhorn::airsystem::{free_air_volume_cuft, ATMOSPHERIC_PSI, CUBIC_FEET_PER_GALLON};
/// // Emptying 1 gal from 1 atm gauge to 0 psig releases exactly 1 gal of free air.
/// let cuft = free_air_volume_cuft(1.0, ATMOSPHERIC_PSI, 0.0, ATMOSPHERIC_PSI)?;
/// assert!((cuft - CUBIC_FEET_PER_GALLON).abs() < 1e-12);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn free_air_volume_cuft(
    tank_gallons: f64,
    start_psi: f64,
    end_psi: f64,
    atmospheric_psi: f64,
) -> Result<f64> {
    free_air(tank_gallons, "start_psi", start_psi, "end_psi", end_psi, atmospheric_psi)
}

/// Seconds of continuous horn time from `start_psi` down to `cutoff_psi`.
///
/// Formula: `free_air_volume_cuft(..) / horn_cfm * 60`.
///
/// * `tank_gallons`: tank volume in US gallons (> 0).
/// * `start_psi`: tank gauge pressure when you start blowing (e.g. 150).
/// * `cutoff_psi`: lowest gauge pressure at which the horn still sounds
///   acceptably (e.g. 90); must be below `start_psi`.
/// * `horn_cfm`: horn air consumption in CFM of free air (> 0).
/// * `atmospheric_psi`: ambient absolute pressure, usually [`ATMOSPHERIC_PSI`].
///
/// # Example
///
/// ```
/// use trainhorn::airsystem::{tank_runtime_seconds, ATMOSPHERIC_PSI};
/// let s = tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, ATMOSPHERIC_PSI)?;
/// assert_eq!(format!("{s:.2}"), "10.92");
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn tank_runtime_seconds(
    tank_gallons: f64,
    start_psi: f64,
    cutoff_psi: f64,
    horn_cfm: f64,
    atmospheric_psi: f64,
) -> Result<f64> {
    let horn_cfm = positive("horn_cfm", horn_cfm)?;
    let air = free_air(tank_gallons, "start_psi", start_psi, "cutoff_psi", cutoff_psi, atmospheric_psi)?;
    Ok(air / horn_cfm * 60.0)
}

/// Whole blasts of `blast_seconds` that fit in `runtime_seconds`.
///
/// ```
/// assert_eq!(trainhorn::airsystem::blast_count(10.9, 2.0)?, 5);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn blast_count(runtime_seconds: f64, blast_seconds: f64) -> Result<u64> {
    let runtime = non_negative("runtime_seconds", runtime_seconds)?;
    let blast = positive("blast_seconds", blast_seconds)?;
    // Small epsilon so 0.9 / 0.3 style exact divisions are not lost to rounding.
    let n = (runtime / blast + 1e-9).floor();
    if n >= u64::MAX as f64 {
        return Err(InputError::new("blast count is too large"));
    }
    Ok(n as u64)
}

/// A compressor delivery curve that falls linearly with pressure, for
/// [`compressor_recovery_seconds`].
///
/// Delivery is `cfm_at_zero_psi` at 0 psi and the rated CFM at `rated_psi`,
/// linear in between.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearFlow {
    /// Free-air delivery at 0 psi, in CFM (>= the rated CFM).
    pub cfm_at_zero_psi: f64,
    /// Gauge pressure at which the compressor's rated CFM applies (> 0).
    pub rated_psi: f64,
}

/// Seconds for a compressor to refill the tank from `cut_in_psi` to
/// `cut_out_psi`.
///
/// **Constant-flow model** (`curve = None`): the compressor delivers
/// `compressor_cfm` of free air over the whole range, so
/// `t = V_free / compressor_cfm * 60`. Use the CFM the manufacturer rates
/// *at* (or near) your cut-out pressure; output falls as pressure rises, so a
/// 0 psi rating gives an optimistic time.
///
/// **Linear-flow model** (`curve = Some(LinearFlow { .. })`): delivery falls
/// linearly from `cfm_at_zero_psi` at 0 psi to `compressor_cfm` at
/// `rated_psi`, `q(p) = a + b*p`. Integrating `dt = (V_tank / P_atm) dp / q(p)`
/// gives:
///
/// ```text
/// t = V_tank / P_atm * ln(q(p_out) / q(p_in)) / b * 60     (b != 0)
/// ```
///
/// # Errors
///
/// [`InputError`] on invalid input, if `cfm_at_zero_psi < compressor_cfm`,
/// or if the linear curve reaches zero output below `cut_out_psi`.
///
/// # Example
///
/// ```
/// use trainhorn::airsystem::{compressor_recovery_seconds, LinearFlow, ATMOSPHERIC_PSI};
/// // 2 gal tank, 110 -> 150 psi, 1.0 CFM compressor: about 43.7 s.
/// let t = compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, None, ATMOSPHERIC_PSI)?;
/// assert_eq!(format!("{t:.1}"), "43.7");
///
/// // Same compressor rated 1.0 CFM @ 150 psi but 2.0 CFM @ 0 psi refills faster.
/// let curve = LinearFlow { cfm_at_zero_psi: 2.0, rated_psi: 150.0 };
/// let faster = compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, Some(curve), ATMOSPHERIC_PSI)?;
/// assert!(faster < t);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn compressor_recovery_seconds(
    tank_gallons: f64,
    cut_in_psi: f64,
    cut_out_psi: f64,
    compressor_cfm: f64,
    curve: Option<LinearFlow>,
    atmospheric_psi: f64,
) -> Result<f64> {
    let compressor_cfm = positive("compressor_cfm", compressor_cfm)?;
    let air = free_air(tank_gallons, "cut_out_psi", cut_out_psi, "cut_in_psi", cut_in_psi, atmospheric_psi)?;

    let Some(curve) = curve else {
        return Ok(air / compressor_cfm * 60.0);
    };
    let q0 = positive("cfm_at_zero_psi", curve.cfm_at_zero_psi)?;
    let p_rated = positive("rated_psi", curve.rated_psi)?;
    if q0 < compressor_cfm {
        return Err(InputError::new(
            "cfm_at_zero_psi must be at least compressor_cfm (output falls with pressure)",
        ));
    }
    let slope = (compressor_cfm - q0) / p_rated; // cfm per psi, <= 0
    let q_in = q0 + slope * cut_in_psi;
    let q_out = q0 + slope * cut_out_psi;
    if q_out <= 0.0 {
        return Err(InputError::new(format!(
            "the linear flow model gives no output at {cut_out_psi} psi; \
             check cfm_at_zero_psi, compressor_cfm and rated_psi"
        )));
    }
    if slope == 0.0 {
        return Ok(air / q0 * 60.0);
    }
    let tank_cuft = tank_gallons * CUBIC_FEET_PER_GALLON;
    Ok(tank_cuft / atmospheric_psi * (q_out / q_in).ln() / slope * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_names_both_pressures() {
        let e = tank_runtime_seconds(5.0, 100.0, 150.0, 10.0, ATMOSPHERIC_PSI).unwrap_err();
        assert_eq!(
            e.message(),
            "start_psi (100 psi) must be greater than cutoff_psi (150 psi)"
        );
    }

    #[test]
    fn blast_count_rejects_nan() {
        assert!(blast_count(f64::NAN, 1.0).is_err());
        assert!(blast_count(10.0, f64::INFINITY).is_err());
    }
}
