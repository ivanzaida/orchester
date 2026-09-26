//! Choosing among several installed copies of a CLI. A computer can have more than one on the
//! `PATH` (on Windows the Codex desktop app's `codex.exe` and npm's `codex.cmd`, with different
//! versions); the newest one is the default, and the user can still pick another file.

use std::{cmp::Ordering, path::PathBuf};

use super::{AgentProvider, adapter::parse_version};
use crate::command::{Command, CommandRunner};

/// A copy of a CLI and the version its `--version` reported (`None`: it did not answer as the
/// expected CLI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
  pub executable: PathBuf,
  pub version: Option<String>,
}

/// Asks each of `candidates` for its version (read-only: `--version`) and orders them newest
/// first. Copies whose version cannot be read come last, in their original order.
pub fn rank_installed(
  provider: AgentProvider,
  candidates: Vec<PathBuf>,
  runner: &impl CommandRunner,
) -> Vec<Installed> {
  let mut installed: Vec<Installed> = candidates
    .into_iter()
    .map(|executable| {
      let version = runner
        .run(&Command::new(&executable, ["--version"]))
        .ok()
        .and_then(|output| parse_version(provider, &output));
      Installed { executable, version }
    })
    .collect();
  // A stable sort keeps the `PATH` order between copies of the same version.
  installed.sort_by(|a, b| compare_versions(b.version.as_deref(), a.version.as_deref()));
  installed
}

/// Orders versions like `0.139.0` numerically, part by part; a missing version is the oldest.
/// Text after the numbers of a part (`0.1.0-beta`) is ignored.
pub fn compare_versions(a: Option<&str>, b: Option<&str>) -> Ordering {
  match (a, b) {
    (None, None) => Ordering::Equal,
    (None, Some(_)) => Ordering::Less,
    (Some(_), None) => Ordering::Greater,
    (Some(a), Some(b)) => parts(a).cmp(&parts(b)),
  }
}

fn parts(version: &str) -> Vec<u64> {
  version
    .split('.')
    .map(|part| {
      let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
      // A part without digits orders as 0, like a missing part.
      digits.parse().unwrap_or(0)
    })
    .collect()
}
