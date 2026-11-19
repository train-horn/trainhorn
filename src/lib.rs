//! Small, dependency-free calculators for train horn setups on trucks, cars
//! and boats.
//!
//! * [`acoustics`]: sound pressure level at a distance (inverse-square law),
//!   the inverse (distance for a target level), loss per doubling of
//!   distance, adding several horns, and optional atmospheric absorption.
//! * [`airsystem`]: free air in a tank (Boyle's law), continuous horn time,
//!   whole blasts per tank, and compressor refill time (constant or linear
//!   flow curve).
//! * [`electrical`]: copper AWG resistance computed from the AWG definition,
//!   round-trip voltage drop, and the smallest gauge for a voltage-drop budget.
//!
//! This crate is a port of the tested Python package
//! [trainhorn-calc](https://github.com/train-horn/trainhorn-calc): same
//! formulas, same constants, and the Rust tests assert the same reference
//! values. Browser versions of these calculators are on
//! [train-horn.com](https://train-horn.com/), an independent reference on
//! train horns.
//!
//! # Quick start
//!
//! ```
//! use trainhorn::{acoustics, airsystem, electrical, LengthUnit};
//!
//! // A horn rated 150 dB at 10 ft: how loud is it at 100 ft?
//! let spl = acoustics::spl_at_distance(150.0, 100.0, 10.0)?;
//! assert!((spl - 130.0).abs() < 1e-9);
//!
//! // How long does a 5 gallon tank (150 -> 110 psi) feed a 10 CFM horn?
//! let secs = airsystem::tank_runtime_seconds(5.0, 150.0, 110.0, 10.0, airsystem::ATMOSPHERIC_PSI)?;
//! assert_eq!(airsystem::blast_count(secs, 2.0)?, 5);
//!
//! // Which copper gauge keeps a 30 A compressor within 3 % drop over 10 ft at 12 V?
//! let wire = electrical::suggest_awg(30.0, 10.0, 12.0, 3.0, LengthUnit::Feet, 20.0)?;
//! assert_eq!(wire.awg.to_string(), "6");
//! # Ok::<(), trainhorn::InputError>(())
//! ```
//!
//! # Errors
//!
//! Every function that takes user input validates it and returns
//! [`InputError`] for non-finite numbers, zero or negative distances,
//! inverted pressure ranges, unknown units or gauges, and similar mistakes.
//! Nothing panics on bad input.
//!
//! # Scope
//!
//! Results are engineering estimates, not measurements. The acoustic model
//! is a free-field point source; it is not a legal noise assessment. The wire
//! sizing checks voltage drop only, not ampacity or fuse sizing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod acoustics;
pub mod airsystem;
pub mod electrical;
mod error;
mod units;

pub use error::InputError;
pub use units::LengthUnit;

/// Result type used by every fallible calculator in this crate.
pub type Result<T> = core::result::Result<T, InputError>;

// Compile and run the README examples as doc tests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
