//! `api-providers`: the `connections` scenario with agent CLI connections on API providers
//! (`docs/design/api-providers.md`, frames `y2qhJ`, `tnEKp` in `mKKPM`): Claude · DeepSeek and Codex · DeepSeek checked
//! and running on DeepSeek's APIs with the key in DEEPSEEK_API_KEY, Claude · Gateway on a gateway on this computer, not
//! checked yet, and Claude · Old gateway whose key secret was archived (Key missing); and Researcher, a profile draft
//! on Codex · DeepSeek. The key secrets are records only: no value is in any keychain, so a run on them does not start,
//! which the run says.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, BaseUrl, SignIn},
  id::{ConnectionId, SecretId},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, ResourceStore, SecretStore},
  time::Timestamp,
};

use super::connections;
use crate::kontur::{Kontur, SeedError, catalogue::Catalogue, name};

const MODELS: [&str; 2] = ["deepseek-v4-flash", "deepseek-v4-pro"];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed_catalogue(kontur).map(drop)
}

/// [`seed`], giving the catalogue of the `connections` scenario it builds on.
pub(super) fn seed_catalogue(kontur: &mut Kontur) -> Result<Catalogue, SeedError> {
  let catalogue = connections::seed_catalogue(kontur)?;
  let at = kontur.ago(3);
  let key = key_secret(kontur, "DEEPSEEK_API_KEY", None, at)?;
  let old = key_secret(kontur, "OLD_GATEWAY_KEY", Some(at), kontur.ago(30))?;
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  let program = |file: &str| bin.join(file);
  let claude = connections::cli(
    kontur,
    ("Claude · DeepSeek", AgentProvider::ClaudeCodeCli, program("claude")),
    ("2.1.89", SignIn::SignedOut, &[]),
    at,
  )?;
  on_provider(kontur, claude, ("https://api.deepseek.com/anthropic", key, &MODELS))?;
  let codex = connections::cli(
    kontur,
    ("Codex · DeepSeek", AgentProvider::CodexCli, program("codex")),
    ("0.46.0", SignIn::SignedIn, &[]),
    at,
  )?;
  on_provider(kontur, codex, ("https://api.deepseek.com", key, &MODELS[1..]))?;
  let old_gateway = connections::cli(
    kontur,
    ("Claude · Old gateway", AgentProvider::ClaudeCodeCli, program("claude")),
    ("2.1.89", SignIn::SignedIn, &[]),
    at,
  )?;
  on_provider(kontur, old_gateway, ("https://gateway.example.com", old, &MODELS[..1]))?;
  gateway(kontur, program("claude"), key, at)?;
  researcher(kontur, codex, at)?;
  Ok(catalogue)
}

/// A secret of kind API key in Development, archived at `archived` when given; a record only.
fn key_secret(
  kontur: &mut Kontur,
  named: &str,
  archived: Option<Timestamp>,
  at: Timestamp,
) -> Result<SecretId, SeedError> {
  let new = NewSecret {
    name: SecretName::new(named).map_err(|error| SeedError::Invalid(error.to_string()))?,
    kind: SecretKind::ApiKey,
    environment: Environment::Development,
  };
  let secret = Secret::from_stored(SecretId::generate(), new, at, None, archived);
  kontur.db.create_secret(&secret)?;
  Ok(secret.id())
}

/// Puts stored connection `id` on the provider at `url` with the key in `key` and `models`.
fn on_provider(
  kontur: &mut Kontur,
  id: ConnectionId,
  (url, key, models): (&str, SecretId, &[&str]),
) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let stored = kontur
    .db
    .connection(id)?
    .ok_or_else(|| SeedError::Invalid("connection".to_owned()))?;
  let mut connection = AgentConnection::try_from(stored).map_err(|error| invalid(&error))?;
  let models = models
    .iter()
    .map(|model| ModelId::new(model).map_err(|error| invalid(&error)))
    .collect::<Result<Vec<_>, _>>()?;
  let url = BaseUrl::new(url).map_err(|error| invalid(&error))?;
  let provider = ApiProvider::new(url, key, models).map_err(|error| invalid(&error))?;
  connection
    .set_api_provider(Some(provider))
    .map_err(|error| invalid(&error))?;
  kontur.db.save_connection(&connection.into_connection())?;
  Ok(())
}

/// Claude · Gateway: a gateway on this computer, never checked.
fn gateway(kontur: &mut Kontur, program: std::path::PathBuf, key: SecretId, at: Timestamp) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Claude · Gateway"),
    AgentProvider::ClaudeCodeCli,
    program,
    at,
  )
  .map_err(|error| invalid(&error))?;
  let provider = ApiProvider::new(
    BaseUrl::new("http://localhost:4000").map_err(|error| invalid(&error))?,
    key,
    vec![ModelId::new("gateway-model").map_err(|error| invalid(&error))?],
  )
  .map_err(|error| invalid(&error))?;
  connection
    .set_api_provider(Some(provider))
    .map_err(|error| invalid(&error))?;
  kontur.db.create_connection(&connection.into_connection())?;
  Ok(())
}

/// Researcher, a profile draft on Codex · DeepSeek.
fn researcher(kontur: &mut Kontur, codex: ConnectionId, at: Timestamp) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let body = AgentProfile::new(
    name("Researcher"),
    String::new(),
    PromptText::new("Research the question.").map_err(|error| invalid(&error))?,
    codex,
    ModelId::new(MODELS[1]).map_err(|error| invalid(&error))?,
    ProviderSettings::default(),
  );
  let slug = Slug::new("researcher").map_err(|error| invalid(&error))?;
  kontur.db.create_resource(slug, name("Researcher"), &body, at)?;
  Ok(())
}
