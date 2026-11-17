//! A team project's instructions through the tools (`docs/design/local-and-team-parity.md`): they are the team's
//! resources, read from the team's configuration this session read. `list_instructions` and `get_instruction` read
//! them, the agents and pipelines an instruction names are the team's, `save_instruction_draft` publishes the next
//! version on the team server at once (without `instruction`, the team's new instruction as its version 1), checked
//! as a local save is, and `publish_instruction` answers it as it stands: the team keeps no draft on a computer. What
//! is published reaches agents' prompts, so it is a person's decision as a local publication is
//! ([`widening::prompt_allowed`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  resource::{Instruction, ResourceBody, ResourceKind},
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Value, json};

use super::{
  super::wire,
  created::refused as edit_refused,
  given_by::Givers,
  instructions::{SaveInstructionDraft, entry_json, entry_named, instruction, refused, write_error},
  team_profiles::TeamProject,
  team_resources::{Sent, TeamEntry, config_read, entries, marked, named, pipeline_choices, profile_choices, send},
  widening,
};
use crate::{
  agents::data::VersionState,
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    editor::WriteError,
    publication::missing,
  },
  messages::{self, Message},
  prompt_changes,
  sharing::team_edit::{NewTeamResource, Publication},
  tasks::outside::Outside,
  team_config::TeamConfig,
  workspace::OpenProject,
};

const KIND: (ResourceKind, Missing) = (ResourceKind::Instruction, Missing::Instruction);

/// The team's instructions in `config` with the team's profiles and pipelines they choose from, as the local tools read
/// a project's own.
fn data(config: &TeamConfig) -> ProjectInstructions {
  let profiles = profile_choices(config);
  let entries = entries::<Instruction>(config, ResourceKind::Instruction)
    .into_iter()
    .map(|team| InstructionEntry {
      id: team.id,
      name: team.name,
      slug: team.slug,
      body: team.body,
      state: VersionState::Published(team.number),
      draft_saved_at: None,
    })
    .collect();
  ProjectInstructions {
    entries: Arc::new(entries),
    pipelines: Arc::new(pipeline_choices(config, &profiles)),
    profiles: Arc::new(profiles),
    automation_drafts: Arc::default(),
  }
}

/// The team's instructions of the project at `open`, each with its id, as `list_instructions` lists them; none for a
/// local project and while the team's configuration was not read this session.
pub(super) fn listed(
  services: &Services,
  (open, db): (&OpenProject, &ProjectDb),
) -> Result<Vec<(ResourceId, Value)>, ToolError> {
  let Some(config) = config_read(services, (open, db))? else {
    return Ok(Vec::new());
  };
  let (data, givers) = (data(&config), Givers::team(&config));
  Ok(
    data
      .entries
      .iter()
      .map(|entry| (entry.id, shown(&data, &givers, entry, open.project.id())))
      .collect(),
  )
}

/// `entry` as `list_instructions` lists it, marked as the team's.
fn shown(data: &ProjectInstructions, givers: &Givers, entry: &InstructionEntry, project: ProjectId) -> Value {
  let number = entry
    .state
    .published()
    .expect("a team instruction is its latest version");
  marked(entry_json((data, givers), entry, project), number)
}

/// The team instruction `named` of the team project `team`, when the team has one by that id, name or slug.
pub(super) fn of(team: &TeamProject, named: &str) -> Option<ResourceId> {
  self::named::<Instruction>(&team.config, KIND, named)
    .ok()
    .map(|entry| entry.id)
}

/// Team instruction `id` of `open`'s project as `get_instruction` answers it, read now, with its latest version.
pub(super) async fn shown_now(services: &Services, open: &OpenProject, id: ResourceId) -> Result<Value, ToolError> {
  let (services, open) = (services.clone(), open.clone());
  blocking(move || {
    let config = services.workspace.team_read(&open.repo_root).loaded()?;
    let latest: TeamEntry<Instruction> = named(&config, KIND, &id.to_string())?;
    let data = data(&config);
    let entry = data
      .entry(id)
      .ok_or_else(|| ToolError::not_found(Missing::Instruction, id.to_string()))?;
    let mut instruction = shown(&data, &Givers::team(&config), entry, open.project.id());
    instruction["versions"] = json!([{
      "version": latest.number.get(),
      "published_at": wire::time(latest.published_at),
      "text": latest.body.text().as_str(),
    }]);
    Ok(json!({ "instruction": instruction }))
  })
  .await
}

/// Publishes the instruction `args` give in the team project `team` as its next version, or as the team's new
/// instruction; the instruction as `get_instruction` shows it after.
pub(super) async fn save(
  services: &Services,
  team: &TeamProject,
  args: SaveInstructionDraft,
) -> Result<Value, ToolError> {
  let (reading, config, root) = (services.clone(), team.config.clone(), team.open.repo_root.clone());
  let sent = blocking(move || prepared(&reading, (&root, &config), &args)).await?;
  let id = send(services, &team.open.repo_root, sent).await?;
  services.changes.record(team.open.project.id(), Outside::Instructions);
  shown_now(services, &team.open, id).await
}

/// What a save of `args` sends, checked as a local save and a publication are, and refused while publishing it is a
/// person's decision. Blocks on SQLite.
fn prepared(
  services: &Services,
  (root, config): (&Path, &TeamConfig),
  args: &SaveInstructionDraft,
) -> Result<Sent<Instruction>, ToolError> {
  let data = data(config);
  let existing = args
    .instruction
    .as_deref()
    .map(|named| entry_named(&data, named))
    .transpose()?;
  if existing.is_some() && args.name.is_some() {
    return Err(refused(
      "name_not_editable",
      Message::new("appMcp.errors.nameNotEditable"),
    ));
  }
  let body = instruction(&data, args, existing)?;
  if let Some(unavailable) = missing(body.text(), body.events()).first() {
    return Err(write_error(&WriteError::Unavailable(*unavailable)));
  }
  body
    .check_publishable()
    .map_err(|error| write_error(&WriteError::Store(StoreError::Publish(error))))?;
  let new_name = || {
    Name::new(args.name.as_deref().unwrap_or_default())
      .map_err(|error| refused("invalid_name", messages::name_error(&error)))
  };
  if existing.is_none() {
    new_name()?;
  }
  // Every save publishes at once, so it reaches agents' prompts as a local publication does.
  widening::prompt_allowed(services, vec![prompt_changes::instruction()])?;
  if let Some(entry) = existing {
    let base = entry
      .state
      .published()
      .expect("a team instruction is its latest version");
    return Ok(Sent::Next(entry.id, base, body));
  }
  let name = new_name()?;
  let id = ResourceId::generate();
  let publication = Publication::of_body(root, id, &body).map_err(|error| edit_refused(&error))?;
  Ok(Sent::New(NewTeamResource {
    id,
    kind: ResourceKind::Instruction,
    name,
    publication,
  }))
}
