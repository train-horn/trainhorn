//! Plan a truck train horn install: loudness at distance, tank runtime,
//! compressor recovery and compressor wire size.
//!
//! Run: `cargo run --example horn_setup`

use trainhorn::airsystem::{self, ATMOSPHERIC_PSI};
use trainhorn::{acoustics, electrical, LengthUnit};

fn main() -> Result<(), trainhorn::InputError> {
    // Horn rated 150 dB at 10 ft.
    println!("Horn rated 150 dB @ 10 ft (free-field estimate):");
    for ft in [50.0, 100.0, 500.0, 1000.0] {
        let db = acoustics::spl_at_distance(150.0, ft, 10.0)?;
        println!("  {ft:>6} ft: {db:5.1} dB");
    }
    let ft = acoustics::distance_for_spl(150.0, 85.0, 10.0)?;
    println!("  falls to 85 dB at about {ft:.0} ft");

    // 5 gal tank, 150 psi cut-out, horn usable down to 110 psi, 10 CFM horn.
    let runtime = airsystem::tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, ATMOSPHERIC_PSI)?;
    let blasts = airsystem::blast_count(runtime, 2.0)?;
    println!("\n5 gal tank, 150 -> 110 psi, 10 CFM horn: {runtime:.1} s ({blasts} two-second blasts)");

    // 1.0 CFM compressor refilling 110 -> 150 psi.
    let refill = airsystem::compressor_recovery_seconds(5.0, 110.0, 150.0, 1.0, None, ATMOSPHERIC_PSI)?;
    println!("1.0 CFM compressor refill: {refill:.0} s");

    // 30 A compressor, 12 ft from the battery, 12 V, 3 % drop budget.
    let wire = electrical::suggest_awg(30.0, 12.0, 12.0, 3.0, LengthUnit::Feet, 20.0)?;
    println!(
        "\n30 A compressor, 12 ft run at 12 V: {} AWG copper ({:.2} V, {:.1} % drop)",
        wire.awg, wire.voltage_drop_v, wire.voltage_drop_pct
    );
    println!("\nMore calculators: https://train-horn.com/");
    Ok(())
}
