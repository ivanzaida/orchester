//! `save_pipeline_draft`: a pipeline's editor through Orchester's MCP server. The steps given replace
//! the draft's, each field checked as the step inspector checks it, and only what the inspector offers
//! is taken: published profiles and runnable tools, write access where the profile writes, and a
//! narrowing of the profile's permissions only where its CLI enforces the narrower choice. Steps that
//! break a pipeline rule (an unknown step, a cycle, a return to a step that does not run before) are
//! not saved, as the editor keeps no such draft.

use std::num::NonZeroU32;

use orchester_core::{
  connection::invocation::Platform,
  permission::{
    AllowedCommands, CommandPermission, NetworkAccess, StepCommands, StepNetwork, StepPermissions,
    capability::{PermissionOption, support},
  },
  resource::{DraftSaved, FailureRoute, ModelId, PipelineEdit, RepositoryAccess, Step, StepKey, StepKind},
  tokens::{BudgetInputError, TokenBudget},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::Translate,
  pipelines::{opened, pipeline_json, pipeline_named},
  profile_input::Access,
  widening,
};
use crate::{
  agents::permissions::with_command,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  budget::budget_error,
  drafts::save_draft_in,
  messages::Message,
  pipelines::{
    data::{Assignables, ProfileChoice},
    fields::{parse_instructions, parse_name},
    problems::{problem_view, rule_message},
  },
  tasks::outside::Outside,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SavePipelineDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The pipeline's id, or its name.
  pub pipeline: String,
  /// What the pipeline is for; left out, it stays as it is.
  pub description: Option<String>,
  /// Every step, in the order the editor shows them.
  pub steps: Vec<StepInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct StepInput {
  /// The step's key: small letters, digits and -; other steps and instructions name it.
  pub key: String,
  pub name: String,
  /// What the step's agent or tool is asked to do; may use {{variables}}.
  #[serde(default)]
  pub instructions: String,
  /// The published agent profile that runs the step, by id or name.
  pub profile: Option<String>,
  /// The published tool the step calls instead, by id or name.
  pub tool: Option<String>,
  /// Keys of the steps it runs after.
  #[serde(default)]
  pub runs_after: Vec<String>,
  /// The step's result waits for a person's approval (a plan, for instance).
  #[serde(default)]
  pub approval_gate: bool,
  /// Where the run returns when this step fails.
  pub on_failure: Option<FailureInput>,
  /// Tokens the step may use; left out, no step budget.
  pub budget: Option<u64>,
  /// An agent step's repository access; read_only when left out.
  pub repository: Option<Access>,
  /// An agent step's model instead of its profile's.
  pub model: Option<String>,
  /// An agent step's narrowing of its profile's permissions.
  pub narrow: Option<NarrowInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct FailureInput {
  /// The key of an earlier step this one runs after.
  pub return_to: String,
  /// How often it returns before the run waits for a person; 1 or more.
  pub retries: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct NarrowInput {
  /// Only these of the profile's allowed commands.
  pub only_commands: Option<Vec<String>>,
  /// No network for this step, where the profile has it.
  #[serde(default)]
  pub network_off: bool,
  /// Ask a person about each MCP or app tool call of this step.
  #[serde(default)]
  pub ask_before_tool_calls: bool,
}

pub async fn save_pipeline_draft(
  services: &Services,
  args: SavePipelineDraft,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let then = translate.text(&Message::new("pipelines.flow.then"));
  let (opened, saved) = blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let id = pipeline_named(&db, &args.pipeline)?;
    let current = opened(&db, id, &args.pipeline)?;
    let steps = args
      .steps
      .iter()
      .map(|input| step(input, &current.assignables))
      .collect::<Result<Vec<_>, _>>()?;
    let description = args
      .description
      .unwrap_or_else(|| current.body.description().to_owned());
    let edit = PipelineEdit::from_steps(&description, steps);
    // The editor keeps no draft whose steps break a rule; steps without a profile or tool only wait to
    // be published.
    let broken: Vec<Message> = edit
      .problems()
      .iter()
      .filter(|problem| problem.blocks_saving())
      .map(|problem| problem_view(&edit, problem, &then).message)
      .collect();
    if !broken.is_empty() {
      return Err(ToolError::refused("invalid_pipeline", broken));
    }
    let body = edit
      .validate()
      .map_err(|error| ToolError::refused("invalid_pipeline", vec![rule_message(&edit, &error)]))?;
    // Neither the published version's approvals nor those a person added to the draft since are dropped.
    let mut dropped = weakened_gates(current.body.steps(), body.steps());
    if let Some(latest) = &current.latest {
      for reason in weakened_gates(latest.body.steps(), body.steps()) {
        if !dropped.contains(&reason) {
          dropped.push(reason);
        }
      }
    }
    widening::allowed(&services, dropped)?;
    let saved = save_draft_in(&open.repo_root, id, body)?;
    services.changes.record(open.project.id(), Outside::Pipelines);
    Ok((opened(&ProjectDb::open(&open.repo_root)?, id, &args.pipeline)?, saved))
  })
  .await?;
  let mut value = pipeline_json(&opened, translate);
  value["draft_kept"] = json!(saved == DraftSaved::Stored);
  Ok(json!({ "pipeline": value }))
}

/// The approvals `steps` drop from `base`, the latest published version (or, before the first, the draft as
/// it stands): a step that waits for a person's approval, or asks a person about its tool calls, and no
/// longer does or is gone. Dropping one is a person's decision; other narrowing within the profile is not.
fn weakened_gates(base: &[Step], steps: &[Step]) -> Vec<Message> {
  let asks =
    |step: &Step| matches!(&step.kind, StepKind::Agent { permissions, .. } if permissions.interactive_tool_approvals);
  let mut reasons = Vec::new();
  for old in base {
    let new = steps.iter().find(|step| step.key == old.key);
    let named = |key: &'static str| Message::new(key).with("step", old.name.as_str().to_owned());
    if old.approval_gate && !new.is_some_and(|step| step.approval_gate) {
      reasons.push(named("appMcp.widens.approvalGate"));
    }
    if asks(old) && !new.is_some_and(asks) {
      reasons.push(named("appMcp.widens.stepToolCalls"));
    }
  }
  reasons
}

/// The step `input` describes, or why the inspector would not take it, naming the step.
fn step(input: &StepInput, assignables: &Assignables) -> Result<Step, ToolError> {
  let invalid = |reason: Message| {
    let at = Message::new("appMcp.errors.inStep").with("step", input.key.clone());
    ToolError::refused("invalid_step", vec![at, reason])
  };
  let key = step_key(&input.key).map_err(invalid)?;
  let depends_on = input
    .runs_after
    .iter()
    .map(|key| step_key(key))
    .collect::<Result<_, _>>()
    .map_err(invalid)?;
  let on_failure = input
    .on_failure
    .as_ref()
    .map(|route| {
      let retries = NonZeroU32::new(route.retries).ok_or_else(|| Message::new("pipelines.problems.invalidRetries"))?;
      Ok(FailureRoute {
        return_to: step_key(&route.return_to)?,
        max_retries: retries,
      })
    })
    .transpose()
    .map_err(invalid)?;
  let budget = input
    .budget
    .map(|tokens| TokenBudget::new(tokens).map_err(|_| budget_error(BudgetInputError::Zero, "")))
    .transpose()
    .map_err(invalid)?;
  Ok(Step {
    key,
    name: parse_name(&input.name).map_err(invalid)?,
    kind: kind(input, assignables).map_err(invalid)?,
    instructions: parse_instructions(&input.instructions).map_err(invalid)?,
    depends_on,
    approval_gate: input.approval_gate,
    on_failure,
    budget,
  })
}

fn step_key(key: &str) -> Result<StepKey, Message> {
  StepKey::new(key).map_err(|_| Message::new("appMcp.errors.stepKeyInvalid").with("key", key.to_owned()))
}

/// What runs the step: a published profile with what the step may change of it, or a runnable tool.
fn kind(input: &StepInput, assignables: &Assignables) -> Result<StepKind, Message> {
  match (&input.profile, &input.tool) {
    (Some(_), Some(_)) => Err(Message::new("appMcp.errors.profileOrTool")),
    (None, Some(tool)) => {
      if input.repository.is_some() || input.model.is_some() || input.narrow.is_some() {
        return Err(Message::new("appMcp.errors.toolStepOptions"));
      }
      let lowered = tool.to_lowercase();
      let choice = assignables
        .tools
        .iter()
        .find(|choice| choice.id.to_string() == *tool || choice.name.to_lowercase() == lowered)
        .ok_or_else(|| Message::new("appMcp.errors.toolNotPublished").with("tool", tool.clone()))?;
      if !choice.runnable {
        return Err(Message::new("appMcp.errors.toolNotRunnable").with("tool", tool.clone()));
      }
      Ok(StepKind::Tool { tool: Some(choice.id) })
    }
    (profile, None) => {
      let choice = profile
        .as_deref()
        .map(|named| profile_choice(assignables, named))
        .transpose()?;
      agent(input, choice)
    }
  }
}

fn profile_choice<'a>(assignables: &'a Assignables, named: &str) -> Result<&'a ProfileChoice, Message> {
  assignables
    .profiles
    .iter()
    .find(|choice| choice.id.to_string() == named)
    .or_else(|| assignables.profile_named(named).and_then(|id| assignables.profile(id)))
    .ok_or_else(|| Message::new("appMcp.errors.profileNotPublished").with("profile", named.to_owned()))
}

/// An agent step on `choice` (none: unassigned), with the access, model and narrowing given.
fn agent(input: &StepInput, choice: Option<&ProfileChoice>) -> Result<StepKind, Message> {
  let repository = input
    .repository
    .map_or(RepositoryAccess::ReadOnly, RepositoryAccess::from);
  let writes = choice.is_some_and(|choice| choice.permissions.repository() == RepositoryAccess::Write);
  if repository == RepositoryAccess::Write && !writes {
    return Err(Message::new("appMcp.errors.stepCannotWrite"));
  }
  let model = input
    .model
    .as_deref()
    .map(|model| ModelId::new(model.trim()).map_err(|_| Message::new("errors.modelInvalid")))
    .transpose()?;
  let permissions = match (&input.narrow, choice) {
    (None, _) => StepPermissions::default(),
    (Some(_), None) => return Err(Message::new("appMcp.errors.narrowNeedsProfile")),
    (Some(narrow), Some(choice)) => narrowed(narrow, choice)?,
  };
  Ok(StepKind::Agent {
    profile: choice.map(|choice| choice.id),
    repository,
    model,
    permissions,
  })
}

/// The narrowing `narrow` asks for, where the inspector offers it on `choice`: some of an allowed list
/// its CLI enforces, network off where it has network and its CLI can block it.
fn narrowed(narrow: &NarrowInput, choice: &ProfileChoice) -> Result<StepPermissions, Message> {
  let enforced = |option| {
    choice
      .provider
      .is_some_and(|provider| support(provider, Platform::current(), option).is_enforced())
  };
  let mut permissions = StepPermissions {
    interactive_tool_approvals: narrow.ask_before_tool_calls,
    ..StepPermissions::default()
  };
  if let Some(commands) = &narrow.only_commands {
    let CommandPermission::Allowed(allowed) = choice.permissions.commands() else {
      return Err(Message::new("appMcp.errors.narrowCommandsNotOffered"));
    };
    if allowed.entries().is_empty() || !enforced(PermissionOption::AllowedCommands) {
      return Err(Message::new("appMcp.errors.narrowCommandsNotOffered"));
    }
    let mut only = AllowedCommands::default();
    for command in commands {
      only = with_command(&only, command)?;
      if only
        .entries()
        .last()
        .is_some_and(|added| !allowed.entries().contains(added))
      {
        return Err(Message::new("appMcp.errors.commandNotInProfile").with("command", command.clone()));
      }
    }
    // Keeping every command of the profile narrows nothing, as the inspector's toggles have it.
    if only.entries().len() < allowed.entries().len() {
      permissions.commands = StepCommands::Only(only);
    }
  }
  if narrow.network_off {
    if choice.permissions.network() != NetworkAccess::On || !enforced(PermissionOption::NetworkOff) {
      return Err(Message::new("appMcp.errors.networkOffNotOffered"));
    }
    permissions.network = StepNetwork::Off;
  }
  Ok(permissions)
}
