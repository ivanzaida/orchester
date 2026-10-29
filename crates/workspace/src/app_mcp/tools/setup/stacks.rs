//! `list_stacks`, `get_stack`, `save_stack_draft` and `publish_stack` ([`super::services`]). A stack starts only
//! published services, whose programs a person allowed, so saving and publishing one widens nothing.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{DraftSaved, ResourceKind},
  service::{ServiceName, Stack, StackName},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Translate,
    across::{InProjects, across},
  },
  profiles::no_draft,
  services::{entry_json, entry_named, invalid, publish_error},
};
use crate::{
  agent_testing::data::{Entry, create, entries, stack_slug},
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::{revision_of, save_draft_expecting_in},
  messages::Message,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InStack {
  /// The project's id, or its name.
  pub project: String,
  /// The stack's id, or its name.
  pub stack: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveStackDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The stack to change, by id or name; left out, a new one is made under `name`.
  pub stack: Option<String>,
  /// A new stack's name: 1–64 lowercase letters, digits and single hyphens. A stack keeps the name it was made with.
  pub name: Option<String>,
  /// The services' names, at least one, each once, in the order they are listed.
  pub services: Vec<String>,
  /// The stack's `revision` as get_stack or the last save returned it: the draft is saved only while it is
  /// unchanged, else refused as draft_changed. Left out, the save goes over a change saved meanwhile.
  pub expected_revision: Option<String>,
}

pub(super) async fn list_stacks(
  services: &Services,
  args: InProjects,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || across(&services, args.project.as_deref(), |_, db| Ok(entries::<Stack, _>(db)?))).await?;
  Ok(read.json("stacks", translate, |_, entries| {
    entries.iter().map(summary_json).collect()
  }))
}

pub(super) async fn get_stack(services: &Services, args: InStack) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = entry_named(
      &entries::<Stack, _>(&ProjectDb::open(&open.repo_root)?)?,
      &args.stack,
      Missing::Stack,
    )?
    .id;
    Ok(json!({ "stack": stack_json(&open.repo_root, id)? }))
  })
  .await
}

pub(super) async fn save_stack_draft(services: &Services, args: SaveStackDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let all = entries::<Stack, _>(&db)?;
    let existing = args
      .stack
      .as_deref()
      .map(|named| entry_named(&all, named, Missing::Stack))
      .transpose()?;
    let name = match (existing, &args.name) {
      (Some(entry), Some(name)) if name != &*entry.name => {
        return Err(ToolError::refused(
          "name_not_editable",
          vec![Message::new("appMcp.services.errors.stackNameNotEditable")],
        ));
      }
      (Some(entry), _) => entry.body.name().clone(),
      (None, name) => {
        StackName::new(name.as_deref().unwrap_or_default()).map_err(|error| invalid("invalid_stack", &error))?
      }
    };
    let stack = stack_of(name, &args.services)?;
    let (id, saved) = match existing {
      Some(entry) => {
        let expected = args.expected_revision.as_deref();
        (
          entry.id,
          save_draft_expecting_in(&open.repo_root, entry.id, stack, expected)?,
        )
      }
      None if args.expected_revision.is_some() => {
        return Err(ToolError::InvalidArguments(
          "expected_revision names a stack's draft; a new stack has none".into(),
        ));
      }
      None => {
        let slug = stack_slug(stack.name());
        if db.slugs(ResourceKind::Stack)?.contains(&slug) {
          return Err(ToolError::refused(
            "name_taken",
            vec![Message::new("appMcp.services.errors.stackNameTaken").with("name", slug.as_str().to_owned())],
          ));
        }
        (create(&mut db, slug, &stack, Timestamp::now())?, DraftSaved::Stored)
      }
    };
    let mut value = stack_json(&open.repo_root, id)?;
    value["draft_kept"] = json!(saved == DraftSaved::Stored);
    Ok(json!({ "stack": value }))
  })
  .await
}

pub(super) async fn publish_stack(services: &Services, args: InStack) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let id = entry_named(&entries::<Stack, _>(&db)?, &args.stack, Missing::Stack)?.id;
    let draft = db.draft::<Stack>(id)?.ok_or_else(no_draft)?;
    services.draft_decided::<Stack>(&db, id)?;
    db.publish_draft_matching(id, draft.body(), Timestamp::now())
      .map_err(|error| publish_error(&error))?;
    Ok(json!({ "stack": stack_json(&open.repo_root, id)? }))
  })
  .await
}

fn stack_of(name: StackName, services: &[String]) -> Result<Stack, ToolError> {
  let services = services
    .iter()
    .map(|service| ServiceName::new(service))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| invalid("invalid_stack", &error))?;
  Stack::new(name, services).map_err(|error| invalid("invalid_stack", &error))
}

/// Stack `id` as get_stack shows it, with its revision, read before it.
fn stack_json(root: &Path, id: ResourceId) -> Result<Value, ToolError> {
  let db = ProjectDb::open(root)?;
  let revision = revision_of::<Stack>(&db, id)?;
  let all = entries::<Stack, _>(&db)?;
  let entry = all
    .iter()
    .find(|entry| entry.id == id)
    .ok_or_else(|| ToolError::not_found(Missing::Stack, id.to_string()))?;
  let mut value = summary_json(entry);
  value["revision"] = json!(revision);
  Ok(value)
}

fn summary_json(entry: &Entry<Stack>) -> Value {
  let mut value = entry_json(entry);
  value["services"] = json!(
    entry
      .body
      .services()
      .iter()
      .map(ServiceName::as_str)
      .collect::<Vec<_>>()
  );
  value
}
