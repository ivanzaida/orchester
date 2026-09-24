//! What Orchester asks each CLI and how it reads the answers. Only read-only commands taken
//! from the CLIs' own `--help` are used: none starts a session, sends a prompt, or changes the
//! CLI's configuration or login.
//!
//! - Codex CLI: `codex --version`, `codex login status` ("Show login status") and
//!   `codex debug models --bundled` ("Render the raw model catalog as JSON", `--bundled`:
//!   "Skip refresh and dump only the bundled catalog shipped with this binary", so no network
//!   request and no cache write).
//! - Claude Code CLI: `claude --version` and `claude auth status --json` ("Show
//!   authentication status"). It has no model list; `claude --help` documents the `opus` and
//!   `sonnet` aliases for `--model` and the effort levels of `--effort`.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

use super::{
  AgentProvider, CheckResult, ConnectionCheck, SignIn,
  catalog::{CatalogModel, CatalogSource, ModelCatalog},
};
use crate::{resource::ModelId, time::Timestamp};

const VERSION_ARGS: &[&str] = &["--version"];
const CODEX_SIGN_IN_ARGS: &[&str] = &["login", "status"];
const CODEX_CATALOG_ARGS: &[&str] = &["debug", "models", "--bundled"];
const CLAUDE_SIGN_IN_ARGS: &[&str] = &["auth", "status", "--json"];
/// Model aliases documented by `claude --help` for `--model`.
const CLAUDE_MODEL_ALIASES: [&str; 2] = ["opus", "sonnet"];
/// Levels documented by `claude --help` for `--effort`.
const CLAUDE_EFFORTS: [&str; 4] = ["low", "medium", "high", "max"];
/// Codex lists models meant for people with this visibility; others are internal.
const CODEX_LISTED: &str = "list";

/// Output of a finished command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
  /// The command exited with status 0.
  pub success: bool,
  pub stdout: String,
  pub stderr: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandError {
  #[error("the program does not exist")]
  NotFound,
  #[error("the program did not finish in time")]
  TimedOut,
  #[error("the program could not be run: {0}")]
  Failed(String),
}

/// Runs a program to completion. The process boundary: the desktop runs real programs, tests
/// replay captured output.
pub trait CommandRunner {
  fn run(&self, program: &Path, args: &[&str]) -> Result<CommandOutput, CommandError>;
}

/// Checks the CLI at `executable`: its version, whether it is signed in and its models.
pub fn check(provider: AgentProvider, executable: &Path, runner: &impl CommandRunner, now: Timestamp) -> ConnectionCheck {
  let result = match runner.run(executable, VERSION_ARGS) {
    Err(CommandError::NotFound) => CheckResult::ExecutableMissing,
    Err(CommandError::TimedOut | CommandError::Failed(_)) => CheckResult::CouldNotRun,
    Ok(output) => match parse_version(provider, &output) {
      None => CheckResult::NotRecognized,
      Some(version) => CheckResult::Available {
        version,
        sign_in: sign_in(provider, executable, runner),
        catalog: catalog(provider, executable, runner),
      },
    },
  };
  ConnectionCheck { at: now, result }
}

/// The version number in `--version` output, if the output is the expected CLI's:
/// `codex-cli 0.139.0` or `2.1.89 (Claude Code)`.
pub fn parse_version(provider: AgentProvider, output: &CommandOutput) -> Option<String> {
  let marker = match provider {
    AgentProvider::CodexCli => "codex-cli",
    AgentProvider::ClaudeCodeCli => "(Claude Code)",
  };
  let text = output.stdout.trim();
  if !output.success || !text.contains(marker) {
    return None;
  }
  text
    .split_whitespace()
    .find(|token| token.starts_with(|c: char| c.is_ascii_digit()))
    .map(str::to_owned)
}

fn sign_in(provider: AgentProvider, executable: &Path, runner: &impl CommandRunner) -> SignIn {
  let args = match provider {
    AgentProvider::CodexCli => CODEX_SIGN_IN_ARGS,
    AgentProvider::ClaudeCodeCli => CLAUDE_SIGN_IN_ARGS,
  };
  // A status command that cannot run leaves the sign-in unknown, not signed out.
  let Ok(output) = runner.run(executable, args) else {
    return SignIn::Unknown;
  };
  match provider {
    AgentProvider::CodexCli => parse_codex_login_status(&output),
    AgentProvider::ClaudeCodeCli => parse_claude_auth_status(&output),
  }
}

/// `codex login status` prints `Logged in using …` or `Not logged in`, on either stream.
pub fn parse_codex_login_status(output: &CommandOutput) -> SignIn {
  let text = format!("{}\n{}", output.stdout, output.stderr);
  if text.contains("Not logged in") {
    SignIn::SignedOut
  } else if output.success && text.lines().any(|line| line.trim_start().starts_with("Logged in")) {
    SignIn::SignedIn
  } else {
    SignIn::Unknown
  }
}

/// `claude auth status --json` prints an object with `loggedIn`; it exits with 1 when signed
/// out, so the exit status alone says nothing.
pub fn parse_claude_auth_status(output: &CommandOutput) -> SignIn {
  #[derive(Deserialize)]
  struct Status {
    #[serde(rename = "loggedIn")]
    logged_in: bool,
  }
  match serde_json::from_str::<Status>(output.stdout.trim()) {
    Ok(Status { logged_in: true }) => SignIn::SignedIn,
    Ok(Status { logged_in: false }) => SignIn::SignedOut,
    Err(_) => SignIn::Unknown,
  }
}

fn catalog(provider: AgentProvider, executable: &Path, runner: &impl CommandRunner) -> ModelCatalog {
  match provider {
    AgentProvider::CodexCli => {
      // Without a readable catalog the list stays empty: models can still be entered by hand
      // and are marked as not verified.
      let models = runner
        .run(executable, CODEX_CATALOG_ARGS)
        .ok()
        .filter(|output| output.success)
        .and_then(|output| parse_codex_catalog(&output.stdout))
        .unwrap_or_default();
      ModelCatalog::new(CatalogSource::Provider, models)
    }
    AgentProvider::ClaudeCodeCli => ModelCatalog::new(CatalogSource::DocumentedAliases, claude_aliases()),
  }
}

/// Listed models of `codex debug models` output, in catalog order.
pub fn parse_codex_catalog(json: &str) -> Option<Vec<CatalogModel>> {
  #[derive(Deserialize)]
  struct Catalog {
    models: Vec<Model>,
  }
  #[derive(Deserialize)]
  struct Model {
    slug: String,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    supported_reasoning_levels: Vec<Level>,
  }
  #[derive(Deserialize)]
  struct Level {
    effort: String,
  }
  let catalog: Catalog = serde_json::from_str(json).ok()?;
  let models = catalog
    .models
    .into_iter()
    .filter(|model| model.visibility.as_deref() == Some(CODEX_LISTED))
    .filter_map(|model| {
      let id = ModelId::new(&model.slug).ok()?;
      let efforts = model
        .supported_reasoning_levels
        .into_iter()
        .map(|level| level.effort)
        .collect();
      Some(CatalogModel::new(id, efforts))
    })
    .collect();
  Some(models)
}

fn claude_aliases() -> Vec<CatalogModel> {
  CLAUDE_MODEL_ALIASES
    .into_iter()
    .map(|alias| {
      let id = ModelId::new(alias).expect("documented aliases are valid model ids");
      CatalogModel::new(id, CLAUDE_EFFORTS.map(str::to_owned).to_vec())
    })
    .collect()
}

/// Where the provider's program would be found on `path` (the `PATH` variable), in search
/// order. The caller keeps the ones that exist.
pub fn executable_candidates(provider: AgentProvider, path: Option<&OsStr>) -> Vec<PathBuf> {
  let Some(path) = path else {
    return Vec::new();
  };
  let names = program_files(provider.program_name());
  std::env::split_paths(path)
    .filter(|directory| !directory.as_os_str().is_empty())
    .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
    .collect()
}

/// npm installs the CLIs on Windows as `.cmd` shims next to a shell script; native installers
/// put an `.exe` on the `PATH`.
#[cfg(windows)]
fn program_files(name: &str) -> Vec<String> {
  vec![format!("{name}.exe"), format!("{name}.cmd")]
}

#[cfg(not(windows))]
fn program_files(name: &str) -> Vec<String> {
  vec![name.to_owned()]
}
