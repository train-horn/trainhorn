//! Sound pressure level (SPL) versus distance for a horn in open air.
//!
//! # Model
//!
//! A horn is treated as a point source radiating into a free field. Sound
//! intensity falls with the square of distance (inverse-square law), so the
//! sound pressure level falls by `20 * log10(d2 / d1)` decibels:
//!
//! ```text
//! L(d) = L_ref - 20 * log10(d / d_ref) - alpha * (d - d_ref)
//! ```
//!
//! where `L_ref` is the level measured (or rated) at the reference distance
//! `d_ref` (horn ratings are commonly quoted "at 10 ft" or "at 1 m") and
//! `alpha` is an optional atmospheric absorption coefficient in dB per
//! kilometre, applied to the extra path beyond `d_ref` (default 0).
//!
//! # Assumptions and limits
//!
//! * Free field: no ground reflection, walls, wind, temperature gradients or
//!   barriers. Real outdoor levels can be several dB higher or lower.
//! * The source is small compared with the distances involved. Very close to
//!   the horn (inside roughly one or two horn lengths) the law does not hold.
//! * Absorption depends strongly on frequency, temperature and humidity. The
//!   values in [`AIR_ABSORPTION_DB_PER_KM`] are rounded, typical figures for
//!   10 °C and 70 % relative humidity. Use them as a rough guide only.
//! * Results are estimates, not measurements, and are not a legal noise
//!   assessment.
//!
//! The browser version of this calculator:
//! <https://train-horn.com/tools/decibel-distance/>.

use crate::error::{non_negative, number, positive};
use crate::{InputError, LengthUnit, Result};

/// Approximate atmospheric absorption in dB/km at 10 °C and 70 % RH
/// (an ISO 9613-2 Table 2 condition, from the ISO 9613-1 equations), as
/// `(octave band centre frequency in Hz, dB/km)`.
///
/// Train horn chords sit mostly between about 250 Hz and 600 Hz, where
/// absorption is only 1-2 dB/km, so it matters only at long range.
pub const AIR_ABSORPTION_DB_PER_KM: [(u32, f64); 8] = [
    (63, 0.1),
    (125, 0.4),
    (250, 1.0),
    (500, 1.9),
    (1000, 3.7),
    (2000, 9.7),
    (4000, 32.8),
    (8000, 117.0),
];

/// Look up [`AIR_ABSORPTION_DB_PER_KM`] for an octave band centre frequency.
///
/// ```
/// assert_eq!(trainhorn::acoustics::air_absorption_db_per_km(500), Some(1.9));
/// assert_eq!(trainhorn::acoustics::air_absorption_db_per_km(440), None);
/// ```
pub fn air_absorption_db_per_km(octave_hz: u32) -> Option<f64> {
    AIR_ABSORPTION_DB_PER_KM
        .iter()
        .find(|(hz, _)| *hz == octave_hz)
        .map(|(_, db)| *db)
}

/// Estimate the SPL at `distance` from a level known at `ref_distance`
/// (pure inverse-square law, no air absorption).
///
/// Formula: `L = source_db - 20 * log10(distance / ref_distance)`.
/// Both distances must be in the same unit; which unit does not matter.
///
/// # Errors
///
/// [`InputError`] if a value is not finite or a distance is not > 0.
///
/// # Example
///
/// ```
/// // "150 dB at 10 ft" -> 130 dB at 100 ft (each tenfold distance costs 20 dB)
/// let db = trainhorn::acoustics::spl_at_distance(150.0, 100.0, 10.0)?;
/// assert!((db - 130.0).abs() < 1e-9);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn spl_at_distance(source_db: f64, distance: f64, ref_distance: f64) -> Result<f64> {
    spl_at_distance_with_absorption(source_db, distance, ref_distance, LengthUnit::Feet, 0.0)
}

/// Like [`spl_at_distance`], plus atmospheric absorption over the path
/// beyond `ref_distance`.
///
/// Formula: `L = source_db - 20*log10(distance/ref_distance)
/// - absorption_db_per_km * (distance - ref_distance) / 1000`, with the
/// distances converted from `unit` to metres for the absorption term.
///
/// # Example
///
/// ```
/// use trainhorn::{acoustics, LengthUnit};
/// // 1000 m past a 10 m reference at 2 dB/km costs an extra 2 dB.
/// let plain = acoustics::spl_at_distance(150.0, 1010.0, 10.0)?;
/// let absorbed = acoustics::spl_at_distance_with_absorption(
///     150.0, 1010.0, 10.0, LengthUnit::Meters, 2.0)?;
/// assert!((plain - absorbed - 2.0).abs() < 1e-9);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn spl_at_distance_with_absorption(
    source_db: f64,
    distance: f64,
    ref_distance: f64,
    unit: LengthUnit,
    absorption_db_per_km: f64,
) -> Result<f64> {
    let source_db = number("source_db", source_db)?;
    let distance = positive("distance", distance)?;
    let ref_distance = positive("ref_distance", ref_distance)?;
    let absorption = non_negative("absorption_db_per_km", absorption_db_per_km)?;
    Ok(level(source_db, distance, ref_distance, unit, absorption))
}

fn level(source_db: f64, distance: f64, ref_distance: f64, unit: LengthUnit, absorption: f64) -> f64 {
    let mut l = source_db - 20.0 * (distance / ref_distance).log10();
    if absorption != 0.0 {
        l -= absorption * (unit.to_meters(distance) - unit.to_meters(ref_distance)) / 1000.0;
    }
    l
}

/// Distance at which the level falls to `target_db` (free field); the
/// inverse of [`spl_at_distance`].
///
/// Formula: `d = ref_distance * 10^((source_db - target_db) / 20)`. The
/// result is in the unit of `ref_distance`. `target_db` may be higher than
/// `source_db`; the answer is then closer than `ref_distance`.
///
/// # Errors
///
/// [`InputError`] on non-finite input, `ref_distance <= 0`, or levels so far
/// apart that the distance overflows or underflows.
///
/// # Example
///
/// ```
/// // Where does a 150 dB @ 10 ft horn drop to 90 dB? About 10,000 ft.
/// let ft = trainhorn::acoustics::distance_for_spl(150.0, 90.0, 10.0)?;
/// assert!((ft - 10_000.0).abs() < 1e-6);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn distance_for_spl(source_db: f64, target_db: f64, ref_distance: f64) -> Result<f64> {
    distance_for_spl_with_absorption(source_db, target_db, ref_distance, LengthUnit::Feet, 0.0)
}

/// Like [`distance_for_spl`], with atmospheric absorption.
///
/// With absorption the equation has no elementary closed form, so it is
/// solved by bisection (the level is strictly decreasing with distance).
/// The result is in `unit`.
///
/// # Example
///
/// ```
/// use trainhorn::{acoustics, LengthUnit};
/// let free = acoustics::distance_for_spl(150.0, 90.0, 10.0)?;
/// let absorbed = acoustics::distance_for_spl_with_absorption(
///     150.0, 90.0, 10.0, LengthUnit::Feet, 1.9)?;
/// assert!(absorbed < free);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn distance_for_spl_with_absorption(
    source_db: f64,
    target_db: f64,
    ref_distance: f64,
    unit: LengthUnit,
    absorption_db_per_km: f64,
) -> Result<f64> {
    let source_db = number("source_db", source_db)?;
    let target_db = number("target_db", target_db)?;
    let ref_distance = positive("ref_distance", ref_distance)?;
    let absorption = non_negative("absorption_db_per_km", absorption_db_per_km)?;

    let free_field = ref_distance * 10f64.powf((source_db - target_db) / 20.0);
    if free_field == 0.0 || free_field.is_infinite() {
        return Err(InputError::new(
            "source_db and target_db are too far apart to give a usable distance",
        ));
    }
    if absorption == 0.0 || free_field == ref_distance {
        return Ok(free_field);
    }

    // The root lies between the free-field answer and the reference distance:
    // absorption lowers the level beyond d_ref and raises it inside d_ref.
    let (mut lo, mut hi) = if free_field < ref_distance {
        (free_field, ref_distance)
    } else {
        (ref_distance, free_field)
    };
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if level(source_db, mid, ref_distance, unit, absorption) - target_db > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-12 * hi {
            break;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// Level change in dB when the distance is multiplied by `ratio` (free
/// field): `20 * log10(ratio)`.
///
/// ```
/// let db = trainhorn::acoustics::distance_ratio_loss(10.0)?;
/// assert!((db - 20.0).abs() < 1e-12);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn distance_ratio_loss(ratio: f64) -> Result<f64> {
    Ok(20.0 * positive("ratio", ratio)?.log10())
}

/// Free-field loss for each doubling of distance: `20 * log10(2)`, about
/// 6.02 dB.
///
/// ```
/// assert!((trainhorn::acoustics::db_doubling_loss() - 6.0206).abs() < 1e-4);
/// ```
pub fn db_doubling_loss() -> f64 {
    20.0 * 2f64.log10()
}

/// Combine incoherent sources (e.g. two horns) into one level.
///
/// Formula: `L = 10 * log10(sum(10^(L_i / 10)))`. Two equal sources add
/// about 3.01 dB; they do not double the decibels.
///
/// # Errors
///
/// [`InputError`] if `levels_db` is empty or contains a non-finite value.
///
/// # Example
///
/// ```
/// let db = trainhorn::acoustics::combine_levels(&[100.0, 100.0])?;
/// assert!((db - 103.0103).abs() < 1e-4);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn combine_levels(levels_db: &[f64]) -> Result<f64> {
    if levels_db.is_empty() {
        return Err(InputError::new("combine_levels needs at least one level"));
    }
    let mut peak = f64::NEG_INFINITY;
    for &v in levels_db {
        peak = peak.max(number("level", v)?);
    }
    // Factor out the peak to avoid overflow with very large dB values.
    let sum: f64 = levels_db
        .iter()
        .map(|v| 10f64.powf((v - peak) / 10.0))
        .sum();
    Ok(peak + 10.0 * sum.log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorption_table_is_monotonic() {
        for w in AIR_ABSORPTION_DB_PER_KM.windows(2) {
            assert!(w[0].0 < w[1].0 && w[0].1 < w[1].1);
        }
    }

    #[test]
    fn combine_handles_huge_levels() {
        let db = combine_levels(&[4000.0, 4000.0]).unwrap();
        assert!((db - 4000.0 - 10.0 * 2f64.log10()).abs() < 1e-9);
    }

    #[test]
    fn extreme_level_gap_is_an_error() {
        assert!(distance_for_spl(10000.0, 0.0, 10.0).is_err());
        assert!(distance_for_spl(0.0, 10000.0, 10.0).is_err());
    }
}
