//! `trainhorn` command-line tool. All logic lives in `lib.rs` so it can be
//! unit tested; this file only wires up stdin/stdout and the exit code.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match trainhorn_cli::run(&args) {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(trainhorn_cli::CliError::Usage(msg)) => {
            eprintln!("trainhorn: {msg}\nRun `trainhorn help` for usage.");
            ExitCode::from(2)
        }
        Err(trainhorn_cli::CliError::Input(msg)) => {
            eprintln!("trainhorn: {msg}");
            ExitCode::from(1)
        }
    }
}
