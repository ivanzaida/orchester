//! `save_service_draft`: a service's definition through Orchester's MCP server, in the shape `get_service` returns
//! it, so a client reads a definition, changes it and saves it back. Every field is checked by the core type it
//! becomes. A service runs its program with the person's rights, network access and no sandbox, in a run's worktree,
//! so making one, or changing what it runs, is a person's decision unless a person lets automation make it
//! ([`crate::agent_testing::widening`]).

use std::collections::BTreeMap;

use orchester_core::{
  resource::{DraftSaved, ResourceKind},
  service::Service,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
  services::{entry_named, invalid, service_json},
  widening,
};
use crate::{
  agent_testing::{
    data::{create, entries, service_slug},
    widening::{NEW_SERVICE, changes},
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::save_draft_expecting_in,
  messages::Message,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveServiceDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The service to change, by id or name; left out, a new one is made under the definition's name.
  pub service: Option<String>,
  /// The whole definition, replacing the draft's.
  pub definition: ServiceDefinition,
  /// The service's `revision` as get_service or the last save returned it: the draft is saved only while it is
  /// unchanged, else refused as draft_changed. Left out, the save goes over a change saved meanwhile.
  pub expected_revision: Option<String>,
}

/// A program Orchester starts for a run, without a shell. Texts may hold `{{port}}`, `{{url}}`, `{{run}}` and
/// `{{services.<name>.url}}` or `{{services.<name>.port}}`, filled per run.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ServiceDefinition {
  /// The service's name, which needs and stacks name it by: 1–64 lowercase letters, digits and single hyphens. A
  /// service keeps the name it was made with.
  pub name: String,
  /// The program: a name on the search path or a path, started without a shell.
  pub program: String,
  /// Each passed to the program as it is.
  #[serde(default)]
  pub arguments: Vec<String>,
  /// A folder inside the run's worktree; left out or empty, the worktree itself.
  #[serde(default)]
  pub working_folder: String,
  /// Variables the program gets, by name: `{"plain": "text"}`, or `{"secret": "NAME"}` for a project secret, whose
  /// value the runner reads when it starts the service.
  #[serde(default)]
  pub variables: BTreeMap<String, DefinitionValue>,
  /// The services started and ready before this one, in order.
  #[serde(default)]
  pub needs: Vec<String>,
  pub readiness: DefinitionReadiness,
  /// What runs once after the service is ready; left out, nothing.
  #[serde(default)]
  pub seed: DefinitionSeed,
  /// The data a run gets of its own, deleted when the run ends; left out, none.
  #[serde(default)]
  pub run_data: DefinitionRunData,
  /// How agents see and use the service's UI; left out, none.
  #[serde(default)]
  pub ui_driver: DefinitionUiDriver,
  /// Where `ui_open` starts, e.g. `{{url}}/teams/{{seed.team_id}}`; left out, the driver decides.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ui_start: Option<String>,
  /// Repository path patterns the service owns, e.g. `web/cloud/**`.
  #[serde(default)]
  pub owned_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionValue {
  Plain(String),
  Secret(String),
}

/// When the started service counts as ready, checked for up to `timeout_secs` (1–3600).
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionReadiness {
  /// Its port accepts connections.
  PortOpen { timeout_secs: u64 },
  /// An HTTP 2xx or 3xx answer on `path` (starting with `/`) of its URL.
  Http { path: String, timeout_secs: u64 },
  /// A line of its output matches the regular expression `pattern`.
  LogLine { pattern: String, timeout_secs: u64 },
}

#[derive(Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionSeed {
  #[default]
  None,
  /// HTTP requests sent in order; each captured name once across them.
  Requests { requests: Vec<DefinitionRequest> },
  /// A program run without a shell in the service's working folder.
  Command {
    program: String,
    #[serde(default)]
    arguments: Vec<String>,
  },
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DefinitionRequest {
  pub method: DefinitionMethod,
  /// An http(s) URL or one starting with a placeholder, e.g. `{{url}}/api/teams`.
  pub url: String,
  #[serde(default)]
  pub body: Option<String>,
  #[serde(default)]
  pub headers: Vec<DefinitionHeader>,
  /// Values the answer gives later requests and agents, e.g. `{"name": "team_id", "path": "$.id"}`.
  #[serde(default)]
  pub captures: Vec<DefinitionCapture>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionMethod {
  Get,
  Post,
  Put,
  Patch,
  Delete,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DefinitionHeader {
  pub name: String,
  pub value: DefinitionValue,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DefinitionCapture {
  pub name: String,
  pub path: String,
}

#[derive(Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionRunData {
  #[default]
  None,
  /// A fresh database container per run.
  DatabaseContainer,
  /// Files in a folder below the worktree, e.g. `runs/{{run}}/team`.
  RunFolder { folder: String },
}

#[derive(Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum DefinitionUiDriver {
  #[default]
  None,
  /// Windows UI Automation, macOS Accessibility.
  Native,
  /// An app with its own MCP server.
  Mcp,
}

pub(super) async fn save_service_draft(services: &Services, args: SaveServiceDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let service = service_of(&args.definition)?;
    let (id, saved) = match args.service.as_deref() {
      Some(named) => {
        let all = entries::<Service, _>(&db)?;
        let entry = entry_named(&all, named, Missing::Service)?;
        if entry.body.name() != service.name() {
          return Err(ToolError::refused(
            "name_not_editable",
            vec![Message::new("appMcp.services.errors.nameNotEditable")],
          ));
        }
        widening::allowed(&services, changes(&entry.body, &service))?;
        let expected = args.expected_revision.as_deref();
        (
          entry.id,
          save_draft_expecting_in(&open.repo_root, entry.id, service, expected)?,
        )
      }
      None if args.expected_revision.is_some() => {
        return Err(ToolError::InvalidArguments(
          "expected_revision names a service's draft; a new service has none".into(),
        ));
      }
      None => {
        let slug = service_slug(service.name());
        if db.slugs(ResourceKind::Service)?.contains(&slug) {
          return Err(name_taken(slug.as_str()));
        }
        widening::allowed(&services, vec![Message::new(NEW_SERVICE)])?;
        (create(&mut db, slug, &service, Timestamp::now())?, DraftSaved::Stored)
      }
    };
    let mut value = service_json(&open.repo_root, id)?;
    value["draft_kept"] = json!(saved == DraftSaved::Stored);
    Ok(json!({ "service": value }))
  })
  .await
}

/// The service `definition` describes, checked by the constructors a stored body is read with.
fn service_of(definition: &ServiceDefinition) -> Result<Service, ToolError> {
  let value = serde_json::to_value(definition).expect("a definition is plain data with string keys");
  serde_json::from_value(value).map_err(|error| invalid("invalid_service", &error))
}

fn name_taken(name: &str) -> ToolError {
  ToolError::refused(
    "name_taken",
    vec![Message::new("appMcp.services.errors.serviceNameTaken").with("name", name.to_owned())],
  )
}
