//! The agent providers the `usage` scenarios split usage by (decided 2026-10-28): beside the catalogue's *Codex ·
//! Kontur* and *Claude Code · Kontur*, *Codex · Client* and *Claude · DeepSeek* on DeepSeek's Anthropic API with the
//! key in DEEPSEEK_API_KEY (a record only, no value anywhere) — four, so the fourth by name falls in Other providers —
//! and the connection each agent invocation started on: Codex's on Kontur and Client in turn, Claude Code's on Kontur
//! and DeepSeek in turn, every seventh unrecorded, as before Orchester recorded connections, which then reads as its
//! profile's. Every third review of auto mode's reviewer records no connection: those read as *Earlier runs*. The names
//! are illustrative.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, BaseUrl},
  id::{ConnectionId, SecretId},
  resource::ModelId,
  run::{InvocationKind, Run},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ActivityStore, ConnectionStore, SecretStore},
  usage::UsedProvider,
};

use crate::kontur::{Kontur, SeedError, catalogue};

/// Every seventh invocation records no connection.
const UNRECORDED_EVERY: usize = 7;
/// Every third review records no connection.
const EARLIER_REVIEW_EVERY: usize = 3;

/// The connections invocations start on, by CLI, in turn.
pub struct SeedProviders {
  codex: [UsedProvider; 2],
  claude: [UsedProvider; 2],
  /// Invocations and reviews given a connection or none so far: all invocations, Codex's, Claude Code's, reviews.
  turns: (usize, usize, usize, usize),
}

impl SeedProviders {
  /// Adds *Codex · Client* and *Claude · DeepSeek* beside the catalogue's connections.
  pub fn add(kontur: &mut Kontur) -> Result<Self, SeedError> {
    let at = kontur.ago(14 * 24 * 60);
    let programs = kontur.repo.join(".orchester").join("recorded");
    let known = |kontur: &Kontur, title: &str| -> Result<UsedProvider, SeedError> {
      let connection = kontur
        .db
        .connections()?
        .into_iter()
        .find(|connection| connection.name().as_str() == title)
        .ok_or_else(|| SeedError::Invalid(format!("no connection {title}")))?;
      Ok(used(connection.id(), title))
    };
    let (codex, claude) = (known(kontur, "Codex · Kontur")?, known(kontur, "Claude Code · Kontur")?);
    let client = catalogue::recorded_connection(
      &mut kontur.db,
      ("Codex · Client", AgentProvider::CodexCli, &programs.join("codex")),
      at,
    )?;
    let deepseek = catalogue::recorded_connection(
      &mut kontur.db,
      (
        "Claude · DeepSeek",
        AgentProvider::ClaudeCodeCli,
        &programs.join("claude"),
      ),
      at,
    )?;
    on_deepseek(kontur, deepseek)?;
    Ok(Self {
      codex: [codex, used(client, "Codex · Client")],
      claude: [claude, used(deepseek, "Claude · DeepSeek")],
      turns: (0, 0, 0, 0),
    })
  }

  /// Records which connection each of `run`'s agent invocations started on, in turn.
  pub fn record(&mut self, kontur: &mut Kontur, run: &Run) -> Result<(), SeedError> {
    for invocation in kontur.db.invocations(run.id())? {
      let InvocationKind::Agent { model } = &invocation.kind else {
        continue;
      };
      let (all, codex, claude, _) = &mut self.turns;
      *all += 1;
      if all.is_multiple_of(UNRECORDED_EVERY) {
        continue;
      }
      let connection = if model.as_str().starts_with("claude") {
        *claude += 1;
        &self.claude[*claude % self.claude.len()]
      } else {
        *codex += 1;
        &self.codex[*codex % self.codex.len()]
      };
      kontur.db.save_invocation_connection(invocation.id, connection)?;
    }
    Ok(())
  }

  /// The connection the next review of auto mode's reviewer, on Claude Code · Kontur, records: none every third time.
  pub fn reviewer(&mut self) -> Option<UsedProvider> {
    let reviews = &mut self.turns.3;
    *reviews += 1;
    (!reviews.is_multiple_of(EARLIER_REVIEW_EVERY)).then(|| self.claude[0].clone())
  }
}

fn used(id: ConnectionId, name: &str) -> UsedProvider {
  UsedProvider { id, name: name.into() }
}

/// Puts connection `id` on DeepSeek's Anthropic API, with the key in a DEEPSEEK_API_KEY secret that holds no value.
fn on_deepseek(kontur: &mut Kontur, id: ConnectionId) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let new = NewSecret {
    name: SecretName::new("DEEPSEEK_API_KEY").map_err(|error| invalid(&error))?,
    kind: SecretKind::ApiKey,
    environment: Environment::Development,
  };
  let secret = Secret::from_stored(SecretId::generate(), new, kontur.ago(14 * 24 * 60), None, None);
  kontur.db.create_secret(&secret)?;
  let stored = kontur
    .db
    .connection(id)?
    .ok_or_else(|| SeedError::Invalid("connection".to_owned()))?;
  let mut connection = AgentConnection::try_from(stored).map_err(|error| invalid(&error))?;
  let provider = ApiProvider::new(
    BaseUrl::new("https://api.deepseek.com/anthropic").map_err(|error| invalid(&error))?,
    secret.id(),
    vec![ModelId::new("deepseek-v4-pro").map_err(|error| invalid(&error))?],
  )
  .map_err(|error| invalid(&error))?;
  connection
    .set_api_provider(Some(provider))
    .map_err(|error| invalid(&error))?;
  kontur.db.save_connection(&connection.into_connection())?;
  Ok(())
}
