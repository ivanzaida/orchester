//! What Orchester asks each CLI and how it reads the answers. Only read-only commands taken
//! from the CLIs' own `--help` are used: none starts a session, sends a prompt, or changes the
//! CLI's configuration or login.
//!
//! - Codex CLI: `codex --version`, `codex login status` ("Show login status") and `codex debug models --bundled`
//!   ("Render the raw model catalog as JSON", `--bundled`: "Skip refresh and dump only the bundled catalog shipped with
//!   this binary", so no network request and no cache write).
//! - Claude Code CLI: `claude --version`, `claude auth status --json` ("Show authentication status") and the
//!   `initialize` control request of its stream-json input, whose answer lists its models; that one starts no session
//!   and sends no prompt ([`super::claude_catalog`]).
//!
//! A native provider has no CLI: nothing here runs for it, and [`check`] has no answer for it.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use serde::Deserialize;

use super::{
  AgentProvider, CheckResult, ConnectionCheck, SignIn,
  catalog::{CatalogModel, CatalogSource, ModelCatalog},
  claude_catalog::{MODELS_ARGS, MODELS_REQUEST, claude_models, parse_claude_models},
};
use crate::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  resource::ModelId,
  time::Timestamp,
};

const VERSION_ARGS: &[&str] = &["--version"];
const CODEX_SIGN_IN_ARGS: &[&str] = &["login", "status"];
const CODEX_CATALOG_ARGS: &[&str] = &["debug", "models", "--bundled"];
const CLAUDE_SIGN_IN_ARGS: &[&str] = &["auth", "status", "--json"];
/// Codex lists models meant for people with this visibility; others are internal.
const CODEX_LISTED: &str = "list";

/// The agent CLIs this module asks.
#[derive(Clone, Copy)]
enum AgentCli {
  Codex,
  Claude,
}

impl AgentCli {
  const fn of(provider: AgentProvider) -> Option<Self> {
    match provider {
      AgentProvider::CodexCli => Some(Self::Codex),
      AgentProvider::ClaudeCodeCli => Some(Self::Claude),
      AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => None,
    }
  }
}

/// Checks the CLI at `executable`: its version, whether it is signed in and its models. The CLI
/// sees only `environment`, the one agents run with ([`super::invocation::agent_environment`]),
/// so that the check finds what a run would: variables of an agent host Orchester itself runs in
/// change the Claude Code CLI's answer. `None` for a native provider: it has no CLI, so nothing runs
/// and there is nothing to report.
pub fn check(
  provider: AgentProvider,
  executable: &Path,
  runner: &impl CommandRunner,
  environment: &[(String, String)],
  now: Timestamp,
) -> Option<ConnectionCheck> {
  let kind = AgentCli::of(provider)?;
  let cli = Cli {
    executable,
    runner,
    environment,
  };
  let result = match cli.run(VERSION_ARGS) {
    Err(CommandError::NotFound) => CheckResult::ExecutableMissing,
    Err(
      CommandError::TimedOut
      | CommandError::Failed(_)
      | CommandError::NotStarted(_)
      | CommandError::UnsafeScriptArgument { .. },
    ) => CheckResult::CouldNotRun,
    Ok(output) => match parse_version(provider, &output) {
      None => CheckResult::NotRecognized,
      Some(version) => CheckResult::Available {
        version,
        sign_in: sign_in(kind, &cli),
        catalog: catalog(kind, &cli),
      },
    },
  };
  Some(ConnectionCheck { at: now, result })
}

/// The version number in `--version` output, if the output is the expected CLI's:
/// `codex-cli 0.139.0` or `2.1.89 (Claude Code)`. No output is a native provider's.
pub fn parse_version(provider: AgentProvider, output: &CommandOutput) -> Option<String> {
  let marker = match AgentCli::of(provider)? {
    AgentCli::Codex => "codex-cli",
    AgentCli::Claude => "(Claude Code)",
  };
  let text = output.stdout.trim();
  if !output.success() || !text.contains(marker) {
    return None;
  }
  text
    .split_whitespace()
    .find(|token| token.starts_with(|c: char| c.is_ascii_digit()))
    .map(str::to_owned)
}

/// One CLI and the environment it runs with.
struct Cli<'a, R> {
  executable: &'a Path,
  runner: &'a R,
  environment: &'a [(String, String)],
}

impl<R: CommandRunner> Cli<'_, R> {
  fn run(&self, args: &[&str]) -> Result<CommandOutput, CommandError> {
    self.runner.run(&self.command(args))
  }

  fn command(&self, args: &[&str]) -> Command {
    Command::new(self.executable, args.iter().copied()).with_environment(self.environment.to_vec())
  }
}

fn sign_in(kind: AgentCli, cli: &Cli<'_, impl CommandRunner>) -> SignIn {
  let args = match kind {
    AgentCli::Codex => CODEX_SIGN_IN_ARGS,
    AgentCli::Claude => CLAUDE_SIGN_IN_ARGS,
  };
  // A status command that cannot run leaves the sign-in unknown, not signed out.
  let Ok(output) = cli.run(args) else {
    return SignIn::Unknown;
  };
  match kind {
    AgentCli::Codex => parse_codex_login_status(&output),
    AgentCli::Claude => parse_claude_auth_status(&output),
  }
}

/// `codex login status` prints `Logged in using …` or `Not logged in`, on either stream.
pub fn parse_codex_login_status(output: &CommandOutput) -> SignIn {
  let text = format!("{}\n{}", output.stdout, output.stderr);
  if text.contains("Not logged in") {
    SignIn::SignedOut
  } else if output.success() && text.lines().any(|line| line.trim_start().starts_with("Logged in")) {
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

fn catalog(kind: AgentCli, cli: &Cli<'_, impl CommandRunner>) -> ModelCatalog {
  match kind {
    AgentCli::Codex => {
      // Without a readable catalog the list stays empty: models can still be entered by hand
      // and are marked as not verified.
      let models = cli
        .run(CODEX_CATALOG_ARGS)
        .ok()
        .filter(|output| output.success())
        .and_then(|output| parse_codex_catalog(&output.stdout))
        .unwrap_or_default();
      ModelCatalog::new(CatalogSource::Provider, models)
    }
    AgentCli::Claude => {
      // Without the CLI's list the catalog still has the current model ids and the aliases.
      let listed = cli
        .runner
        .run(&cli.command(MODELS_ARGS).with_stdin(format!("{MODELS_REQUEST}\n")))
        .ok()
        .filter(|output| output.success())
        .and_then(|output| parse_claude_models(&output.stdout));
      let source = if listed.is_some() {
        CatalogSource::ProviderAndCurrentIds
      } else {
        CatalogSource::DocumentedAliases
      };
      ModelCatalog::new(source, claude_models(listed))
    }
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

/// Where `program` (see [`super::ConnectionKind::program_name`]) would be found on `path` (the
/// `PATH` variable), in search order. The caller keeps the ones that exist.
pub fn executable_candidates(program: &str, path: Option<&OsStr>) -> Vec<PathBuf> {
  let Some(path) = path else {
    return Vec::new();
  };
  let names = program_files(program);
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
