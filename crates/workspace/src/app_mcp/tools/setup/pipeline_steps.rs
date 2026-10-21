//! `save_pipeline_draft`: a pipeline's editor through Orchester's MCP server. The steps given replace
//! the draft's, each field checked as the step inspector checks it, and only what the inspector offers
//! is taken: published profiles and runnable tools, write access where the profile writes, and a
//! narrowing of the profile's permissions only where its CLI enforces the narrower choice. Steps that
//! break a pipeline rule (an unknown step, a cycle, a return to a step that does not run before) are
//! not saved, as the editor keeps no such draft.

use std::num::NonZeroU32;

use orchester_core::{
  permission::StepPermissions,
  resource::{Draft, DraftSaved, FailureRoute, ModelId, PipelineEdit, RepositoryAccess, Step, StepKey, StepKind},
  time::Timestamp,
  tokens::{BudgetInputError, TokenBudget},
};
use orchester_store::{ProjectDb, StoreError};
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
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  budget::budget_error,
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
  /// Whether an agent step's agent is told to merge the base branch when it moves (true) or never (false); left
  /// out, the default rule: a step with repository write that does not review or verify (no `review` or `verify` in
  /// its key, no failure route back to a writing step).
  pub takes_base_moves: Option<bool>,
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
  let (project, opened, saved) = blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
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
    // The pipeline keeps the sources it lists, those of the steps it still has.
    let edit = PipelineEdit::from_steps(&description, current.body.sources().clone(), steps);
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
    let dropped = crate::pipelines::widening::draft_reasons(&current, &body);
    widening::allowed(&services, dropped)?;
    let draft = Draft::new(body, current.based_on, Timestamp::now());
    let saved = db
      .save_draft_matching(id, &current.body, current.latest.as_ref().map(|v| v.number), &draft)
      .map_err(|error| match &error {
        StoreError::DraftChanged(_) => ToolError::refused("draft_changed", vec![crate::messages::store_error(&error)]),
        _ => error.into(),
      })?;
    services.changes.record(open.project.id(), Outside::Pipelines);
    Ok((
      open.project.id(),
      opened(&ProjectDb::open(&open.repo_root)?, id, &args.pipeline)?,
      saved,
    ))
  })
  .await?;
  let mut value = pipeline_json(&opened, project, translate);
  value["draft_kept"] = json!(saved == DraftSaved::Stored);
  Ok(json!({ "pipeline": value }))
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
      if input.repository.is_some()
        || input.model.is_some()
        || input.narrow.is_some()
        || input.takes_base_moves.is_some()
      {
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
  let narrowing = match (&input.narrow, choice) {
    (None, _) => StepPermissions::default(),
    (Some(_), None) => return Err(Message::new("appMcp.errors.narrowNeedsProfile")),
    (Some(narrow), Some(choice)) => narrowed(narrow, choice)?,
  };
  Ok(StepKind::Agent {
    profile: choice.map(|choice| choice.id),
    repository,
    model,
    permissions: StepPermissions {
      takes_base_moves: input.takes_base_moves,
      ..narrowing
    },
  })
}

fn narrowed(narrow: &NarrowInput, choice: &ProfileChoice) -> Result<StepPermissions, Message> {
  crate::pipelines::narrowing::permissions(
    narrow.only_commands.as_deref(),
    narrow.network_off,
    narrow.ask_before_tool_calls,
    choice,
  )
}
