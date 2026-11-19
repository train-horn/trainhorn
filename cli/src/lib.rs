//! Argument parsing and output formatting for the `trainhorn` command-line
//! tool. The calculations come from the [`trainhorn`] library crate.
//!
//! [`run`] takes the arguments (without the program name) and returns the
//! text to print, so every command can be tested without spawning a process.

#![forbid(unsafe_code)]

use std::fmt;

use trainhorn::airsystem::{self, LinearFlow, ATMOSPHERIC_PSI};
use trainhorn::electrical::{self, Awg};
use trainhorn::{acoustics, InputError, LengthUnit};

/// Version printed by `trainhorn --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Why a command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// The command line itself is wrong (unknown command or flag, missing
    /// value). Exit code 2.
    Usage(String),
    /// The command line is well formed but a value is out of range (for
    /// example a negative distance). Exit code 1.
    Input(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(m) | CliError::Input(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for CliError {}

impl From<InputError> for CliError {
    fn from(e: InputError) -> Self {
        CliError::Input(e.to_string())
    }
}

type CliResult<T> = Result<T, CliError>;

fn usage(msg: impl Into<String>) -> CliError {
    CliError::Usage(msg.into())
}

/// Full help text.
pub const HELP: &str = "\
trainhorn - calculators for train horn setups on trucks, cars and boats

USAGE:
  trainhorn <command> [options]

COMMANDS:
  db        Sound level at a distance (inverse-square law)
              --source DB  (--ref-ft N | --ref-m N)  (--at-ft N | --at-m N)...
              [--octave HZ | --absorption DB_PER_KM]
  distance  Distance at which the level falls to a target
              --source DB  (--ref-ft N | --ref-m N)  --target DB
              [--octave HZ | --absorption DB_PER_KM]
  combine   Total level of several horns sounding together
              DB DB [DB...]
  tank      Horn time from one tank of air
              --gallons G --start PSI --cutoff PSI --horn-cfm CFM
              [--blast SECONDS] [--atm PSI]
  refill    Compressor time to refill the tank
              --gallons G --cut-in PSI --cut-out PSI --cfm CFM
              [--cfm-at-zero CFM --rated-psi PSI] [--atm PSI]
  wire      Smallest copper AWG for a voltage-drop budget
              --amps A (--length-ft N | --length-m N) --volts V
              [--max-drop PCT (default 3)] [--temp C (default 20)]
  vdrop     Voltage drop of a given AWG copper run
              --awg SIZE --amps A (--length-ft N | --length-m N)
              [--volts V] [--temp C (default 20)]
  help      Show this help
  version   Show the version

EXAMPLES:
  trainhorn db --source 150 --ref-ft 10 --at-ft 100
  trainhorn db --source 150 --ref-m 1 --at-m 50 --at-m 200 --octave 500
  trainhorn distance --source 150 --ref-ft 10 --target 85
  trainhorn combine 118 118
  trainhorn tank --gallons 5 --start 150 --cutoff 110 --horn-cfm 10 --blast 2
  trainhorn refill --gallons 5 --cut-in 110 --cut-out 150 --cfm 1.0
  trainhorn wire --amps 30 --length-ft 12 --volts 12
  trainhorn vdrop --awg 8 --amps 30 --length-ft 12 --volts 12

Distances in one command may mix feet and metres; they are converted.
Flags also accept the --flag=value form.

Results are free-field engineering estimates, not measurements and not a
legal noise assessment. Wire sizing checks voltage drop only, not ampacity.

Browser calculators and horn guides: https://train-horn.com/
";

/// Run one command. `args` excludes the program name.
pub fn run(args: &[String]) -> CliResult<String> {
    let Some((cmd, rest)) = args.split_first() else {
        return Ok(HELP.to_string());
    };
    match cmd.as_str() {
        "help" | "-h" | "--help" => Ok(HELP.to_string()),
        "version" | "-V" | "--version" => Ok(format!("trainhorn {VERSION}\n")),
        "db" => cmd_db(rest),
        "distance" => cmd_distance(rest),
        "combine" => cmd_combine(rest),
        "tank" => cmd_tank(rest),
        "refill" => cmd_refill(rest),
        "wire" => cmd_wire(rest),
        "vdrop" => cmd_vdrop(rest),
        other => Err(usage(format!("unknown command '{other}'"))),
    }
}

// ---------------------------------------------------------------- parsing

/// Parsed `--flag value` pairs, in command-line order.
struct Flags {
    cmd: &'static str,
    pairs: Vec<(String, String)>,
}

impl Flags {
    /// Parse `args` as `--name value` / `--name=value` pairs. Only names in
    /// `allowed` are accepted; names in `repeatable` may appear more than once.
    fn parse(
        cmd: &'static str,
        args: &[String],
        allowed: &[&str],
        repeatable: &[&str],
    ) -> CliResult<Flags> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            let Some(body) = arg.strip_prefix("--") else {
                return Err(usage(format!("{cmd}: unexpected argument '{arg}'")));
            };
            let (name, value) = match body.split_once('=') {
                Some((n, v)) => (n.to_string(), v.to_string()),
                None => {
                    let v = args
                        .get(i + 1)
                        .ok_or_else(|| usage(format!("{cmd}: --{body} needs a value")))?;
                    i += 1;
                    (body.to_string(), v.clone())
                }
            };
            if !allowed.contains(&name.as_str()) {
                return Err(usage(format!("{cmd}: unknown option --{name}")));
            }
            if !repeatable.contains(&name.as_str()) && pairs.iter().any(|(n, _)| *n == name) {
                return Err(usage(format!("{cmd}: --{name} given more than once")));
            }
            pairs.push((name, value));
            i += 1;
        }
        Ok(Flags { cmd, pairs })
    }

    fn raw(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn num_opt(&self, name: &str) -> CliResult<Option<f64>> {
        self.raw(name)
            .map(|v| parse_num(self.cmd, name, v))
            .transpose()
    }

    fn num(&self, name: &str) -> CliResult<f64> {
        self.num_opt(name)?
            .ok_or_else(|| usage(format!("{}: --{name} is required", self.cmd)))
    }

    fn num_or(&self, name: &str, default: f64) -> CliResult<f64> {
        Ok(self.num_opt(name)?.unwrap_or(default))
    }

    /// A length given as either `--<base>-ft` or `--<base>-m` (exactly one).
    fn length(&self, base: &str) -> CliResult<(f64, LengthUnit)> {
        let ft = self.num_opt(&format!("{base}-ft"))?;
        let m = self.num_opt(&format!("{base}-m"))?;
        match (ft, m) {
            (Some(v), None) => Ok((v, LengthUnit::Feet)),
            (None, Some(v)) => Ok((v, LengthUnit::Meters)),
            (Some(_), Some(_)) => Err(usage(format!(
                "{}: give --{base}-ft or --{base}-m, not both",
                self.cmd
            ))),
            (None, None) => Err(usage(format!(
                "{}: --{base}-ft or --{base}-m is required",
                self.cmd
            ))),
        }
    }

    /// Every `--<base>-ft` / `--<base>-m` value, in command-line order.
    fn lengths(&self, base: &str) -> CliResult<Vec<(f64, LengthUnit)>> {
        let ft = format!("{base}-ft");
        let m = format!("{base}-m");
        let mut out = Vec::new();
        for (name, value) in &self.pairs {
            let unit = if *name == ft {
                LengthUnit::Feet
            } else if *name == m {
                LengthUnit::Meters
            } else {
                continue;
            };
            out.push((parse_num(self.cmd, name, value)?, unit));
        }
        if out.is_empty() {
            return Err(usage(format!(
                "{}: at least one --{base}-ft or --{base}-m is required",
                self.cmd
            )));
        }
        Ok(out)
    }

    /// Air absorption in dB/km from `--octave HZ` or `--absorption DB_PER_KM`.
    fn absorption(&self) -> CliResult<f64> {
        match (self.raw("octave"), self.num_opt("absorption")?) {
            (Some(_), Some(_)) => Err(usage(format!(
                "{}: give --octave or --absorption, not both",
                self.cmd
            ))),
            (Some(hz), None) => {
                let hz: u32 = hz.trim().parse().map_err(|_| {
                    usage(format!(
                        "{}: --octave must be a whole number of Hz, got '{hz}'",
                        self.cmd
                    ))
                })?;
                acoustics::air_absorption_db_per_km(hz).ok_or_else(|| {
                    let bands: Vec<String> = acoustics::AIR_ABSORPTION_DB_PER_KM
                        .iter()
                        .map(|(hz, _)| hz.to_string())
                        .collect();
                    CliError::Input(format!(
                        "--octave must be one of {} Hz, got {hz}",
                        bands.join(", ")
                    ))
                })
            }
            (None, Some(a)) => Ok(a),
            (None, None) => Ok(0.0),
        }
    }
}

fn parse_num(cmd: &str, name: &str, value: &str) -> CliResult<f64> {
    let v: f64 = value
        .trim()
        .parse()
        .map_err(|_| usage(format!("{cmd}: --{name} must be a number, got '{value}'")))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(usage(format!(
            "{cmd}: --{name} must be a finite number, got '{value}'"
        )))
    }
}

/// Format a number with at most `max_decimals`, dropping trailing zeros.
fn num(v: f64, max_decimals: usize) -> String {
    let s = format!("{v:.max_decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn len(v: f64, unit: LengthUnit) -> String {
    format!("{} {}", num(v, 2), unit)
}

/// `90 s` / `2 min 30 s` / `1 h 5 min`.
fn duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{} s", num(seconds, 1));
    }
    let total = seconds.round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h} h {m} min")
    } else if s == 0 {
        format!("{m} min")
    } else {
        format!("{m} min {s} s")
    }
}

fn convert(value: f64, from: LengthUnit, to: LengthUnit) -> f64 {
    to.from_meters(from.to_meters(value))
}

const ESTIMATE_NOTE: &str = "Free-field estimate, not a measurement or a legal noise assessment.\n";

// ---------------------------------------------------------------- commands

fn cmd_db(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "db",
        args,
        &[
            "source",
            "ref-ft",
            "ref-m",
            "at-ft",
            "at-m",
            "octave",
            "absorption",
        ],
        &["at-ft", "at-m"],
    )?;
    let source = f.num("source")?;
    let (ref_d, ref_unit) = f.length("ref")?;
    let points = f.lengths("at")?;
    let absorption = f.absorption()?;

    let mut out = format!(
        "Source: {} dB at {}{}\n",
        num(source, 2),
        len(ref_d, ref_unit),
        absorption_note(absorption)
    );
    for (d, unit) in points {
        let ref_in_unit = convert(ref_d, ref_unit, unit);
        let db =
            acoustics::spl_at_distance_with_absorption(source, d, ref_in_unit, unit, absorption)?;
        out.push_str(&format!("  at {}: {} dB\n", len(d, unit), num(db, 1)));
    }
    out.push_str(ESTIMATE_NOTE);
    Ok(out)
}

fn absorption_note(absorption: f64) -> String {
    if absorption == 0.0 {
        String::new()
    } else {
        format!(", air absorption {} dB/km", num(absorption, 2))
    }
}

fn cmd_distance(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "distance",
        args,
        &[
            "source",
            "ref-ft",
            "ref-m",
            "target",
            "octave",
            "absorption",
        ],
        &[],
    )?;
    let source = f.num("source")?;
    let target = f.num("target")?;
    let (ref_d, unit) = f.length("ref")?;
    let absorption = f.absorption()?;
    let d = acoustics::distance_for_spl_with_absorption(source, target, ref_d, unit, absorption)?;
    let other = match unit {
        LengthUnit::Feet => LengthUnit::Meters,
        LengthUnit::Meters => LengthUnit::Feet,
    };
    Ok(format!(
        "Source: {} dB at {}{}\n  falls to {} dB at {} ({})\n{ESTIMATE_NOTE}",
        num(source, 2),
        len(ref_d, unit),
        absorption_note(absorption),
        num(target, 2),
        len(d, unit),
        len(convert(d, unit, other), other),
    ))
}

fn cmd_combine(args: &[String]) -> CliResult<String> {
    if args.is_empty() {
        return Err(usage(
            "combine: give at least one level in dB, e.g. `combine 118 118`",
        ));
    }
    let levels = args
        .iter()
        .map(|a| {
            let v: f64 = a
                .trim()
                .parse()
                .map_err(|_| usage(format!("combine: '{a}' is not a number")))?;
            Ok(v)
        })
        .collect::<CliResult<Vec<f64>>>()?;
    let total = acoustics::combine_levels(&levels)?;
    let list: Vec<String> = levels.iter().map(|v| num(*v, 2)).collect();
    Ok(format!(
        "{} dB combined: {} dB\n(incoherent sources: two equal horns add about 3 dB)\n",
        list.join(" + "),
        num(total, 1)
    ))
}

fn cmd_tank(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "tank",
        args,
        &["gallons", "start", "cutoff", "horn-cfm", "blast", "atm"],
        &[],
    )?;
    let gallons = f.num("gallons")?;
    let start = f.num("start")?;
    let cutoff = f.num("cutoff")?;
    let cfm = f.num("horn-cfm")?;
    let atm = f.num_or("atm", ATMOSPHERIC_PSI)?;
    let blast = f.num_opt("blast")?;

    let secs = airsystem::tank_runtime_seconds(gallons, start, cutoff, cfm, atm)?;
    let air = airsystem::free_air_volume_cuft(gallons, start, cutoff, atm)?;
    let mut out = format!(
        "{} gal tank, {} -> {} psi: {} cu ft of free air\n  {} CFM horn runs for {}\n",
        num(gallons, 2),
        num(start, 1),
        num(cutoff, 1),
        num(air, 3),
        num(cfm, 2),
        duration(secs)
    );
    if let Some(b) = blast {
        let n = airsystem::blast_count(secs, b)?;
        out.push_str(&format!("  = {n} full {}-second blasts\n", num(b, 2)));
    }
    Ok(out)
}

fn cmd_refill(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "refill",
        args,
        &[
            "gallons",
            "cut-in",
            "cut-out",
            "cfm",
            "cfm-at-zero",
            "rated-psi",
            "atm",
        ],
        &[],
    )?;
    let gallons = f.num("gallons")?;
    let cut_in = f.num("cut-in")?;
    let cut_out = f.num("cut-out")?;
    let cfm = f.num("cfm")?;
    let atm = f.num_or("atm", ATMOSPHERIC_PSI)?;
    let curve = match (f.num_opt("cfm-at-zero")?, f.num_opt("rated-psi")?) {
        (Some(q0), Some(p)) => Some(LinearFlow {
            cfm_at_zero_psi: q0,
            rated_psi: p,
        }),
        (None, None) => None,
        _ => {
            return Err(usage(
                "refill: --cfm-at-zero and --rated-psi must be given together",
            ))
        }
    };
    let secs = airsystem::compressor_recovery_seconds(gallons, cut_in, cut_out, cfm, curve, atm)?;
    let model = match curve {
        None => format!("constant {} CFM", num(cfm, 2)),
        Some(c) => format!(
            "linear flow, {} CFM at 0 psi to {} CFM at {} psi",
            num(c.cfm_at_zero_psi, 2),
            num(cfm, 2),
            num(c.rated_psi, 1)
        ),
    };
    Ok(format!(
        "{} gal tank, {} -> {} psi ({model}): {}\n",
        num(gallons, 2),
        num(cut_in, 1),
        num(cut_out, 1),
        duration(secs)
    ))
}

fn cmd_wire(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "wire",
        args,
        &["amps", "length-ft", "length-m", "volts", "max-drop", "temp"],
        &[],
    )?;
    let amps = f.num("amps")?;
    let (length, unit) = f.length("length")?;
    let volts = f.num("volts")?;
    let max_drop = f.num_or("max-drop", 3.0)?;
    let temp = f.num_or("temp", 20.0)?;
    let s = electrical::suggest_awg(amps, length, volts, max_drop, unit, temp)?;
    Ok(format!(
        "{} A over {} one way at {} V, max {}% drop:\n  {} AWG copper: {} V drop ({}%)\n\
         Voltage drop only; check ampacity and fuse size separately.\n",
        num(amps, 2),
        len(length, unit),
        num(volts, 2),
        num(max_drop, 2),
        s.awg,
        num(s.voltage_drop_v, 3),
        num(s.voltage_drop_pct, 2)
    ))
}

fn cmd_vdrop(args: &[String]) -> CliResult<String> {
    let f = Flags::parse(
        "vdrop",
        args,
        &["awg", "amps", "length-ft", "length-m", "volts", "temp"],
        &[],
    )?;
    let awg: Awg = f
        .raw("awg")
        .ok_or_else(|| usage("vdrop: --awg is required"))?
        .parse()?;
    let amps = f.num("amps")?;
    let (length, unit) = f.length("length")?;
    let temp = f.num_or("temp", 20.0)?;
    let volts = f.num_opt("volts")?;
    let drop = electrical::voltage_drop(amps, length, awg, unit, temp)?;
    let mut out = format!(
        "{} AWG copper, {} A over {} one way: {} V drop",
        awg,
        num(amps, 2),
        len(length, unit),
        num(drop, 3)
    );
    if let Some(v) = volts {
        if v <= 0.0 {
            return Err(CliError::Input(format!(
                "volts must be greater than 0, got {}",
                num(v, 3)
            )));
        }
        out.push_str(&format!(
            " ({}% of {} V)",
            num(drop / v * 100.0, 2),
            num(v, 2)
        ));
    }
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting() {
        assert_eq!(num(130.0, 1), "130");
        assert_eq!(num(129.96, 1), "130");
        assert_eq!(num(3.0103, 2), "3.01");
        assert_eq!(num(-0.0001, 2), "0");
        assert_eq!(num(1500.0, 0), "1500");
    }

    #[test]
    fn durations() {
        assert_eq!(duration(9.96), "10 s");
        assert_eq!(duration(43.7), "43.7 s");
        assert_eq!(duration(60.0), "1 min");
        assert_eq!(duration(150.4), "2 min 30 s");
        assert_eq!(duration(3900.0), "1 h 5 min");
    }

    #[test]
    fn unit_conversion_roundtrip() {
        let m = convert(10.0, LengthUnit::Feet, LengthUnit::Meters);
        assert!((m - 3.048).abs() < 1e-12);
        assert!((convert(m, LengthUnit::Meters, LengthUnit::Feet) - 10.0).abs() < 1e-12);
    }
}
