//! `native`: native Anthropic API connections as the native frames show them (`docs/design/native-connections.md`,
//! Row 8 `OyEXT` of `XGtkg`): Anthropic API · Kontur, tested and connected, with claude-opus-5-5 (the default, high)
//! and claude-sonnet-5-5 listed by the endpoint, which Builder and Reviewer run on, and claude-review-preview added by
//! hand (05e–05j); Anthropic API · Old key, whose last test found its key rejected (05g's sentence); Anthropic API ·
//! Archived, whose key secret was archived (key missing, 05k); and Anthropic API · New, never tested (not checked,
//! 05k). The Codex and Claude Code CLIs are there as in `connections`, with Explorer and Planner on Codex. One task's
//! run waits on Implement, held for the person, to approve a native agent's command run through pwsh with a 120-second
//! limit (15 `DvSDJ`).
//!
//! The run is recorded, not executed, as in `cards`: a runner that attaches ends it, so the card waits only in a copy
//! started without a runner. The key secrets are records only: no value is in any keychain, and nothing here was tested
//! for real, so *Test connection* says what the runner could not do.

use std::{fs, num::NonZeroU32, path::PathBuf};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CheckResult, ConnectionCheck, Effort, EndpointPreset,
    ModelSource, NativeModel, NativeModels, SignIn,
  },
  id::{ConnectionId, SecretId},
  integration::WORKTREES_DIR,
  resource::ModelId,
  run::{CommandShell, NativeCommandAsk, PendingToolApproval},
  store::ConnectionStore,
  time::Timestamp,
};

use super::{api_providers::key_secret, cards, connections};
use crate::kontur::{Kontur, SeedError, name};

/// The default model, Builder's.
const OPUS: &str = "claude-opus-5-5";
const SONNET: &str = "claude-sonnet-5-5";
/// The model added by hand.
const BY_HAND: &str = "claude-review-preview";
const URL: &str = "https://api.anthropic.com";
/// The command the native agent asks to run (15 `DvSDJ`).
const COMMAND: &str = "npm install zod --save-exact";
const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let at = kontur.ago(4);
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  fs::create_dir_all(&bin).map_err(invalid)?;
  let program = |file: &str| -> Result<PathBuf, SeedError> {
    let path = bin.join(file);
    fs::write(&path, "").map_err(invalid)?;
    Ok(path)
  };
  let codex = connections::cli(
    kontur,
    ("Codex · Kontur", AgentProvider::CodexCli, program("codex")?),
    ("0.46.0", SignIn::SignedIn, &["gpt-5.5", "gpt-5.4-mini"]),
    at,
  )?;
  connections::cli(
    kontur,
    ("Claude · Kontur", AgentProvider::ClaudeCodeCli, program("claude")?),
    ("2.1.89", SignIn::SignedIn, &[OPUS, SONNET]),
    at,
  )?;
  let key = key_secret(kontur, "ANTHROPIC_API_KEY", None, kontur.ago(60))?;
  let listed = kontur.ago(2);
  let kontur_api = native(
    kontur,
    "Anthropic API · Kontur",
    (key, models(listed)?),
    Some(CheckResult::Api { balance: None }),
  )?;
  let old = key_secret(kontur, "OLD_ANTHROPIC_KEY", None, kontur.ago(90))?;
  native(
    kontur,
    "Anthropic API · Old key",
    (old, models(listed)?),
    Some(CheckResult::KeyRejected),
  )?;
  let archived = key_secret(kontur, "RETIRED_ANTHROPIC_KEY", Some(kontur.ago(30)), kontur.ago(120))?;
  native(
    kontur,
    "Anthropic API · Archived",
    (archived, NativeModels::default()),
    None,
  )?;
  native(kontur, "Anthropic API · New", (key, NativeModels::default()), None)?;
  let published = kontur.ago(7 * 24 * 60);
  let catalogue = crate::kontur::catalogue::native(&mut kontur.db, (codex, kontur_api), OPUS, published)?;
  waiting_command(kontur, &catalogue.feature_delivery)
}

/// The two models the endpoint listed at `at`, claude-opus-5-5 the default at high, and one added by hand (05e).
fn models(at: Timestamp) -> Result<NativeModels, SeedError> {
  let limit = |tokens: u32| NonZeroU32::new(tokens);
  let listed = |id: &str, output: u32, efforts: &[Effort]| -> Result<NativeModel, SeedError> {
    Ok(NativeModel::new(
      ModelId::new(id).map_err(invalid)?,
      limit(1_000_000),
      limit(output),
      efforts.to_vec(),
      ModelSource::Endpoint { at },
    ))
  };
  let opus = listed(OPUS, 128_000, &[Effort::Low, Effort::Medium, Effort::High, Effort::Max])?;
  let sonnet = listed(SONNET, 64_000, &[Effort::Low, Effort::Medium, Effort::High])?;
  let manual = NativeModel::new(
    ModelId::new(BY_HAND).map_err(invalid)?,
    None,
    None,
    Vec::new(),
    ModelSource::Manual,
  );
  let default = opus.id().clone();
  NativeModels::new(vec![opus, sonnet, manual], Some(default), Some(Effort::High)).map_err(invalid)
}

/// A native Anthropic API connection named `named` on the official API with the key in `key` and `models`, its last
/// test having found `checked`; its id.
fn native(
  kontur: &mut Kontur,
  named: &str,
  (key, models): (SecretId, NativeModels),
  checked: Option<CheckResult>,
) -> Result<ConnectionId, SeedError> {
  let url = BaseUrl::new(URL).map_err(invalid)?;
  let api = ApiEndpoint::new(EndpointPreset::Official, url, key).with_models(models);
  let at = kontur.ago(3);
  let mut connection = AgentConnection::native(
    ConnectionId::generate(),
    name(named),
    AgentProvider::AnthropicApi,
    api,
    kontur.ago(5),
  )?;
  if let Some(result) = checked {
    connection.record_check(ConnectionCheck { at, result })?;
  }
  let connection = connection.into_connection();
  kontur.db.create_connection(&connection)?;
  Ok(connection.id())
}

/// A task whose run waits on Implement, held for the person, for Builder's command, run through pwsh in the run's
/// worktree and stopped after 120 seconds, recorded as the runner records a native agent's ask: the working folder as
/// its target, no note.
fn waiting_command(kontur: &mut Kontur, pipeline: &crate::kontur::catalogue::Published) -> Result<(), SeedError> {
  let title = "Retry failed webhook deliveries";
  let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), kontur.ago(50))?;
  let (run, invocation) = cards::at_work(kontur, &mut task, pipeline, kontur.ago(30))?;
  kontur.seeded(&task, "native command waiting for approval");
  let worktree = kontur.repo.join(WORKTREES_DIR).join(run.id().to_string());
  let request = PendingToolApproval {
    id: format!("{}:call_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: COMMAND.to_owned(),
    target: Some(worktree.to_string_lossy().into_owned()),
    requested_at: Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 2 * 60_000),
    native: Some(NativeCommandAsk {
      shell: Some(CommandShell::Pwsh),
      time_limit_seconds: 120,
    }),
  };
  kontur.db.request_tool_approval(&request)?;
  cards::hold(kontur, run, (&invocation, true))
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
