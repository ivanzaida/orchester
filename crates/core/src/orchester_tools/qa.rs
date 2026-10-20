//! `start_qa_app`: a QA copy of the app the run's worktree builds, started by the runner from the worktree's
//! [`QA_APP_SCRIPT`] on a seeded scenario, off-screen and without focus as the script places it, and kept for the
//! step: `call_run_app` without a pid calls it, through the exact discovery file the script names. Each open QA app
//! holds one of the computer's QA slots, and a project's runs use as many as its *QA apps at once*
//! ([`crate::limits::QaSlots`]): a start while all of them are held waits until one app stops. The runner stops the
//! app when the agent's turn ends, and with it frees its slot.

use std::{num::NonZeroU32, path::PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::{resource::StepKey, run::RunNumber};

/// The script that builds and starts a QA copy of the app, relative to the worktree.
pub const QA_APP_SCRIPT: &str = "tools/qa-app.ps1";

/// The longest scenario name, as the script's scenarios are short kebab-case names.
const MAX_SCENARIO_CHARS: usize = 64;

/// A validated `start_qa_app` call: which seed scenario, and whether the copy runs its agents as stand-ins
/// (`-FakeAgents`) or has no runner (`-NoRunner`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QaAppStart {
  scenario: String,
  fake_agents: bool,
  no_runner: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QaAppInput {
  scenario: String,
  #[serde(default)]
  fake_agents: Option<bool>,
  #[serde(default)]
  no_runner: Option<bool>,
}

impl QaAppStart {
  pub(super) fn parse(arguments: Value) -> Result<Self, String> {
    let input: QaAppInput = serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    if !is_kebab(&input.scenario) || input.scenario.chars().count() > MAX_SCENARIO_CHARS {
      return Err(format!(
        "scenario must be a seed scenario's kebab-case name of at most {MAX_SCENARIO_CHARS} characters"
      ));
    }
    let (fake_agents, no_runner) = (input.fake_agents.unwrap_or(false), input.no_runner.unwrap_or(false));
    if fake_agents && no_runner {
      return Err("fake_agents runs agents through the runner; it cannot go with no_runner".to_owned());
    }
    Ok(Self {
      scenario: input.scenario,
      fake_agents,
      no_runner,
    })
  }

  pub fn scenario(&self) -> &str {
    &self.scenario
  }

  /// The script's arguments after its path, for the QA folder `name` ([`qa_name`]).
  pub fn script_args(&self, name: &str) -> Vec<String> {
    let mut args = vec![
      "-Name".to_owned(),
      name.to_owned(),
      "-Scenario".to_owned(),
      self.scenario.clone(),
    ];
    if self.fake_agents {
      args.push("-FakeAgents".to_owned());
    }
    if self.no_runner {
      args.push("-NoRunner".to_owned());
    }
    args
  }
}

/// What a `start_qa_app` call's approval names, as its target: the script's git blob (what an *Approve similar* rule
/// is kept to, so a rule never covers a call after the script changed), the scenario, and the files changed since the
/// run's base revision that run code while the app builds ([`QaApproval::watches`]), which the person should look at
/// first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QaApproval {
  pub script: String,
  pub scenario: String,
  pub changed: Vec<String>,
}

impl QaApproval {
  /// "tools/qa-app.ps1 <blob> · scenario <scenario>", then " · changed since the run's base: <files>" when any did.
  pub fn target(&self) -> String {
    let mut target = format!("{QA_APP_SCRIPT} {} · scenario {}", self.script, self.scenario);
    if !self.changed.is_empty() {
      target.push_str(" · changed since the run's base: ");
      target.push_str(&self.changed.join(", "));
    }
    target
  }

  /// The script's blob a [`QaApproval::target`] names; `None` for any other text.
  pub fn script_of(target: &str) -> Option<&str> {
    let rest = target.strip_prefix(QA_APP_SCRIPT)?.strip_prefix(' ')?;
    let blob = rest.split(' ').next()?;
    (!blob.is_empty() && blob.chars().all(|c| c.is_ascii_hexdigit())).then_some(blob)
  }

  /// Whether `path`, relative to the worktree, is one whose change the approval names, as it runs code while the app
  /// builds: the script, a build script, a manifest or lock file (a new dependency or proc macro, a `build = …`), a
  /// Cargo configuration (`rustc-wrapper`, runners, aliases) or a toolchain file.
  pub fn watches(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let folder = path.strip_suffix(name).unwrap_or_default();
    path == QA_APP_SCRIPT
      || matches!(
        name,
        "build.rs" | "Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
      )
      || (matches!(name, "config" | "config.toml") && (folder == ".cargo/" || folder.ends_with("/.cargo/")))
  }
}

/// The QA folder name (`.tmp/qa/<name>/`) of `step` in run `run`: `run-<number>-<step>`, in the script's kebab-case.
pub fn qa_name(run: RunNumber, step: &StepKey) -> String {
  let step: String = step
    .as_str()
    .chars()
    .map(|c| {
      if c.is_ascii_alphanumeric() {
        c.to_ascii_lowercase()
      } else {
        '-'
      }
    })
    .collect();
  format!("run-{}-{}", run.get(), step.trim_matches('-'))
}

/// What the script printed about the app it started: its `pid <n>` line and its `discovery <file>` line, the MCP
/// discovery file in the QA folder, which the runner checks lies there before it reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartedQaApp {
  pub pid: NonZeroU32,
  pub discovery: PathBuf,
}

impl StartedQaApp {
  /// The app the script's standard output names, its last `pid` and `discovery` lines.
  pub fn read(stdout: &str) -> Option<Self> {
    let value = |prefix: &str| {
      stdout
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(prefix))
        .map(str::trim)
    };
    let pid = value("pid ")?.parse().ok()?;
    let discovery = PathBuf::from(value("discovery ")?);
    Some(Self { pid, discovery })
  }
}

fn is_kebab(name: &str) -> bool {
  name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
    && name
      .chars()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
