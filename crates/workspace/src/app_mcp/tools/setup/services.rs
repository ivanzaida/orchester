//! `list_services`, `get_service`, `save_service_draft` and `publish_service`, and the same four for stacks
//! ([`super::stacks`]): a project's services and stacks (`docs/architecture/agent-testing.md`), which live in the
//! project database with a draft and published versions as tools do. Publishing checks that every service a
//! service needs, or a stack lists, is published and that the needs form no cycle. A service runs a program, so
//! making one, changing what it runs, and its first version are person_only unless a person lets automation change
//! agent permissions. Nothing of a secret's value is read or returned: a variable or header names a project secret.

use std::{fmt::Display, path::Path};

use orchester_core::{
  id::ResourceId,
  resource::{CommandArg, PublishError},
  service::{NeedsError, ReadyCheck, Service, UiDriver},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Spec, Translate,
    across::{InProjects, across},
    parse,
  },
  profiles::{no_draft, state_json},
  service_input, stacks, widening,
};
use crate::{
  agent_testing::{
    data::{Entry, entries},
    widening::{FIRST_VERSION, changes},
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::revision_of,
  messages::{self, Message},
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_services",
    description: "A project's services, the programs Orchester starts for runs: id, name, where each stands \
                  (draft, stale_draft or published, with its published version), program, the services it needs, \
                  its readiness check and its UI driver. Without `project`, every project's, each naming its \
                  project (unreadable_projects lists the projects that could not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_service",
    description: "One service (its draft if it has one, otherwise its latest version): where it stands, its \
                  definition in the shape save_service_draft takes (name, program, arguments, working_folder, \
                  variables as {plain} or {secret: NAME}, needs, readiness, seed, run_data, ui_driver, owned_paths), \
                  and its revision, which save_service_draft's expected_revision takes.",
    schema: schema_for_type::<InService>,
    read_only: true,
  },
  Spec {
    name: "save_service_draft",
    description: "Replaces a service's draft with `definition`; without `service` a new one is made under the \
                  definition's name, unique among the project's services. A service keeps its name. The program \
                  starts without a shell in the run's worktree, with your rights, network access and no sandbox. \
                  Variables are plain text or a project secret by name, never a secret's value. Unless a person \
                  turned on Let automation change agent permissions, making a service is refused as person_only, as \
                  is changing its program, arguments, working folder, variables or seed; its readiness, needs, run \
                  data, UI driver and owned paths are always taken. With expected_revision, the revision \
                  get_service or the last save returned, the draft is saved only while it is unchanged, else \
                  refused as draft_changed. Publish it with publish_service.",
    schema: schema_for_type::<service_input::SaveServiceDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_service",
    description: "Publishes a service's draft as its next version, so runs can start it. Refused as \
                  needs_not_published while a service it needs is not published, as needs_cycle when the needs \
                  would form a cycle with the published services, for a draft started from an older version than \
                  the latest, and as draft_changed for a draft that changed while it was checked. Unless a person \
                  turned on Let automation change agent permissions, a service's first version is refused as \
                  person_only, and so is a draft that runs another program, other arguments, another working \
                  folder, other variables or another seed than the latest version.",
    schema: schema_for_type::<InService>,
    read_only: false,
  },
  Spec {
    name: "list_stacks",
    description: "A project's stacks, named sets of services started together: id, name, where each stands and its \
                  services. Without `project`, every project's, each naming its project (unreadable_projects lists \
                  the projects that could not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_stack",
    description: "One stack (its draft if it has one, otherwise its latest version): name, services, where it stands \
                  and its revision, which save_stack_draft's expected_revision takes.",
    schema: schema_for_type::<stacks::InStack>,
    read_only: true,
  },
  Spec {
    name: "save_stack_draft",
    description: "Replaces a stack's services with `services` (at least one, each once); without `stack` a new one \
                  is made under `name`, unique among the project's stacks. A stack keeps its name. A stack starts \
                  only published services, so saving it runs nothing new. With expected_revision the draft is saved \
                  only while it is unchanged, else refused as draft_changed. Publish it with publish_stack.",
    schema: schema_for_type::<stacks::SaveStackDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_stack",
    description: "Publishes a stack's draft as its next version. Refused as needs_not_published while a service it \
                  lists is not published, as needs_cycle when their needs form a cycle, for a draft started from an \
                  older version than the latest, and as draft_changed for a draft that changed while it was checked.",
    schema: schema_for_type::<stacks::InStack>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InService {
  /// The project's id, or its name.
  pub project: String,
  /// The service's id, or its name.
  pub service: String,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_services" => list_services(services, parse(arguments)?, translate).await,
    "get_service" => get_service(services, parse(arguments)?).await,
    "save_service_draft" => service_input::save_service_draft(services, parse(arguments)?).await,
    "publish_service" => publish_service(services, parse(arguments)?).await,
    "list_stacks" => stacks::list_stacks(services, parse(arguments)?, translate).await,
    "get_stack" => stacks::get_stack(services, parse(arguments)?).await,
    "save_stack_draft" => stacks::save_stack_draft(services, parse(arguments)?).await,
    "publish_stack" => stacks::publish_stack(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_services(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |_, db| {
      Ok(entries::<Service, _>(db)?)
    })
  })
  .await?;
  Ok(read.json("services", translate, |_, entries| {
    entries.iter().map(summary_json).collect()
  }))
}

async fn get_service(services: &Services, args: InService) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = entry_named(
      &entries::<Service, _>(&ProjectDb::open(&open.repo_root)?)?,
      &args.service,
      Missing::Service,
    )?
    .id;
    Ok(json!({ "service": service_json(&open.repo_root, id)? }))
  })
  .await
}

async fn publish_service(services: &Services, args: InService) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let id = entry_named(&entries::<Service, _>(&db)?, &args.service, Missing::Service)?.id;
    let draft = db.draft::<Service>(id)?.ok_or_else(no_draft)?;
    let latest = db.latest_version::<Service>(id)?;
    services.draft_decided::<Service>(&db, id)?;
    let reasons = match &latest {
      Some(version) => changes(version.body(), draft.body()),
      None => vec![Message::new(FIRST_VERSION)],
    };
    widening::allowed(&services, reasons)?;
    // Published only while the draft is the one just checked.
    db.publish_draft_matching(id, draft.body(), Timestamp::now())
      .map_err(|error| publish_error(&error))?;
    Ok(json!({ "service": service_json(&open.repo_root, id)? }))
  })
  .await
}

/// The entry of `entries` named by its id or else, ignoring case, by its name.
pub(super) fn entry_named<'a, B>(
  entries: &'a [Entry<B>],
  named: &str,
  missing: Missing,
) -> Result<&'a Entry<B>, ToolError> {
  let lowered = named.to_lowercase();
  // An id names one entry for certain; a name could also be another entry's id.
  let by_id = entries.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| entries.iter().find(|entry| entry.name.to_lowercase() == lowered))
    .ok_or_else(|| ToolError::not_found(missing, named))
}

/// Service `id` as get_service shows it, with its revision ([`crate::drafts::draft_revision`]), read before it: a save
/// made meanwhile shows in the definition, so a save on that revision is refused rather than going over it.
pub(super) fn service_json(root: &Path, id: ResourceId) -> Result<Value, ToolError> {
  let db = ProjectDb::open(root)?;
  let revision = revision_of::<Service>(&db, id)?;
  let all = entries::<Service, _>(&db)?;
  let entry = all
    .iter()
    .find(|entry| entry.id == id)
    .ok_or_else(|| ToolError::not_found(Missing::Service, id.to_string()))?;
  let mut value = entry_json(entry);
  value["definition"] = serde_json::to_value(&entry.body).expect("a service is plain data with string keys");
  value["revision"] = json!(revision);
  Ok(value)
}

/// What every service or stack item names: its id, name and where it stands.
pub(super) fn entry_json<B>(entry: &Entry<B>) -> Value {
  json!({ "id": entry.id.to_string(), "name": &*entry.name, "state": state_json(entry.state) })
}

fn summary_json(entry: &Entry<Service>) -> Value {
  let service = &entry.body;
  let mut value = entry_json(entry);
  value["program"] = json!(service.program().as_str());
  value["arguments"] = json!(service.arguments().iter().map(CommandArg::as_str).collect::<Vec<_>>());
  value["needs"] = json!(service.needs().iter().map(|need| need.as_str()).collect::<Vec<_>>());
  value["readiness"] = json!(match service.readiness().check() {
    ReadyCheck::PortOpen => "port_open",
    ReadyCheck::Http(_) => "http",
    ReadyCheck::LogLine(_) => "log_line",
  });
  value["ui_driver"] = json!(match service.ui_driver() {
    UiDriver::None => "none",
    UiDriver::Native => "native",
    UiDriver::Mcp => "mcp",
  });
  value
}

/// A definition that is not valid: `error` says why in the words of the core type that refused it, which name a
/// variable or header but never repeat its value or a request body.
pub(super) fn invalid(code: &str, error: &impl Display) -> ToolError {
  ToolError::refused(
    code,
    vec![Message::new("appMcp.services.errors.invalid").with("reason", error.to_string())],
  )
}

pub(super) fn publish_error(error: &StoreError) -> ToolError {
  let code = match error {
    StoreError::NoDraft(_) => return no_draft(),
    StoreError::Publish(PublishError::StaleDraft { .. }) => "stale_draft",
    StoreError::DraftChanged(_) => "draft_changed",
    StoreError::Publish(PublishError::VersionLimit) => "version_limit",
    StoreError::Publish(PublishError::Needs(NeedsError::Cycle(_))) => "needs_cycle",
    StoreError::Publish(PublishError::Needs(_)) => "needs_not_published",
    _ => "project_data",
  };
  ToolError::refused(code, vec![messages::store_error(error)])
}
