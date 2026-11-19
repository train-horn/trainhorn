//! Copper wire gauge (AWG) selection for 12 V / 24 V horn and compressor
//! circuits.
//!
//! # Resistance per AWG size
//!
//! Resistances are *computed* from the AWG definition rather than typed in
//! from a table:
//!
//! * AWG diameter (ASTM B258): `d(n) = 0.005 in * 92^((36 - n) / 39)`, where
//!   1/0, 2/0, 3/0, 4/0 are n = 0, -1, -2, -3.
//! * Annealed copper resistivity at 20 °C (100 % IACS):
//!   `rho = 1.7241e-8 ohm*m`.
//! * `R = rho * L / A` with `A = pi * d^2 / 4`.
//! * Optional temperature correction: `R_T = R_20 * (1 + 0.00393 * (T - 20))`.
//!
//! The computed values agree with published solid-copper AWG tables (for
//! example NEC Chapter 9 Table 8) to within about 1 %; e.g. 10 AWG is 0.999
//! ohm per 1000 ft and 4 AWG is 0.249 ohm per 1000 ft. Stranded wire of the
//! same gauge is a few percent higher, which the default 3 % budget absorbs.
//!
//! # Voltage drop
//!
//! Current flows out and back, so the conductor length is twice the one-way
//! run (a ground return through the chassis is usually *worse* than a wire):
//!
//! ```text
//! V_drop = I * R_per_length * 2 * one_way_length
//! drop_% = V_drop / V_system * 100
//! ```
//!
//! # Scope
//!
//! This suggests the *smallest* gauge that meets a voltage-drop budget. It
//! does not check ampacity (insulation temperature rating, bundling,
//! engine-bay heat) or fuse sizing. Always size the fuse to protect the wire
//! you choose.

use std::fmt;
use std::str::FromStr;

use crate::error::{number, positive};
use crate::{InputError, LengthUnit, Result};

/// Annealed copper resistivity, 100 % IACS, at 20 °C, in ohm·m.
pub const COPPER_RESISTIVITY_OHM_M: f64 = 1.7241e-8;

/// Temperature coefficient of resistance of annealed copper near 20 °C.
pub const COPPER_TEMP_COEFF_PER_C: f64 = 0.00393;

/// An American Wire Gauge size, from 40 AWG (thinnest) to 4/0 (thickest).
///
/// Parse it from text (`"10"`, `"10 AWG"`, `"1/0"`, `"0000"`) or build it
/// from a number with [`Awg::gauge`]. It displays as the usual label.
///
/// ```
/// use trainhorn::electrical::Awg;
/// let a: Awg = "2/0".parse()?;
/// assert_eq!(a, "00".parse()?);
/// assert_eq!(a.to_string(), "2/0");
/// assert_eq!(Awg::gauge(10)?.to_string(), "10");
/// assert!("5/0".parse::<Awg>().is_err());
/// # Ok::<(), trainhorn::InputError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Awg(i8);

impl Awg {
    /// Gauge `n` for 1..=40 AWG. Use [`Awg::aught`] for 1/0..4/0.
    pub fn gauge(n: u8) -> Result<Awg> {
        if (1..=40).contains(&n) {
            Ok(Awg(n as i8))
        } else {
            Err(InputError::new(format!(
                "AWG size must be between 4/0 and 40, got {n}"
            )))
        }
    }

    /// Aught sizes: `aught(1)` is 1/0, ..., `aught(4)` is 4/0.
    pub fn aught(zeros: u8) -> Result<Awg> {
        if (1..=4).contains(&zeros) {
            Ok(Awg(1 - zeros as i8))
        } else {
            Err(InputError::new(format!(
                "aught sizes are 1/0 to 4/0, got {zeros}/0"
            )))
        }
    }

    /// The ASTM B258 formula index: 10 for 10 AWG, 0 for 1/0, -3 for 4/0.
    pub fn index(self) -> i8 {
        self.0
    }

    /// Bare conductor diameter in millimetres (solid wire).
    ///
    /// ```
    /// use trainhorn::electrical::Awg;
    /// assert!((Awg::aught(4)?.diameter_mm() - 11.684).abs() < 0.01);
    /// # Ok::<(), trainhorn::InputError>(())
    /// ```
    pub fn diameter_mm(self) -> f64 {
        0.127 * 92f64.powf((36.0 - f64::from(self.0)) / 39.0) // 0.005 in = 0.127 mm
    }

    /// Conductor cross-section area in mm².
    pub fn area_mm2(self) -> f64 {
        let d = self.diameter_mm();
        std::f64::consts::PI * d * d / 4.0
    }

    /// Copper resistance in ohms per 1000 ft at `temperature_c`.
    ///
    /// # Errors
    ///
    /// [`InputError`] unless -60 <= `temperature_c` <= 200.
    ///
    /// ```
    /// use trainhorn::electrical::Awg;
    /// let r = Awg::gauge(10)?.resistance_ohm_per_1000ft(20.0)?;
    /// assert!((r - 0.9989).abs() < 0.01);
    /// # Ok::<(), trainhorn::InputError>(())
    /// ```
    pub fn resistance_ohm_per_1000ft(self, temperature_c: f64) -> Result<f64> {
        let t = number("temperature_c", temperature_c)?;
        if !(-60.0..=200.0).contains(&t) {
            return Err(InputError::new("temperature_c must be between -60 and 200 degC"));
        }
        let area_m2 = self.area_mm2() * 1e-6;
        let r20 = COPPER_RESISTIVITY_OHM_M * 304.8 / area_m2; // 1000 ft = 304.8 m
        Ok(r20 * (1.0 + COPPER_TEMP_COEFF_PER_C * (t - 20.0)))
    }
}

impl fmt::Display for Awg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 <= 0 {
            write!(f, "{}/0", 1 - self.0)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl FromStr for Awg {
    type Err = InputError;

    fn from_str(s: &str) -> Result<Self> {
        let upper = s.trim().to_ascii_uppercase().replace("AWG", "");
        let text = upper.trim();
        let aught = match text {
            "1/0" | "0" => Some(1),
            "2/0" | "00" => Some(2),
            "3/0" | "000" => Some(3),
            "4/0" | "0000" => Some(4),
            _ => None,
        };
        if let Some(z) = aught {
            return Awg::aught(z);
        }
        match text.parse::<i64>() {
            Ok(n) if (1..=40).contains(&n) => Ok(Awg(n as i8)),
            Ok(_) => Err(InputError::new(format!(
                "AWG size must be between 4/0 and 40, got {s:?}"
            ))),
            Err(_) => Err(InputError::new(format!(
                "unknown AWG size {s:?}; use 1-40 or 1/0..4/0"
            ))),
        }
    }
}

/// Gauges considered by [`suggest_awg`], thinnest to thickest: the sizes
/// commonly sold as automotive primary wire and battery cable (even sizes,
/// plus 1 AWG and the aught sizes). Odd sizes still work everywhere else.
pub const AWG_SIZES: [Awg; 14] = [
    Awg(18),
    Awg(16),
    Awg(14),
    Awg(12),
    Awg(10),
    Awg(8),
    Awg(6),
    Awg(4),
    Awg(2),
    Awg(1),
    Awg(0),
    Awg(-1),
    Awg(-2),
    Awg(-3),
];

/// Voltage lost in a two-conductor copper run: `I * R * 2 * one_way_length`.
///
/// # Example
///
/// ```
/// use trainhorn::{electrical::{voltage_drop, Awg}, LengthUnit};
/// // 20 A over 10 ft (one way) of 10 AWG loses about 0.40 V.
/// let v = voltage_drop(20.0, 10.0, Awg::gauge(10)?, LengthUnit::Feet, 20.0)?;
/// assert!((v - 0.3995).abs() < 1e-3);
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn voltage_drop(
    amps: f64,
    one_way_length: f64,
    awg: Awg,
    unit: LengthUnit,
    temperature_c: f64,
) -> Result<f64> {
    let amps = positive("amps", amps)?;
    let length = positive("one_way_length", one_way_length)?;
    let feet = LengthUnit::Feet.from_meters(unit.to_meters(length));
    let ohms = awg.resistance_ohm_per_1000ft(temperature_c)? * (2.0 * feet) / 1000.0;
    Ok(amps * ohms)
}

/// Result of [`suggest_awg`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireSuggestion {
    /// The smallest gauge from [`AWG_SIZES`] that meets the budget.
    pub awg: Awg,
    /// Round-trip voltage drop with that gauge, in volts.
    pub voltage_drop_v: f64,
    /// The same drop as a percentage of `system_voltage`.
    pub voltage_drop_pct: f64,
    /// The budget that was requested, in percent.
    pub allowed_drop_pct: f64,
    /// Nominal system voltage.
    pub system_voltage: f64,
    /// Load current in amperes.
    pub amps: f64,
    /// One-way run length, in `unit`.
    pub one_way_length: f64,
    /// Unit of `one_way_length`.
    pub unit: LengthUnit,
}

/// Smallest copper AWG from [`AWG_SIZES`] whose round-trip voltage drop
/// stays within `max_drop_pct`.
///
/// * `amps`: continuous load current (> 0), e.g. a compressor's draw.
/// * `one_way_length`: distance from the power source to the load (> 0).
/// * `system_voltage`: nominal voltage, typically 12 or 24 (> 0).
/// * `max_drop_pct`: allowed drop in percent (0 < x <= 50); 3 is common.
/// * `unit`: unit of `one_way_length`.
/// * `temperature_c`: conductor temperature for the resistance (20 is usual).
///
/// # Errors
///
/// [`InputError`] on invalid input, or if even 4/0 cannot meet the budget.
///
/// # Example
///
/// ```
/// use trainhorn::{electrical::suggest_awg, LengthUnit};
/// let s = suggest_awg(30.0, 10.0, 12.0, 3.0, LengthUnit::Feet, 20.0)?;
/// assert_eq!(s.awg.to_string(), "6");
/// assert!(s.voltage_drop_pct <= 3.0);
/// // At 24 V the same load needs much thinner wire.
/// let s24 = suggest_awg(30.0, 10.0, 24.0, 3.0, LengthUnit::Feet, 20.0)?;
/// assert_eq!(s24.awg.to_string(), "10");
/// # Ok::<(), trainhorn::InputError>(())
/// ```
pub fn suggest_awg(
    amps: f64,
    one_way_length: f64,
    system_voltage: f64,
    max_drop_pct: f64,
    unit: LengthUnit,
    temperature_c: f64,
) -> Result<WireSuggestion> {
    let amps = positive("amps", amps)?;
    let one_way_length = positive("one_way_length", one_way_length)?;
    let system_voltage = positive("system_voltage", system_voltage)?;
    let max_drop_pct = positive("max_drop_pct", max_drop_pct)?;
    if max_drop_pct > 50.0 {
        return Err(InputError::new("max_drop_pct must be 50 or less"));
    }
    let allowed_v = system_voltage * max_drop_pct / 100.0;
    for awg in AWG_SIZES {
        let drop = voltage_drop(amps, one_way_length, awg, unit, temperature_c)?;
        if drop <= allowed_v {
            return Ok(WireSuggestion {
                awg,
                voltage_drop_v: drop,
                voltage_drop_pct: drop / system_voltage * 100.0,
                allowed_drop_pct: max_drop_pct,
                system_voltage,
                amps,
                one_way_length,
                unit,
            });
        }
    }
    Err(InputError::new(format!(
        "no gauge up to 4/0 keeps the drop within {max_drop_pct}% ({amps} A over \
         {one_way_length} {unit}); shorten the run, raise the system voltage or use \
         parallel conductors"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_parse_round_trip() {
        for n in 1..=40u8 {
            let a = Awg::gauge(n).unwrap();
            assert_eq!(a.to_string().parse::<Awg>().unwrap(), a);
        }
        for z in 1..=4u8 {
            let a = Awg::aught(z).unwrap();
            assert_eq!(a.to_string(), format!("{z}/0"));
            assert_eq!(a.to_string().parse::<Awg>().unwrap(), a);
        }
    }

    #[test]
    fn sizes_are_thin_to_thick() {
        for w in AWG_SIZES.windows(2) {
            assert!(w[0].diameter_mm() < w[1].diameter_mm());
        }
        assert_eq!(AWG_SIZES[10].to_string(), "1/0");
        assert_eq!(AWG_SIZES[13].to_string(), "4/0");
    }

    #[test]
    fn constructors_reject_out_of_range() {
        assert!(Awg::gauge(0).is_err());
        assert!(Awg::gauge(41).is_err());
        assert!(Awg::aught(0).is_err());
        assert!(Awg::aught(5).is_err());
    }
}
