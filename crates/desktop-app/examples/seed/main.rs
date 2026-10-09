//! Seeds a QA project for `orchester-app`: a new git repository with the project Kontur (KON) and one
//! scenario's data, registered in the application database of a folder of the app's own. `tools/qa-app.ps1`
//! runs it; the scenarios are in `kontur/scenarios/`.
//!
//! ```text
//! cargo run -p orchester-app --example seed -- --data-dir <app folder> --repo <new folder> --scenario <name>
//!   [--fake-codex <orchester-fake-codex>]
//! ```
//!
//! It prints the project's id (for the app's `navigate` tool), the repository, the application database and
//! each task's key with the state it was left in.

mod kontur;

use std::{
  io::{self, Write},
  path::PathBuf,
  process::ExitCode,
};

use kontur::{SeedRequest, Seeded, scenarios};
use orchester_core::time::Timestamp;

const USAGE: &str = "usage: seed --data-dir <dir> --repo <dir> --scenario <name> [--fake-codex <path>]";

fn main() -> ExitCode {
  let outcome = request(std::env::args().skip(1)).and_then(|request| {
    kontur::seed(&request).map_err(|error| format!("cannot seed {}: {error}", request.scenario.name()))
  });
  match outcome.map(|seeded| report(&seeded)) {
    Ok(Ok(())) => ExitCode::SUCCESS,
    // Standard output went away: the exit code is all that can still tell.
    Ok(Err(_)) => ExitCode::FAILURE,
    Err(message) => {
      // When standard error cannot be written either, the exit code is all that can still tell.
      let _ = writeln!(io::stderr(), "{message}");
      ExitCode::FAILURE
    }
  }
}

/// The request the arguments make, or what is wrong with them.
fn request(mut args: impl Iterator<Item = String>) -> Result<SeedRequest, String> {
  let (mut data_dir, mut repo, mut scenario, mut fake_codex) = (None, None, None, None);
  while let Some(flag) = args.next() {
    let value = args.next().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
    match flag.as_str() {
      "--data-dir" => data_dir = Some(PathBuf::from(value)),
      "--repo" => repo = Some(PathBuf::from(value)),
      "--scenario" => scenario = Some(value),
      "--fake-codex" => fake_codex = Some(PathBuf::from(value)),
      _ => return Err(format!("unknown argument {flag}\n{USAGE}")),
    }
  }
  let name = scenario.ok_or_else(|| format!("--scenario is required\n{USAGE}"))?;
  let scenario = scenarios::named(&name).ok_or_else(|| {
    let known: Vec<&str> = scenarios::ALL.iter().map(|scenario| scenario.name()).collect();
    format!("unknown scenario {name}; known: {}", known.join(", "))
  })?;
  Ok(SeedRequest {
    data_dir: data_dir.ok_or_else(|| format!("--data-dir is required\n{USAGE}"))?,
    repo: repo.ok_or_else(|| format!("--repo is required\n{USAGE}"))?,
    scenario,
    fake_codex,
    now: Timestamp::now(),
  })
}

fn report(seeded: &Seeded) -> io::Result<()> {
  let mut out = io::stdout().lock();
  writeln!(out, "project {}", seeded.project)?;
  writeln!(out, "repo {}", seeded.repo.display())?;
  writeln!(out, "app database {}", seeded.app_database.display())?;
  for task in &seeded.tasks {
    writeln!(out, "{:<7} {:<36} {}", task.key, task.state, task.title)?;
  }
  Ok(())
}
