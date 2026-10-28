//! Seeds a QA project for `orchester-app`: a new git repository with the project Kontur (KON) and one
//! scenario's data, registered in the application database of a folder of the app's own. `tools/qa-app.ps1`
//! runs it; the scenarios are in `kontur/scenarios/`, registered in its `ALL`, the only list of them.
//!
//! ```text
//! cargo run -p orchester-app --example seed -- --data-dir <app folder> --repo <new folder> --scenario <name>
//!   [--fake-codex <orchester-fake-codex>]
//! cargo run -p orchester-app --example seed -- --list-scenarios
//! ```
//!
//! It prints the project's id (for the app's `navigate` tool), the repository, the application database and
//! each task's key with the state it was left in. An unknown scenario exits non-zero with the known ones, sorted,
//! and writes nothing; `--list-scenarios` prints them, one per line.

mod cli;
mod kontur;

use std::{
  io::{self, Write},
  process::ExitCode,
};

use cli::CliError;

fn main() -> ExitCode {
  match cli::run(std::env::args().skip(1), &mut io::stdout().lock()) {
    Ok(()) => ExitCode::SUCCESS,
    // Standard output went away: the exit code is all that can still tell.
    Err(CliError::Output(_)) => ExitCode::FAILURE,
    Err(error) => {
      // When standard error cannot be written either, the exit code is all that can still tell.
      let _ = writeln!(io::stderr(), "{error}");
      ExitCode::FAILURE
    }
  }
}
