//! End-to-end tests: every command through `run`, plus the real binary for
//! exit codes and stdout/stderr.

use std::process::Command;

use trainhorn_cli::{run, CliError};

fn args(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn ok(s: &str) -> String {
    run(&args(s)).unwrap_or_else(|e| panic!("`{s}` failed: {e:?}"))
}

fn usage_err(s: &str) -> String {
    match run(&args(s)) {
        Err(CliError::Usage(m)) => m,
        other => panic!("`{s}`: expected usage error, got {other:?}"),
    }
}

fn input_err(s: &str) -> String {
    match run(&args(s)) {
        Err(CliError::Input(m)) => m,
        other => panic!("`{s}`: expected input error, got {other:?}"),
    }
}

// ---- help / version

#[test]
fn no_args_and_help_print_usage() {
    for s in ["", "help", "--help", "-h"] {
        let out = ok(s);
        assert!(out.contains("USAGE:"), "{s}");
        assert!(out.contains("https://train-horn.com/"), "{s}");
    }
}

#[test]
fn version() {
    assert_eq!(
        ok("--version"),
        format!("trainhorn {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(ok("version"), ok("-V"));
}

#[test]
fn unknown_command() {
    assert_eq!(usage_err("honk"), "unknown command 'honk'");
}

// ---- db

#[test]
fn db_inverse_square_reference_value() {
    let out = ok("db --source 150 --ref-ft 10 --at-ft 100");
    assert!(out.contains("Source: 150 dB at 10 ft\n"), "{out}");
    assert!(out.contains("at 100 ft: 130 dB\n"), "{out}");
    assert!(out.contains("not a measurement"), "{out}");
}

#[test]
fn db_equals_form_and_several_points() {
    let out = ok("db --source=150 --ref-ft=10 --at-ft=20 --at-ft=1000");
    assert!(out.contains("at 20 ft: 144 dB"), "{out}"); // -6.02 dB per doubling
    assert!(out.contains("at 1000 ft: 110 dB"), "{out}");
    let a = out.find("20 ft").unwrap();
    let b = out.find("1000 ft").unwrap();
    assert!(a < b, "points keep command-line order");
}

#[test]
fn db_mixed_units_are_converted() {
    // 10 ft = 3.048 m, so 30.48 m is exactly 10x the reference: -20 dB.
    let out = ok("db --source 150 --ref-ft 10 --at-m 30.48");
    assert!(out.contains("at 30.48 m: 130 dB"), "{out}");
}

#[test]
fn db_with_octave_absorption() {
    // 1010 m from a 10 m reference at 500 Hz (1.9 dB/km): 150 - 40.09 - 1.9.
    let out = ok("db --source 150 --ref-m 10 --at-m 1010 --octave 500");
    assert!(out.contains("air absorption 1.9 dB/km"), "{out}");
    assert!(out.contains("at 1010 m: 108 dB"), "{out}");
    let custom = ok("db --source 150 --ref-m 10 --at-m 1010 --absorption 1.9");
    assert_eq!(out, custom);
}

#[test]
fn db_errors() {
    assert_eq!(
        usage_err("db --ref-ft 10 --at-ft 100"),
        "db: --source is required"
    );
    assert_eq!(
        usage_err("db --source 150 --at-ft 100"),
        "db: --ref-ft or --ref-m is required"
    );
    assert_eq!(
        usage_err("db --source 150 --ref-ft 10"),
        "db: at least one --at-ft or --at-m is required"
    );
    assert_eq!(
        usage_err("db --source 150 --ref-ft 10 --ref-m 3 --at-ft 100"),
        "db: give --ref-ft or --ref-m, not both"
    );
    assert_eq!(
        usage_err("db --source loud --ref-ft 10 --at-ft 100"),
        "db: --source must be a number, got 'loud'"
    );
    assert_eq!(
        usage_err("db --source 150 --source 140 --ref-ft 10 --at-ft 100"),
        "db: --source given more than once"
    );
    assert_eq!(
        usage_err("db --source 150 --ref-ft 10 --at-ft 100 --volume 3"),
        "db: unknown option --volume"
    );
    assert_eq!(usage_err("db --source"), "db: --source needs a value");
    assert_eq!(usage_err("db 150"), "db: unexpected argument '150'");
    assert_eq!(
        usage_err("db --source inf --ref-ft 10 --at-ft 100"),
        "db: --source must be a finite number, got 'inf'"
    );
    assert_eq!(
        input_err("db --source 150 --ref-ft 10 --at-ft -5"),
        "distance must be greater than 0, got -5"
    );
    assert!(input_err("db --source 150 --ref-ft 10 --at-ft 100 --octave 440").contains("440"));
    assert_eq!(
        usage_err("db --source 150 --ref-ft 10 --at-ft 100 --octave 500 --absorption 2"),
        "db: give --octave or --absorption, not both"
    );
}

// ---- distance

#[test]
fn distance_inverse() {
    let out = ok("distance --source 150 --ref-ft 10 --target 90");
    assert!(out.contains("falls to 90 dB at 10000 ft (3048 m)"), "{out}");
}

#[test]
fn distance_in_metres_with_absorption_is_shorter() {
    let free = ok("distance --source 150 --ref-m 1 --target 90");
    assert!(free.contains("at 1000 m (3280.84 ft)"), "{free}");
    let absorbed = ok("distance --source 150 --ref-m 1 --target 90 --octave 2000");
    assert!(absorbed.contains("air absorption 9.7 dB/km"), "{absorbed}");
    assert!(!absorbed.contains("at 1000 m"), "{absorbed}");
}

#[test]
fn distance_requires_target() {
    assert_eq!(
        usage_err("distance --source 150 --ref-ft 10"),
        "distance: --target is required"
    );
}

// ---- combine

#[test]
fn combine_two_equal_horns_adds_3_db() {
    let out = ok("combine 118 118");
    assert!(out.starts_with("118 + 118 dB combined: 121 dB\n"), "{out}");
}

#[test]
fn combine_errors() {
    assert!(usage_err("combine").contains("at least one level"));
    assert_eq!(
        usage_err("combine 100 abc"),
        "combine: 'abc' is not a number"
    );
    assert!(input_err("combine 100 NaN").contains("finite"));
}

// ---- tank

#[test]
fn tank_runtime_and_blasts() {
    // Library reference: 5 gal, 150 -> 110 psi, 10 CFM -> 10.9 s, 5 blasts of 2 s.
    let out = ok("tank --gallons 5 --start 150 --cutoff 110 --horn-cfm 10 --blast 2");
    assert!(out.contains("10 CFM horn runs for 10.9 s"), "{out}");
    assert!(out.contains("= 5 full 2-second blasts"), "{out}");
}

#[test]
fn tank_without_blast_and_bad_pressures() {
    let out = ok("tank --gallons 5 --start 150 --cutoff 110 --horn-cfm 10");
    assert!(!out.contains("blasts"), "{out}");
    assert_eq!(
        input_err("tank --gallons 5 --start 100 --cutoff 150 --horn-cfm 10"),
        "start_psi (100 psi) must be greater than cutoff_psi (150 psi)"
    );
}

// ---- refill

#[test]
fn refill_constant_and_linear() {
    let out = ok("refill --gallons 2 --cut-in 110 --cut-out 150 --cfm 1.0");
    assert!(out.contains("(constant 1 CFM): 43.7 s"), "{out}");
    let curve = ok(
        "refill --gallons 2 --cut-in 110 --cut-out 150 --cfm 1.0 --cfm-at-zero 2 --rated-psi 150",
    );
    assert!(
        curve.contains("linear flow, 2 CFM at 0 psi to 1 CFM at 150 psi"),
        "{curve}"
    );
    let secs: f64 = curve
        .rsplit(": ")
        .next()
        .unwrap()
        .trim_end_matches(" s\n")
        .parse()
        .unwrap();
    assert!(secs < 43.7, "{curve}");
}

#[test]
fn refill_long_times_use_minutes() {
    // 5 gal, 110 -> 150 psi at 1 CFM is about 109 s.
    let out = ok("refill --gallons 5 --cut-in 110 --cut-out 150 --cfm 1");
    assert!(out.contains(": 1 min 49 s"), "{out}");
}

#[test]
fn refill_curve_needs_both_flags() {
    assert_eq!(
        usage_err("refill --gallons 2 --cut-in 110 --cut-out 150 --cfm 1 --cfm-at-zero 2"),
        "refill: --cfm-at-zero and --rated-psi must be given together"
    );
}

// ---- wire / vdrop

#[test]
fn wire_suggestion_matches_library() {
    let out = ok("wire --amps 30 --length-ft 10 --volts 12");
    assert!(out.contains("max 3% drop"), "{out}");
    assert!(out.contains("  6 AWG copper"), "{out}");
    let out24 = ok("wire --amps 30 --length-ft 10 --volts 24");
    assert!(out24.contains("  10 AWG copper"), "{out24}");
}

#[test]
fn wire_in_metres_and_impossible_budget() {
    let ft = ok("wire --amps 30 --length-ft 10 --volts 12");
    let m = ok("wire --amps 30 --length-m 3.048 --volts 12");
    assert_eq!(ft.lines().nth(1), m.lines().nth(1));
    assert!(input_err("wire --amps 500 --length-ft 100 --volts 12 --max-drop 1").contains("4/0"));
}

#[test]
fn vdrop_reference_value() {
    // Library doc: 20 A over 10 ft of 10 AWG loses about 0.40 V.
    let out = ok("vdrop --awg 10 --amps 20 --length-ft 10 --volts 12");
    assert!(
        out.contains("10 AWG copper, 20 A over 10 ft one way: 0.4 V drop (3.33% of 12 V)"),
        "{out}"
    );
}

#[test]
fn vdrop_aught_sizes_and_errors() {
    let out = ok("vdrop --awg 2/0 --amps 100 --length-ft 10");
    assert!(out.starts_with("2/0 AWG copper"), "{out}");
    assert!(!out.contains('%'), "{out}");
    assert!(input_err("vdrop --awg 50 --amps 10 --length-ft 10").contains("AWG"));
    assert_eq!(
        usage_err("vdrop --amps 10 --length-ft 10"),
        "vdrop: --awg is required"
    );
    assert!(input_err("vdrop --awg 10 --amps 10 --length-ft 10 --volts 0").contains("volts"));
}

// ---- the real binary

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trainhorn"))
}

#[test]
fn binary_success_prints_to_stdout() {
    let o = bin()
        .args(args("db --source 150 --ref-ft 10 --at-ft 100"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("130 dB"));
    assert!(o.stderr.is_empty());
}

#[test]
fn binary_exit_codes() {
    let usage = bin().arg("nope").output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("unknown command"));
    assert!(usage.stdout.is_empty());

    let input = bin()
        .args(args("db --source 150 --ref-ft 10 --at-ft 0"))
        .output()
        .unwrap();
    assert_eq!(input.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&input.stderr).contains("distance must be greater than 0"));
}
