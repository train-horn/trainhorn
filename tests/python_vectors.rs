//! The reference vectors of the Python package trainhorn-calc
//! (tests/test_trainhorn_calc.py), ported one to one.

use trainhorn::acoustics::{self, *};
use trainhorn::airsystem::{self, *};
use trainhorn::electrical::{self, *};
use trainhorn::{InputError, LengthUnit};

const FT: LengthUnit = LengthUnit::Feet;
const M: LengthUnit = LengthUnit::Meters;
const ATM: f64 = ATMOSPHERIC_PSI;

#[track_caller]
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} (tol {tol})");
}

/// Python's assertAlmostEqual default: 7 decimal places.
#[track_caller]
fn almost(a: f64, b: f64) {
    close(a, b, 0.5e-7);
}

fn awg(s: &str) -> Awg {
    s.parse().unwrap()
}

// ---------------------------------------------------------------- acoustics

#[test]
fn spl_150db_at_10ft_gives_130_at_100ft() {
    close(spl_at_distance(150.0, 100.0, 10.0).unwrap(), 130.0, 1e-9);
}

#[test]
fn spl_same_distance_returns_source() {
    almost(spl_at_distance(118.0, 10.0, 10.0).unwrap(), 118.0);
}

#[test]
fn spl_closer_than_reference_is_louder() {
    almost(spl_at_distance(150.0, 5.0, 10.0).unwrap(), 150.0 + 20.0 * 2f64.log10());
}

#[test]
fn spl_metres_and_custom_reference() {
    // 120 dB at 1 m -> 100 dB at 10 m
    almost(spl_at_distance_with_absorption(120.0, 10.0, 1.0, M, 0.0).unwrap(), 100.0);
}

#[test]
fn spl_unit_does_not_matter_without_absorption() {
    almost(
        spl_at_distance_with_absorption(140.0, 300.0, 10.0, FT, 0.0).unwrap(),
        spl_at_distance_with_absorption(140.0, 300.0, 10.0, M, 0.0).unwrap(),
    );
}

#[test]
fn spl_absorption_reduces_level() {
    let plain = spl_at_distance_with_absorption(150.0, 1010.0, 10.0, M, 0.0).unwrap();
    let absorbed = spl_at_distance_with_absorption(150.0, 1010.0, 10.0, M, 2.0).unwrap();
    close(plain - absorbed, 2.0, 1e-9);
}

#[test]
fn spl_absorption_converts_feet() {
    let plain = spl_at_distance_with_absorption(150.0, 3290.84, 10.0, FT, 0.0).unwrap();
    let absorbed = spl_at_distance_with_absorption(150.0, 3290.84, 10.0, FT, 10.0).unwrap();
    close(plain - absorbed, 10.0 * (3290.84 - 10.0) * 0.3048 / 1000.0, 1e-9);
}

#[test]
fn distance_for_spl_known_value() {
    close(distance_for_spl(150.0, 130.0, 10.0).unwrap(), 100.0, 1e-9);
    close(distance_for_spl(150.0, 90.0, 10.0).unwrap(), 10000.0, 1e-6);
}

#[test]
fn round_trip_without_absorption() {
    for (src, d) in [(150.0, 37.5), (118.0, 2.0), (153.2, 2500.0), (100.0, 10.0)] {
        let target = spl_at_distance(src, d, 10.0).unwrap();
        close(distance_for_spl(src, target, 10.0).unwrap(), d, 1e-6);
    }
}

#[test]
fn round_trip_with_absorption() {
    for d in [3.0, 50.0, 800.0, 5000.0] {
        let target = spl_at_distance_with_absorption(150.0, d, 10.0, M, 3.7).unwrap();
        let back = distance_for_spl_with_absorption(150.0, target, 10.0, M, 3.7).unwrap();
        close(back, d, d * 1e-9);
    }
}

#[test]
fn absorption_shortens_distance() {
    assert!(
        distance_for_spl_with_absorption(150.0, 90.0, 10.0, FT, 1.9).unwrap()
            < distance_for_spl(150.0, 90.0, 10.0).unwrap()
    );
}

#[test]
fn doubling_loss() {
    close(db_doubling_loss(), 6.0206, 0.5e-4);
    almost(distance_ratio_loss(10.0).unwrap(), 20.0);
}

#[test]
fn combine_levels_vectors() {
    almost(combine_levels(&[100.0, 100.0]).unwrap(), 100.0 + 10.0 * 2f64.log10());
    almost(combine_levels(&[150.0]).unwrap(), 150.0);
    close(combine_levels(&[120.0, 90.0]).unwrap(), 120.0, 0.5e-2);
}

#[test]
fn invalid_acoustic_inputs() {
    assert!(spl_at_distance(150.0, 0.0, 10.0).is_err());
    assert!(spl_at_distance(150.0, -5.0, 10.0).is_err());
    assert!(spl_at_distance(150.0, 10.0, 0.0).is_err());
    assert!("yd".parse::<LengthUnit>().is_err());
    assert!(spl_at_distance(f64::NAN, 10.0, 10.0).is_err());
    assert!(spl_at_distance_with_absorption(150.0, 10.0, 10.0, FT, -1.0).is_err());
    assert!(combine_levels(&[]).is_err());
    assert!(combine_levels(&[100.0, f64::INFINITY]).is_err());
    assert!(distance_ratio_loss(0.0).is_err());
}

#[test]
fn unit_strings_are_accepted() {
    let unit: LengthUnit = " FT ".parse().unwrap();
    almost(spl_at_distance_with_absorption(150.0, 100.0, 10.0, unit, 0.0).unwrap(), 130.0);
}

// ---------------------------------------------------------------- air system

#[test]
fn free_air_known_value() {
    // 1 gallon from 14.696 psig to 0 psig releases exactly 1 gallon of free air
    almost(free_air_volume_cuft(1.0, 14.696, 0.0, ATM).unwrap(), 231.0 / 1728.0);
}

#[test]
fn runtime_known_value() {
    let expected = 5.0 * 231.0 / 1728.0 * 40.0 / 14.696 / 10.0 * 60.0;
    almost(tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, ATM).unwrap(), expected);
    close(expected, 10.916, 0.5e-3);
}

#[test]
fn runtime_scales_linearly() {
    let base = tank_runtime_seconds(2.0, 150.0, 120.0, 5.0, ATM).unwrap();
    almost(tank_runtime_seconds(4.0, 150.0, 120.0, 5.0, ATM).unwrap(), 2.0 * base);
    almost(tank_runtime_seconds(2.0, 150.0, 120.0, 10.0, ATM).unwrap(), base / 2.0);
    almost(tank_runtime_seconds(2.0, 150.0, 90.0, 5.0, ATM).unwrap(), 2.0 * base);
}

#[test]
fn altitude_gives_more_free_air() {
    let sea = tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, ATM).unwrap();
    let high = tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, 12.2).unwrap();
    assert!(high > sea);
}

#[test]
fn blast_count_vectors() {
    assert_eq!(blast_count(10.0, 2.0).unwrap(), 5);
    assert_eq!(blast_count(10.9, 2.0).unwrap(), 5);
    assert_eq!(blast_count(0.0, 1.5).unwrap(), 0);
    assert_eq!(blast_count(0.3 * 3.0, 0.3).unwrap(), 3);
}

#[test]
fn recovery_constant_flow() {
    let expected = 2.0 * 231.0 / 1728.0 * 40.0 / 14.696 / 1.0 * 60.0;
    almost(compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, None, ATM).unwrap(), expected);
}

#[test]
fn recovery_equals_runtime_with_same_cfm() {
    almost(
        compressor_recovery_seconds(3.0, 120.0, 150.0, 2.5, None, ATM).unwrap(),
        tank_runtime_seconds(3.0, 150.0, 120.0, 2.5, ATM).unwrap(),
    );
}

#[test]
fn recovery_linear_model() {
    let constant = compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, None, ATM).unwrap();
    // A flat curve degenerates to the constant model.
    let flat = LinearFlow { cfm_at_zero_psi: 1.0, rated_psi: 150.0 };
    almost(compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, Some(flat), ATM).unwrap(), constant);
    // Falling curve: closed form vs. numeric (midpoint) integration.
    let falling = LinearFlow { cfm_at_zero_psi: 2.0, rated_psi: 150.0 };
    let t = compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, Some(falling), ATM).unwrap();
    let v = 2.0 * 231.0 / 1728.0 / 14.696;
    let steps = 20000;
    let dp = 40.0 / steps as f64;
    let numeric: f64 = (0..steps)
        .map(|i| v * dp / (2.0 - (110.0 + (i as f64 + 0.5) * dp) / 150.0))
        .sum::<f64>()
        * 60.0;
    close(t, numeric, 0.5e-4);
    assert!(t < constant);
}

#[test]
fn invalid_air_inputs() {
    assert!(tank_runtime_seconds(0.0, 150.0, 110.0, 10.0, ATM).is_err());
    assert!(tank_runtime_seconds(5.0, 110.0, 150.0, 10.0, ATM).is_err()); // start below cut-off
    assert!(tank_runtime_seconds(5.0, 150.0, 150.0, 10.0, ATM).is_err());
    assert!(tank_runtime_seconds(5.0, 150.0, -1.0, 10.0, ATM).is_err());
    assert!(tank_runtime_seconds(5.0, 150.0, 110.0, 0.0, ATM).is_err());
    assert!(tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, 0.0).is_err());
    let weak = LinearFlow { cfm_at_zero_psi: 0.5, rated_psi: 150.0 };
    assert!(compressor_recovery_seconds(2.0, 110.0, 150.0, 1.0, Some(weak), ATM).is_err());
    let dies = LinearFlow { cfm_at_zero_psi: 5.0, rated_psi: 100.0 };
    assert!(compressor_recovery_seconds(2.0, 110.0, 150.0, 0.1, Some(dies), ATM).is_err());
    assert!(blast_count(10.0, 0.0).is_err());
}

// ---------------------------------------------------------------- electrical

#[test]
fn resistance_matches_published_tables() {
    // Reference solid-copper values (ohm / 1000 ft at 20 C), within 1 %.
    let published = [
        ("14", 2.525),
        ("12", 1.588),
        ("10", 0.9989),
        ("8", 0.6282),
        ("4", 0.2485),
        ("1/0", 0.09827),
    ];
    for (size, ohms) in published {
        close(awg(size).resistance_ohm_per_1000ft(20.0).unwrap(), ohms, ohms * 0.01);
    }
}

#[test]
fn three_gauges_doubles_area() {
    close(awg("7").area_mm2() / awg("10").area_mm2(), 2.0, 0.01);
}

#[test]
fn awg_label_forms() {
    assert_eq!(awg("1/0").diameter_mm(), awg("0").diameter_mm());
    assert_eq!(awg("4/0").diameter_mm(), awg("0000").diameter_mm());
    assert_eq!(Awg::gauge(10).unwrap().diameter_mm(), awg("10 AWG").diameter_mm());
    close(awg("4/0").diameter_mm(), 11.684, 0.5e-2);
}

#[test]
fn temperature_raises_resistance() {
    let r20 = awg("10").resistance_ohm_per_1000ft(20.0).unwrap();
    almost(awg("10").resistance_ohm_per_1000ft(70.0).unwrap(), r20 * (1.0 + 0.00393 * 50.0));
}

#[test]
fn voltage_drop_known_value() {
    let drop = voltage_drop(20.0, 10.0, awg("10"), FT, 20.0).unwrap();
    almost(drop, 20.0 * awg("10").resistance_ohm_per_1000ft(20.0).unwrap() * 0.02);
    close(drop, 0.3995, 0.5e-3);
}

#[test]
fn voltage_drop_metres() {
    almost(
        voltage_drop(20.0, 3.048, awg("10"), M, 20.0).unwrap(),
        voltage_drop(20.0, 10.0, awg("10"), FT, 20.0).unwrap(),
    );
}

fn suggest(amps: f64, len: f64, volts: f64, pct: f64) -> Result<WireSuggestion, InputError> {
    suggest_awg(amps, len, volts, pct, FT, 20.0)
}

#[test]
fn suggest_awg_known_values() {
    assert_eq!(suggest(30.0, 10.0, 12.0, 3.0).unwrap().awg.to_string(), "6");
    assert_eq!(suggest(30.0, 10.0, 24.0, 3.0).unwrap().awg.to_string(), "10");
    assert_eq!(suggest(5.0, 3.0, 12.0, 3.0).unwrap().awg.to_string(), "18");
    // Python CLI vector: wire 30 10 --volts 24 --max-drop 2 -> 8 AWG
    assert_eq!(suggest(30.0, 10.0, 24.0, 2.0).unwrap().awg.to_string(), "8");
}

#[test]
fn suggestion_is_minimal_and_within_budget() {
    for (amps, len, volts) in [(30.0, 10.0, 12.0), (15.0, 20.0, 12.0), (40.0, 6.0, 24.0), (100.0, 15.0, 12.0)] {
        let s = suggest(amps, len, volts, 3.0).unwrap();
        assert!(s.voltage_drop_pct <= 3.0);
        let idx = AWG_SIZES.iter().position(|a| *a == s.awg).unwrap();
        if idx > 0 {
            let thinner = voltage_drop(amps, len, AWG_SIZES[idx - 1], FT, 20.0).unwrap();
            assert!(thinner / volts * 100.0 > 3.0);
        }
    }
}

#[test]
fn suggestion_fields() {
    let s = suggest(30.0, 10.0, 12.0, 3.0).unwrap();
    assert_eq!(s.unit, FT);
    assert_eq!(s.amps, 30.0);
    assert_eq!(s.allowed_drop_pct, 3.0);
    close(s.voltage_drop_pct, s.voltage_drop_v / 12.0 * 100.0, 1e-12);
}

#[test]
fn impossible_load_raises() {
    assert!(suggest(1000.0, 200.0, 12.0, 3.0).is_err());
}

#[test]
fn invalid_electrical_inputs() {
    for bad in ["5/0", "abc", "-1", "41", ""] {
        assert!(bad.parse::<Awg>().is_err(), "{bad:?} should be rejected");
    }
    assert!(suggest(0.0, 10.0, 12.0, 3.0).is_err());
    assert!(suggest(10.0, 10.0, 12.0, 0.0).is_err());
    assert!(suggest(10.0, 10.0, 12.0, 60.0).is_err());
    assert!(suggest(10.0, 10.0, -12.0, 3.0).is_err());
    assert!(awg("10").resistance_ohm_per_1000ft(500.0).is_err());
}

#[test]
fn modules_are_public() {
    // Paths used in the README.
    let _ = acoustics::AIR_ABSORPTION_DB_PER_KM;
    let _ = airsystem::CUBIC_FEET_PER_GALLON;
    let _ = electrical::COPPER_RESISTIVITY_OHM_M;
}
