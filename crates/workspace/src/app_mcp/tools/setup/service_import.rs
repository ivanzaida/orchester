//! `list_service_proposals` and `save_service_proposals` (ORC-254): a first draft of a project's services and stacks
//! read from its checkout ([`crate::agent_testing::import`]), and the chosen proposals saved as drafts of new services
//! and stacks, never published. Reading runs nothing of the checkout. A new service runs a program once a person
//! publishes it, so saving services is person_only unless a person lets automation change agent permissions; stacks
//! are always taken.

use orchester_core::{resource::ResourceKind, time::Timestamp};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Spec, Translate, parse, projects::InProject},
  widening,
};
use crate::{
  agent_testing::{
    import::{Choice, Draft, Proposal, Proposals, Source, propose_in, save_in},
    widening::IMPORTED_SERVICES,
  },
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_service_proposals",
    description: "A first draft of a project's services and stacks, read from its checkout without running anything \
                  of it: each docker compose service with an image (a fresh container per start, its command, \
                  environment values, depends_on as needs, a healthcheck asking a local URL as an HTTP readiness \
                  check) and each compose file's services as a stack; each package.json script named like dev, \
                  start, preview or serve, workspaces' packages included, run through the package's manager; and \
                  each Cargo binary target, run with cargo run. Each proposal gives its kind, name, definition in \
                  save_service_draft's shape (a stack: its services), source (file and key), unmapped (each key it \
                  could not carry, with why) and exists (the project has that name already, so saving keeps the \
                  project's). skipped lists what was found but not proposed, unreadable the files that could not be \
                  read, both with why; truncated says the checkout was too large to read whole.",
    schema: schema_for_type::<InProject>,
    read_only: true,
  },
  Spec {
    name: "save_service_proposals",
    description: "Saves the chosen proposals of list_service_proposals, by name, as drafts of new services and \
                  stacks; nothing is published, so no run starts them until a person publishes them. The checkout is \
                  read again, so a name it no longer proposes is refused as not_proposed and nothing is saved. A \
                  proposal whose name the project has already is not saved and is listed under taken. A service \
                  runs its program once published, so saving any is refused as person_only unless a person turned \
                  on Let automation change agent permissions; stacks are always taken.",
    schema: schema_for_type::<SaveProposals>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveProposals {
  /// The project's id, or its name.
  pub project: String,
  /// The names of the service proposals to save.
  #[serde(default)]
  pub services: Vec<String>,
  /// The names of the stack proposals to save.
  #[serde(default)]
  pub stacks: Vec<String>,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_service_proposals" => list(services, parse(arguments)?, translate).await,
    "save_service_proposals" => save(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list(services: &Services, args: InProject, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let proposals = blocking(move || {
    let open = services.project(&args.project)?;
    propose_in(&open.repo_root).map_err(|reason| ToolError::refused("checkout_unreadable", vec![reason]))
  })
  .await?;
  Ok(proposals_json(&proposals, translate))
}

async fn save(services: &Services, args: SaveProposals) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let proposals =
      propose_in(&open.repo_root).map_err(|reason| ToolError::refused("checkout_unreadable", vec![reason]))?;
    let choice = Choice {
      services: args.services,
      stacks: args.stacks,
    };
    let chosen = proposals
      .chosen(&choice)
      .map_err(|reason| ToolError::refused("not_proposed", vec![reason]))?;
    let new_services: Vec<&str> = chosen
      .iter()
      .filter(|proposal| matches!(proposal.draft, Draft::Service(_)) && !proposal.exists)
      .map(|proposal| proposal.draft.name())
      .collect();
    if !new_services.is_empty() {
      let reason = Message::new(IMPORTED_SERVICES).with("names", new_services.join(", "));
      widening::allowed(&services, vec![reason])?;
    }
    let imported = save_in(&open.repo_root, &chosen, Timestamp::now())
      .map_err(|reason| ToolError::refused("project_data", vec![reason]))?;
    Ok(json!({
      "saved": imported.saved.iter().map(|saved| json!({
        "kind": kind_name(saved.kind),
        "name": saved.name,
        "id": saved.id.to_string(),
      })).collect::<Vec<_>>(),
      "taken": imported.taken.iter().map(|(kind, name)| json!({ "kind": kind_name(*kind), "name": name }))
        .collect::<Vec<_>>(),
    }))
  })
  .await
}

fn proposals_json(proposals: &Proposals, translate: &Translate) -> Value {
  let reason =
    |source: &Source, message: &Message| json!({ "source": source_json(source), "reason": translate.text(message) });
  json!({
    "proposals": proposals.proposals.iter().map(|proposal| proposal_json(proposal, translate)).collect::<Vec<_>>(),
    "skipped": proposals.skipped.iter().map(|skipped| reason(&skipped.source, &skipped.reason)).collect::<Vec<_>>(),
    "unreadable": proposals.unreadable.iter().map(|unreadable| json!({
      "file": unreadable.file,
      "reason": translate.text(&unreadable.reason),
    })).collect::<Vec<_>>(),
    "truncated": proposals.truncated,
  })
}

fn proposal_json(proposal: &Proposal, translate: &Translate) -> Value {
  let mut value = json!({
    "kind": kind_name(proposal.draft.kind()),
    "name": proposal.draft.name(),
    "source": source_json(&proposal.source),
    "unmapped": proposal.unmapped.iter().map(|unmapped| json!({
      "key": unmapped.key,
      "reason": translate.text(&unmapped.reason),
    })).collect::<Vec<_>>(),
    "exists": proposal.exists,
  });
  match &proposal.draft {
    Draft::Service(service) => {
      value["definition"] = serde_json::to_value(service).expect("a service is plain data with string keys");
    }
    Draft::Stack(stack) => {
      value["services"] = json!(stack.services().iter().map(|name| name.as_str()).collect::<Vec<_>>());
    }
  }
  value
}

fn source_json(source: &Source) -> Value {
  json!({ "file": source.file, "key": source.key })
}

const fn kind_name(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::Stack => "stack",
    _ => "service",
  }
}
