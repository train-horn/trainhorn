use std::fmt;
use std::str::FromStr;

use crate::InputError;

/// Metres in one international foot (exact).
pub(crate) const METERS_PER_FOOT: f64 = 0.3048;

/// Length unit for distances and cable runs.
///
/// Parses from `"ft"` / `"m"` (case-insensitive, surrounding spaces ignored;
/// `"feet"`, `"foot"`, `"meter(s)"` and `"metre(s)"` are accepted too).
///
/// ```
/// use trainhorn::LengthUnit;
/// assert_eq!(" FT ".parse::<LengthUnit>(), Ok(LengthUnit::Feet));
/// assert_eq!("m".parse::<LengthUnit>(), Ok(LengthUnit::Meters));
/// assert!("yd".parse::<LengthUnit>().is_err());
/// assert!((LengthUnit::Feet.to_meters(10.0) - 3.048).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LengthUnit {
    /// International feet (0.3048 m). The default, as horn ratings in the US
    /// are usually quoted "at 10 ft".
    #[default]
    Feet,
    /// Metres.
    Meters,
}

impl LengthUnit {
    /// Convert `value` in this unit to metres.
    pub fn to_meters(self, value: f64) -> f64 {
        match self {
            LengthUnit::Feet => value * METERS_PER_FOOT,
            LengthUnit::Meters => value,
        }
    }

    /// Convert `meters` to this unit.
    pub fn from_meters(self, meters: f64) -> f64 {
        match self {
            LengthUnit::Feet => meters / METERS_PER_FOOT,
            LengthUnit::Meters => meters,
        }
    }

    /// Short symbol: `"ft"` or `"m"`.
    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Feet => "ft",
            LengthUnit::Meters => "m",
        }
    }
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl FromStr for LengthUnit {
    type Err = InputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ft" | "feet" | "foot" => Ok(LengthUnit::Feet),
            "m" | "meter" | "meters" | "metre" | "metres" => Ok(LengthUnit::Meters),
            _ => Err(InputError::new(format!(
                "unit must be one of ft, m; got {s:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for u in [LengthUnit::Feet, LengthUnit::Meters] {
            assert!((u.from_meters(u.to_meters(123.4)) - 123.4).abs() < 1e-12);
            assert_eq!(u.symbol().parse::<LengthUnit>(), Ok(u));
            assert_eq!(u.to_string(), u.symbol());
        }
        assert_eq!(LengthUnit::default(), LengthUnit::Feet);
    }
}
