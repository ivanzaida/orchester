//! Agent CLI connections on API providers (`docs/design/api-providers.md`): the provider `add_cli_connection` takes,
//! `set_api_provider`, and how a connection's provider is listed. A provider names an existing project secret for its
//! key; no tool takes or returns the key's value. Running a connection on a provider sends the project's prompts and
//! code to its host under its key, so a new host or key is refused as `person_only` unless a person lets automation
//! widen; going back to the CLI's own sign-in, or changing only the models, is not a widening.

use orchester_core::{
  connection::ApiProvider,
  resource::ModelId,
  secret::Secret,
  store::SecretStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  connections::{agent_json, agent_named},
  widening,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  connections::{
    api_provider::{base_url, model_ids, provider_of, set_provider_in},
    data::{ProjectConnections, SaveError},
  },
  messages::Message,
  tasks::outside::Outside,
};

/// The model API an agent CLI connection runs on with an API key instead of the CLI's own sign-in.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ApiProviderInput {
  /// The API's base URL: https://, or http:// for localhost, 127.0.0.1 or [::1]; no user name, password, query or
  /// fragment. Claude Code needs an Anthropic Messages API (DeepSeek: https://api.deepseek.com/anthropic), Codex an
  /// OpenAI Responses API (DeepSeek: https://api.deepseek.com).
  pub base_url: String,
  /// The project secret holding the API key: its id, or its name when only one secret that is not archived has it.
  pub key_secret: String,
  /// The model ids the provider serves, at least one; profiles on the connection choose from them.
  pub models: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetApiProvider {
  /// The project's id, or its name.
  pub project: String,
  /// The connection's id, or its name: a Codex or Claude Code CLI connection.
  pub connection: String,
  /// The provider to run on; null runs the connection on its CLI's own sign-in again.
  pub api_provider: Option<ApiProviderInput>,
}

pub async fn set_api_provider(services: &Services, args: SetApiProvider) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let connection = agent_named(&db, &args.connection)?;
    let provider = match &args.api_provider {
      Some(input) => Some(provider_from(&db, input)?),
      None => None,
    };
    if let Some(provider) = &provider {
      let reasons = widens(&db, connection.name().as_str(), connection.api_provider(), provider)?;
      widening::allowed(&services, reasons)?;
    }
    drop(db);
    set_provider_in(&open.repo_root, connection.id(), provider).map_err(|error| saved(&error, &args.connection))?;
    services.changes.record(open.project.id(), Outside::Connections);
    let listed = ProjectConnections::load_in(&open.repo_root)?;
    let connection = listed
      .connection(connection.id())
      .ok_or_else(|| ToolError::not_found(crate::app_mcp::error::Missing::Connection, &args.connection))?;
    Ok(json!({ "connection": agent_json(connection, &listed.secrets, open.project.id()) }))
  })
  .await
}

/// The provider `input` describes in the project of `db`, or why it cannot be one.
pub(super) fn provider_from(db: &ProjectDb, input: &ApiProviderInput) -> Result<ApiProvider, ToolError> {
  let base_url = base_url(&input.base_url).map_err(|reason| ToolError::refused("invalid_base_url", vec![reason]))?;
  let models = model_ids(&input.models.join(",")).map_err(|reason| ToolError::refused("invalid_model", vec![reason]))?;
  let key = secret_named(db, &input.key_secret)?;
  provider_of(base_url, key.id(), models).map_err(|reason| ToolError::refused("invalid_api_provider", vec![reason]))
}

/// What running connection `name`, on `current` now, on `provider` widens: a host or key it does not run on now.
pub(super) fn widens(
  db: &ProjectDb,
  name: &str,
  current: Option<&ApiProvider>,
  provider: &ApiProvider,
) -> Result<Vec<Message>, ToolError> {
  let same = current.is_some_and(|current| current.base_url() == provider.base_url() && current.key() == provider.key());
  if same {
    return Ok(Vec::new());
  }
  let secret = db
    .secret(provider.key())?
    .map_or_else(|| provider.key().to_string(), |secret| secret.name().to_string());
  Ok(vec![
    Message::new("appMcp.widens.apiProvider")
      .with("connection", name.to_owned())
      .with("endpoint", provider.base_url().as_str().to_owned())
      .with("secret", secret),
  ])
}

/// The project's secret that is not archived named by `named`: its id or, when only one secret has it, its name.
fn secret_named(db: &ProjectDb, named: &str) -> Result<Secret, ToolError> {
  let active: Vec<Secret> = db.secrets()?.into_iter().filter(|secret| !secret.is_archived()).collect();
  if let Some(secret) = active.iter().find(|secret| secret.id().to_string() == named) {
    return Ok(secret.clone());
  }
  let mut matching = active.into_iter().filter(|secret| secret.name().as_str() == named);
  let not_found = || {
    ToolError::refused(
      "secret_not_found",
      vec![Message::new("appMcp.errors.secretNotFound").with("secret", named.to_owned())],
    )
  };
  let secret = matching.next().ok_or_else(not_found)?;
  if matching.next().is_some() {
    return Err(ToolError::refused(
      "secret_ambiguous",
      vec![Message::new("appMcp.errors.secretAmbiguous").with("secret", named.to_owned())],
    ));
  }
  Ok(secret)
}

/// A connection's API provider as listed: where it sends the agents' requests, the name of the secret holding its
/// key (never the value) and its models.
pub(super) fn provider_json(provider: &ApiProvider, secrets: &[Secret]) -> Value {
  let key = secrets
    .iter()
    .find(|secret| secret.id() == provider.key())
    .map(|secret| secret.name().to_string());
  json!({
    "base_url": provider.base_url().as_str(),
    "host": provider.base_url().host(),
    "key_secret": key,
    "models": provider.model_ids().map(ModelId::as_str).collect::<Vec<_>>(),
  })
}

fn saved(error: &SaveError, named: &str) -> ToolError {
  match error {
    SaveError::Connection(orchester_core::connection::ConnectionError::NoApiProvider(_)) => ToolError::refused(
      "not_an_agent_cli",
      vec![Message::new("appMcp.errors.notAnAgentCli").with("connection", named.to_owned())],
    ),
    other => ToolError::refused("project_data", vec![other.message()]),
  }
}
