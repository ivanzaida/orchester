//! `list_instructions`, `save_instruction_draft` and `publish_instruction`: a project's instructions as
//! Instructions lists them, one made (*New instruction*: a name and where it applies) and its draft
//! saved as its editor saves it, checked with the editor's rules, and published as Publish does, through
//! the same checked writes (`instructions::editor`). Where an instruction applies and whom it reaches are
//! chosen from what the editor offers: the roles of the project's profiles, its profiles, its pipelines
//! and their steps.

use std::collections::BTreeSet;

use orchester_core::{
  name::Name,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PublishError},
};
use orchester_store::StoreError;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{super::projects::InProject, profiles::no_draft};
use crate::{
  agents::data::VersionState,
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  instructions::{
    data::{InstructionEntry, PipelineChoice, ProjectInstructions},
    editor::{self, WriteError},
    form::{Events, InstructionForm, ScopeChoice, SettingsError, Target},
  },
  messages::{self, Message},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InInstruction {
  /// The project's id, or its name.
  pub project: String,
  /// The instruction's id, or its name.
  pub instruction: String,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum AppliesIn {
  Project,
  AgentRole,
  Pipeline,
  Step,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Agents {
  All,
  Roles,
  Profiles,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum When {
  Start,
  Resume,
  StartAndResume,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveInstructionDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The instruction to change, by id or name; left out, a new one is made.
  pub instruction: Option<String>,
  /// A new instruction's name; an instruction keeps the name it was made with.
  pub name: Option<String>,
  pub applies_in: AppliesIn,
  /// With agent_role: a role of the project's profiles.
  pub role: Option<String>,
  /// With pipeline or step: the pipeline, by id or name.
  pub pipeline: Option<String>,
  /// With step: the step of that pipeline, by key or name.
  pub step: Option<String>,
  /// Which agents it reaches; with agent_role always all of the role's.
  pub agents: Agents,
  /// With roles: roles of the project's profiles.
  #[serde(default)]
  pub agent_roles: Vec<String>,
  /// With profiles: the project's profiles, by id or role.
  #[serde(default)]
  pub agent_profiles: Vec<String>,
  /// When agents get it.
  pub events: When,
  /// The instruction's text; may use {{variables}}, and may be blank in a draft.
  #[serde(default)]
  pub text: String,
}

pub async fn list_instructions(services: &Services, args: InProject) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let data = ProjectInstructions::load_in(&open.repo_root)?;
    let listed: Vec<Value> = data.entries.iter().map(|entry| entry_json(&data, entry)).collect();
    Ok(json!({ "instructions": listed }))
  })
  .await
}

pub async fn save_instruction_draft(services: &Services, args: SaveInstructionDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let data = ProjectInstructions::load_in(&root)?;
    let existing = args
      .instruction
      .as_deref()
      .map(|named| entry_named(&data, named))
      .transpose()?;
    let body = instruction(&data, &args)?;
    let saved = match (existing, &args.name) {
      (Some(_), Some(_)) => {
        return Err(refused(
          "name_not_editable",
          Message::new("appMcp.errors.nameNotEditable"),
        ));
      }
      (Some(entry), None) => editor::save_in(&root, entry, body),
      (None, name) => {
        let name = Name::new(name.as_deref().unwrap_or_default())
          .map_err(|error| refused("invalid_name", messages::name_error(&error)))?;
        editor::create_in(&root, name, &body)
      }
    };
    let id = saved.map_err(|error| write_error(&error))?.id;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::Instructions);
    let data = ProjectInstructions::load_in(&root)?;
    let entry = data
      .entry(id)
      .ok_or_else(|| ToolError::not_found(Missing::Instruction, id.to_string()))?;
    Ok(json!({ "instruction": entry_json(&data, entry) }))
  })
  .await
}

pub async fn publish_instruction(services: &Services, args: InInstruction) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let data = ProjectInstructions::load_in(&open.repo_root)?;
    let entry = entry_named(&data, &args.instruction)?;
    let id = editor::publish_in(&open.repo_root, entry)
      .map_err(|error| write_error(&error))?
      .id;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::Instructions);
    let data = ProjectInstructions::load_in(&open.repo_root)?;
    let entry = data
      .entry(id)
      .ok_or_else(|| ToolError::not_found(Missing::Instruction, id.to_string()))?;
    Ok(json!({ "instruction": entry_json(&data, entry) }))
  })
  .await
}

fn entry_named<'a>(data: &'a ProjectInstructions, named: &str) -> Result<&'a InstructionEntry, ToolError> {
  let lowered = named.to_lowercase();
  data
    .entries
    .iter()
    .find(|entry| entry.id.to_string() == named || entry.name.to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Instruction, named))
}

/// The instruction the editor's fields make, refused with its reasons.
fn instruction(data: &ProjectInstructions, args: &SaveInstructionDraft) -> Result<Instruction, ToolError> {
  let scope = scope(data, args)?;
  let targets = targets(data, args)?;
  let events = match args.events {
    When::Start => Events::Start,
    When::Resume => Events::Resume,
    When::StartAndResume => Events::StartAndResume,
  };
  InstructionForm {
    text: &args.text,
    targets: &targets,
    events,
    scope: &scope,
  }
  .instruction()
  .map_err(|errors| {
    let reasons = [errors.text, errors.settings.map(SettingsError::message)]
      .into_iter()
      .flatten()
      .collect();
    ToolError::refused("invalid_instruction", reasons)
  })
}

fn scope(data: &ProjectInstructions, args: &SaveInstructionDraft) -> Result<ScopeChoice, ToolError> {
  Ok(match args.applies_in {
    AppliesIn::Project => ScopeChoice::Project,
    AppliesIn::AgentRole => {
      ScopeChoice::AgentRole(args.role.as_deref().map(|role| role_named(data, role)).transpose()?)
    }
    AppliesIn::Pipeline => ScopeChoice::Pipeline(
      args
        .pipeline
        .as_deref()
        .map(|named| pipeline_named(data, named).map(|pipeline| pipeline.id))
        .transpose()?,
    ),
    AppliesIn::Step => {
      let pipeline = args
        .pipeline
        .as_deref()
        .map(|named| pipeline_named(data, named))
        .transpose()?;
      let step = match (pipeline, args.step.as_deref()) {
        (Some(pipeline), Some(named)) => {
          let lowered = named.to_lowercase();
          let step = pipeline
            .steps
            .iter()
            .find(|step| step.key.as_str() == named || step.name.to_lowercase() == lowered)
            .ok_or_else(|| ToolError::not_found(Missing::Step, named))?;
          Some(step.key.clone())
        }
        _ => None,
      };
      ScopeChoice::Step(pipeline.map(|pipeline| pipeline.id), step)
    }
  })
}

fn targets(data: &ProjectInstructions, args: &SaveInstructionDraft) -> Result<Vec<Target>, ToolError> {
  Ok(match args.agents {
    Agents::All => vec![Target::All],
    Agents::Roles => args
      .agent_roles
      .iter()
      .map(|role| role_named(data, role).map(Target::Role))
      .collect::<Result<_, _>>()?,
    Agents::Profiles => args
      .agent_profiles
      .iter()
      .map(|named| {
        let lowered = named.to_lowercase();
        data
          .profiles
          .iter()
          .find(|profile| profile.id.to_string() == *named || profile.role.as_str().to_lowercase() == lowered)
          .map(|profile| Target::Profile(profile.id))
          .ok_or_else(|| ToolError::not_found(Missing::Profile, named))
      })
      .collect::<Result<_, _>>()?,
  })
}

/// A role of the project's profiles, as the editor offers them.
fn role_named(data: &ProjectInstructions, named: &str) -> Result<Name, ToolError> {
  let lowered = named.trim().to_lowercase();
  data
    .roles()
    .into_iter()
    .find(|role| role.as_str().to_lowercase() == lowered)
    .ok_or_else(|| {
      refused(
        "unknown_role",
        Message::new("appMcp.errors.unknownRole").with("role", named.to_owned()),
      )
    })
}

fn pipeline_named<'a>(data: &'a ProjectInstructions, named: &str) -> Result<&'a PipelineChoice, ToolError> {
  let lowered = named.to_lowercase();
  data
    .pipelines
    .iter()
    .find(|pipeline| pipeline.id.to_string() == named || pipeline.name.to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, named))
}

fn entry_json(data: &ProjectInstructions, entry: &InstructionEntry) -> Value {
  let body = &entry.body;
  let pipeline_name = |id| data.pipeline(id).map(|pipeline| &*pipeline.name);
  let applies_in = match body.scope() {
    InstructionScope::Project => json!({ "kind": "project" }),
    InstructionScope::AgentRole { role } => json!({ "kind": "agent_role", "role": role.as_str() }),
    InstructionScope::Pipeline { pipeline } => {
      json!({ "kind": "pipeline", "pipeline": { "id": pipeline.to_string(), "name": pipeline_name(*pipeline) } })
    }
    InstructionScope::Step { pipeline, step } => json!({
      "kind": "step",
      "pipeline": { "id": pipeline.to_string(), "name": pipeline_name(*pipeline) },
      "step": step.as_str(),
    }),
  };
  let agents = match body.targets() {
    AgentTargets::All => json!({ "kind": "all" }),
    AgentTargets::Roles(roles) => {
      json!({ "kind": "roles", "roles": roles.iter().map(Name::as_str).collect::<Vec<_>>() })
    }
    AgentTargets::Profiles(profiles) => {
      json!({ "kind": "profiles", "profiles": profiles.iter().map(ToString::to_string).collect::<Vec<_>>() })
    }
  };
  let events: BTreeSet<&str> = body
    .events()
    .iter()
    .map(|event| match event {
      InjectionEvent::Start => "start",
      InjectionEvent::Resume => "resume",
    })
    .collect();
  let state = match entry.state {
    VersionState::Draft { .. } => "draft",
    VersionState::StaleDraft { .. } => "stale_draft",
    VersionState::Published(_) => "published",
  };
  json!({
    "id": entry.id.to_string(),
    "name": &*entry.name,
    "applies_in": applies_in,
    "agents": agents,
    "events": events,
    "text": body.text().as_str(),
    "state": state,
    "published_version": entry.state.published().map(|version| version.get()),
  })
}

fn write_error(error: &WriteError) -> ToolError {
  let code = match error {
    WriteError::Unavailable(_) => "variable_unavailable",
    WriteError::Store(StoreError::NoDraft(_)) => return no_draft(),
    WriteError::Store(StoreError::DraftChanged(_)) => "draft_changed",
    WriteError::Store(StoreError::Publish(PublishError::BlankInstruction)) => "blank_text",
    WriteError::Store(StoreError::Publish(PublishError::StaleDraft { .. })) => "stale_draft",
    WriteError::Store(StoreError::Publish(PublishError::VersionLimit)) => "version_limit",
    WriteError::Store(_) => "project_data",
  };
  ToolError::refused(code, vec![error.message()])
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
