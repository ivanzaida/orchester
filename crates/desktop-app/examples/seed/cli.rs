//! The seed's command line: the arguments, the scenario they name checked against the registry
//! ([`scenarios::ALL`], the only list of scenarios), and what the seed prints. Shared by the `seed` example and its
//! tests (`tests/seed/command_line.rs`).

use std::{io, io::Write, path::PathBuf};

use orchester_core::time::Timestamp;

use crate::kontur::{self, SeedError, SeedRequest, Seeded, scenarios};

const USAGE: &str =
  "usage: seed --data-dir <dir> --repo <dir> --scenario <name> [--fake-codex <path>]\n       seed --list-scenarios";
const LIST_SCENARIOS: &str = "--list-scenarios";

/// What the arguments ask for.
pub enum Command {
  Seed(SeedRequest),
  /// The registry's scenario names, sorted, one per line.
  ListScenarios,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
  #[error("{0} needs a value\n{USAGE}")]
  NoValue(String),
  #[error("unknown argument {0}\n{USAGE}")]
  UnknownArgument(String),
  #[error("{LIST_SCENARIOS} takes no other argument\n{USAGE}")]
  ListWithOthers,
  #[error("{0} is required\n{USAGE}")]
  Missing(&'static str),
  #[error("unknown scenario {0}; the known scenarios are:\n{list}", list = scenario_names().join("\n"))]
  UnknownScenario(String),
  #[error("cannot seed {scenario}: {source}")]
  Seed {
    scenario: &'static str,
    #[source]
    source: SeedError,
  },
  /// Standard output went away.
  #[error("cannot write the seed's report: {0}")]
  Output(#[source] io::Error),
}

/// Every registered scenario's name, sorted.
pub fn scenario_names() -> Vec<&'static str> {
  let mut names: Vec<&'static str> = scenarios::ALL.iter().map(|scenario| scenario.name()).collect();
  names.sort_unstable();
  names
}

/// Does what `args` ask, writing the report or the list to `out`. An unknown scenario is refused before anything is
/// written.
pub fn run(args: impl Iterator<Item = String>, out: &mut impl Write) -> Result<(), CliError> {
  match parse(args)? {
    Command::ListScenarios => list(out).map_err(CliError::Output),
    Command::Seed(request) => {
      let seeded = kontur::seed(&request).map_err(|source| CliError::Seed {
        scenario: request.scenario.name(),
        source,
      })?;
      report(&seeded, out).map_err(CliError::Output)
    }
  }
}

/// The command `args` make, or what is wrong with them.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
  let (mut data_dir, mut repo, mut scenario, mut fake_codex) = (None, None, None, None);
  let (mut list, mut others) = (false, false);
  while let Some(flag) = args.next() {
    if flag == LIST_SCENARIOS {
      list = true;
      continue;
    }
    others = true;
    let value = args.next().ok_or_else(|| CliError::NoValue(flag.clone()))?;
    match flag.as_str() {
      "--data-dir" => data_dir = Some(PathBuf::from(value)),
      "--repo" => repo = Some(PathBuf::from(value)),
      "--scenario" => scenario = Some(value),
      "--fake-codex" => fake_codex = Some(PathBuf::from(value)),
      _ => return Err(CliError::UnknownArgument(flag)),
    }
  }
  match (list, others) {
    (true, true) => return Err(CliError::ListWithOthers),
    (true, false) => return Ok(Command::ListScenarios),
    (false, _) => {}
  }
  let name = scenario.ok_or(CliError::Missing("--scenario"))?;
  let scenario = scenarios::named(&name).ok_or(CliError::UnknownScenario(name))?;
  Ok(Command::Seed(SeedRequest {
    data_dir: data_dir.ok_or(CliError::Missing("--data-dir"))?,
    repo: repo.ok_or(CliError::Missing("--repo"))?,
    scenario,
    fake_codex,
    now: Timestamp::now(),
  }))
}

fn list(out: &mut impl Write) -> io::Result<()> {
  for name in scenario_names() {
    writeln!(out, "{name}")?;
  }
  Ok(())
}

fn report(seeded: &Seeded, out: &mut impl Write) -> io::Result<()> {
  writeln!(out, "project {}", seeded.project)?;
  writeln!(out, "repo {}", seeded.repo.display())?;
  writeln!(out, "app database {}", seeded.app_database.display())?;
  for task in &seeded.tasks {
    writeln!(out, "{:<7} {} · {}", task.key, task.title, task.state)?;
  }
  Ok(())
}
