# trainhorn

[![crates.io](https://img.shields.io/crates/v/trainhorn.svg)](https://crates.io/crates/trainhorn)
[![docs.rs](https://docs.rs/trainhorn/badge.svg)](https://docs.rs/trainhorn)

Small, dependency-free Rust calculators for train horn setups on trucks, cars
and boats:

- **Acoustics**: sound level at a distance (inverse-square law), the inverse
  (distance for a target level), loss per doubling of distance, adding several
  horns, and optional atmospheric absorption.
- **Air system**: free air in a tank (Boyle's law), continuous horn time,
  whole blasts per tank, and compressor refill time (constant or linear flow
  curve).
- **Electrical**: copper AWG resistance computed from the AWG definition,
  round-trip voltage drop, and the smallest gauge for a voltage-drop budget.

It is a port of the tested Python package
[trainhorn-calc](https://github.com/train-horn/trainhorn-calc): same formulas,
same constants, and the Rust test suite asserts the same reference values.
No `unsafe`, no dependencies, and every input is validated: bad values return
an `InputError`, nothing panics.

## Install

```toml
[dependencies]
trainhorn = "1"
```

Minimum supported Rust version: 1.65.

## Usage

```rust
use trainhorn::airsystem::{self, LinearFlow, ATMOSPHERIC_PSI};
use trainhorn::electrical::{self, Awg};
use trainhorn::{acoustics, LengthUnit};

fn main() -> Result<(), trainhorn::InputError> {
    // A horn rated 150 dB at 10 ft is about 130 dB at 100 ft.
    let db = acoustics::spl_at_distance(150.0, 100.0, 10.0)?;
    assert!((db - 130.0).abs() < 1e-9);

    // ...and drops to 90 dB at about 10,000 ft in a free field.
    let ft = acoustics::distance_for_spl(150.0, 90.0, 10.0)?;

    // Two equal horns add about 3 dB, not double.
    let pair = acoustics::combine_levels(&[150.0, 150.0])?; // 153.01

    // 5 gal tank, 150 -> 110 psi, 10 CFM horn: about 10.9 s, five 2 s blasts.
    let secs = airsystem::tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, ATMOSPHERIC_PSI)?;
    let blasts = airsystem::blast_count(secs, 2.0)?;

    // Compressor refill, constant flow or a linear CFM-vs-pressure curve.
    let curve = LinearFlow { cfm_at_zero_psi: 2.0, rated_psi: 150.0 };
    let refill = airsystem::compressor_recovery_seconds(
        2.0, 110.0, 150.0, 1.0, Some(curve), ATMOSPHERIC_PSI)?;

    // Smallest copper gauge for a 30 A compressor, 10 ft run, 12 V, 3 % drop: 6 AWG.
    let wire = electrical::suggest_awg(30.0, 10.0, 12.0, 3.0, LengthUnit::Feet, 20.0)?;
    assert_eq!(wire.awg.to_string(), "6");

    // Gauges parse from the usual labels.
    let ohms = "4/0".parse::<Awg>()?.resistance_ohm_per_1000ft(20.0)?;

    println!("{ft:.0} ft, {pair:.2} dB, {blasts} blasts, {refill:.0} s, {ohms:.4} ohm/kft");
    Ok(())
}
```

Run the bundled example for a full install plan:

```sh
cargo run --example horn_setup
```

## Command-line tool

The [`cli/`](cli) directory holds a small `trainhorn` command built on this
crate. It is packaged as a snap from [`snap/snapcraft.yaml`](snap/snapcraft.yaml)
(`sudo snap install trainhorn`), or build it with Cargo:

```sh
cargo install --path cli
trainhorn db --source 150 --ref-ft 10 --at-ft 100        # 130 dB
trainhorn distance --source 150 --ref-ft 10 --target 85
trainhorn combine 118 118                                # 121 dB
trainhorn tank --gallons 5 --start 150 --cutoff 110 --horn-cfm 10 --blast 2
trainhorn refill --gallons 5 --cut-in 110 --cut-out 150 --cfm 1.0
trainhorn wire --amps 30 --length-ft 12 --volts 12       # 6 AWG
trainhorn vdrop --awg 8 --amps 30 --length-ft 12 --volts 12
trainhorn help
```

Exit code 0 on success, 1 for an out-of-range value, 2 for a usage error.

## Models and limits

| Module | Formula | Main assumptions |
| --- | --- | --- |
| `acoustics` | `L = L_ref - 20 log10(d / d_ref) - alpha (d - d_ref)` | free field, point source, absorption optional (typical values for 10 °C / 70 % RH in `AIR_ABSORPTION_DB_PER_KM`) |
| `airsystem` | `V_free = V_tank (P_start - P_end) / P_atm` | isothermal (Boyle), horn uses its rated CFM down to the cut-off, no line or leak losses |
| `electrical` | `R = rho L / A`, ASTM B258 diameters, `V = I R 2 L` | solid annealed copper at 20 °C unless given; checks voltage drop only, not ampacity or fuse size |

Results are engineering estimates, not measurements, and the acoustic model is
not a legal noise assessment. Real outdoor levels can be several dB higher or
lower because of ground reflection, wind and barriers.

## Tests

```sh
cargo test     # unit tests, the ported Python reference vectors, and doc tests
cargo clippy --all-targets -- -D warnings
cd cli && cargo test   # command-line tool: every command, errors, exit codes
```

## Links

- Browser calculators and train horn reference: [train-horn.com](https://train-horn.com/)
- Python version: [trainhorn-calc](https://github.com/train-horn/trainhorn-calc)

## License

MIT. See [LICENSE](LICENSE).
