//! Automation's decision connection tools use the same workspace writes as the window.
//! No argument accepts authority, a key value or a secret: a server that takes a key is added by a person in
//! Settings › Accounts, where the app keeps the key. Hosted saves are always person_only.

use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  name::Name,
  resource::ModelId,
};
use orchester_store::ProjectDb;
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::{Spec, parse};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::decision::{self, DecisionDraft, DecisionError, Writer},
  messages::Message,
  tasks::outside::Outside,
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "save_decision_connection",
    description: "Adds or edits a decision model connection. Without connection, adds; with its id or name, edits. \
      base_url ends before /systemone; Check uses GET {base_url}/models. It takes no key: a server that needs one is added by a person in Settings › \
      Accounts; an edit keeps the key the connection has. A hosted server is always person_only for automation, \
      including edits. Only localhost, 127.0.0.1 and [::1] count as this computer. Changing server or key drops \
      its listed models and check. A rename keeps them. Returns the saved connection.",
    schema: schema_for_type::<Save>,
    read_only: false,
  },
  Spec {
    name: "check_decision_connection",
    description: "Checks a decision connection by a bodyless GET {base_url}/models with its key if configured. \
      Sends no project request data and never calls /systemone. Records models and the check's state and answer \
      timestamp. selected_model, if given, records model_gone when no longer listed. Public catalogs establish \
      model availability only; they cannot validate a key or prove inference works. Redirects are refused.",
    schema: schema_for_type::<Check>,
    read_only: false,
  },
  Spec {
    name: "remove_decision_connection",
    description: "Deletes a decision model connection by id or name. Refused while the project's reviewer uses it.",
    schema: schema_for_type::<InDecision>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
struct Save {
  project: String,
  connection: Option<String>,
  name: String,
  base_url: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
struct InDecision {
  project: String,
  connection: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
struct Check {
  project: String,
  connection: String,
  selected_model: Option<String>,
}

pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "save_decision_connection" => save(services, parse(arguments)?).await,
    "check_decision_connection" => check(services, parse(arguments)?).await,
    "remove_decision_connection" => remove(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn save(services: &Services, args: Save) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let edited = args
      .connection
      .as_deref()
      .map(|named| named_connection(&db, named))
      .transpose()?;
    let draft = DecisionDraft {
      id: edited.as_ref().map(DecisionConnection::id),
      name: Name::new(&args.name).map_err(|_| invalid("invalid_name"))?,
      base_url: DecisionServerUrl::new(&args.base_url).map_err(|_| invalid("invalid_base_url"))?,
      // The key an edited connection has stays; a tool gives none.
      key: edited.as_ref().and_then(DecisionConnection::key),
    };
    let id = decision::save_in(&open.repo_root, &draft, Writer::Automation).map_err(failure)?;
    services.changes.record(open.project.id(), Outside::Connections);
    let saved = db
      .decision_connection(id)?
      .ok_or_else(|| ToolError::not_found(Missing::Connection, id.to_string()))?;
    Ok(json!({ "connection": connection_json(&saved) }))
  })
  .await
}

async fn check(services: &Services, args: Check) -> Result<Value, ToolError> {
  let read = services.clone();
  let (open, id) = blocking(move || {
    let open = read.project(&args.project)?;
    let id = named_connection(&ProjectDb::open(&open.repo_root)?, &args.connection)?.id();
    Ok((open, id))
  })
  .await?;
  let selected = args
    .selected_model
    .map(|model| ModelId::new(&model).map_err(|_| invalid("invalid_model")))
    .transpose()?;
  let checked = decision::check_in(&open.repo_root, services.vault.get(), id, selected.as_ref())
    .await
    .map_err(failure)?;
  services.changes.record(open.project.id(), Outside::Connections);
  Ok(json!({ "connection": connection_json(&checked) }))
}

async fn remove(services: &Services, args: InDecision) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = named_connection(&ProjectDb::open(&open.repo_root)?, &args.connection)?.id();
    decision::delete_in(&open.repo_root, id).map_err(failure)?;
    services.changes.record(open.project.id(), Outside::Connections);
    Ok(json!({ "removed": id.to_string() }))
  })
  .await
}

fn named_connection(db: &ProjectDb, named: &str) -> Result<DecisionConnection, ToolError> {
  db.decision_connections()?
    .into_iter()
    .find(|connection| connection.id().to_string() == named || connection.name().as_str().eq_ignore_ascii_case(named))
    .ok_or_else(|| ToolError::not_found(Missing::Connection, named))
}

pub(super) fn connection_json(connection: &DecisionConnection) -> Value {
  json!({
    "id": connection.id().to_string(),
    "name": connection.name().as_str(),
    "base_url": connection.base_url().as_str(),
    "on_this_computer": connection.is_on_this_computer(),
    "takes_key": connection.key().is_some(),
    "models": connection.models().iter().map(ModelId::as_str).collect::<Vec<_>>(),
    "last_check": connection.last_check(),
  })
}

fn invalid(code: &str) -> ToolError {
  ToolError::refused(code, vec![Message::new("errors.storage")])
}

fn failure(error: DecisionError) -> ToolError {
  match error {
    DecisionError::Store(error) => error.into(),
    DecisionError::PersonOnly => ToolError::refused("person_only", vec![Message::new("appMcp.errors.personOnly")]),
    DecisionError::CheckStale => invalid("check_stale"),
    DecisionError::EditStale => invalid("edit_stale"),
    DecisionError::KeyUnavailable => invalid("secret_not_found"),
    DecisionError::ClientUnavailable => invalid("check_unavailable"),
  }
}
