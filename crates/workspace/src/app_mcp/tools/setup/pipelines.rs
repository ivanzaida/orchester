//! `get_pipeline` and `create_pipeline`: a pipeline as its editor opens it, its latest version (a pipeline is always a
//! published version, D-032); and a new one made from the starter, a copy of another pipeline, or the steps given,
//! published as v1 or refused with nothing stored.

use std::path::Path;

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  permission::{StepCommands, StepNetwork},
  resource::{Pipeline, PipelineEdit, PromptText, StarterStep, StarterStepText, Step, StepKind},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Translate, pipeline_copy::create_error, wire::Delivers},
  pipeline_steps::{Kept, StepInput, body_of},
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::Link,
  messages::{self, Message},
  pipelines::{
    data::{Assignables, OpenError, OpenedPipeline},
    library::{StarterText, create_with},
    problems::problem_view,
    widening::save_reasons,
    write::save_refusals,
  },
  tasks::outside::Outside,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InPipeline {
  /// The project's id, or its name.
  pub project: String,
  /// The pipeline's id, or its name.
  pub pipeline: String,
}

/// What a new pipeline starts with: the starter, a copy, or nothing but the steps given.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Start {
  Starter,
  Empty,
  Copy,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreatePipeline {
  /// The project's id, or its name.
  pub project: String,
  /// The new pipeline's name; it must give a slug no other pipeline has.
  pub name: String,
  pub from: Start,
  /// With `copy`: the pipeline to copy, by its id or name.
  pub copy_of: Option<String>,
  /// With `empty`: every step, in the order the editor shows them, each as `save_pipeline` takes it.
  pub steps: Option<Vec<StepInput>>,
  /// With `empty`: what the pipeline is for.
  pub description: Option<String>,
  /// What its runs deliver: changes (repository changes, merged), report (the last step's report), outside (work done
  /// through MCP tools outside the repository) or changes_or_report; left out, changes (a copy: as its source).
  pub delivers: Option<Delivers>,
}

pub async fn get_pipeline(services: &Services, args: InPipeline, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, opened) = blocking(move || {
    let (open, _) = services.read_project(&args.project)?;
    let id = pipeline_named(&services, &open.repo_root, &args.pipeline)?;
    Ok((
      open.project.id(),
      opened(&services, &open.repo_root, id, &args.pipeline)?,
    ))
  })
  .await?;
  Ok(json!({ "pipeline": pipeline_json(&opened, project, translate) }))
}

pub async fn create_pipeline(
  services: &Services,
  args: CreatePipeline,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let starter = starter_text(translate);
  let then = translate.text(&Message::new("pipelines.flow.then"));
  let (project, opened) = blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let name = Name::new(&args.name).map_err(|error| refused("invalid_name", messages::name_error(&error)))?;
    let assignables = assignables(&services, &open.repo_root, &db)?;
    let mut body = new_body((&services, &open.repo_root), &args, (&starter, &assignables, &then))?;
    if let Some(delivers) = args.delivers {
      body = body.with_delivers(delivers.into());
    }
    // Creating publishes v1 (D-032): a body the save check refuses is refused first, whoever asks; then what a first
    // version widens, as there is nothing to compare it with, and the tool steps it adds, need a person.
    let refusals = save_refusals(&body, &assignables, None);
    if !refusals.is_empty() {
      return Err(ToolError::refused("not_publishable", refusals));
    }
    widening::allowed(&services, save_reasons(None, &body, &assignables))?;
    let id = create_with(
      &mut db,
      (&open.repo_root, &services.servers),
      name,
      &body,
      Timestamp::now(),
    )
    .map_err(|error| create_error(&error))?;
    // The library lists the version, and the task form offers it.
    services.changes.record(open.project.id(), Outside::Pipelines);
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok((open.project.id(), opened(&services, &open.repo_root, id, &args.name)?))
  })
  .await?;
  Ok(json!({ "pipeline": pipeline_json(&opened, project, translate) }))
}

/// The body `args` ask the new pipeline to start with: the starter's steps with what `assignables` offer, a copy of
/// another pipeline's latest version, or the steps given (`empty`, no steps at all when left out).
fn new_body(
  (services, root): (&Services, &Path),
  args: &CreatePipeline,
  (starter, assignables, then): (&StarterText, &Assignables, &str),
) -> Result<Pipeline, ToolError> {
  if args.from != Start::Empty && (args.steps.is_some() || args.description.is_some()) {
    return Err(refused("steps_not_taken", Message::new("appMcp.errors.stepsNotTaken")));
  }
  match (args.from, &args.copy_of) {
    (Start::Copy, Some(named)) => {
      let source = pipeline_named(services, root, named)?;
      Ok(opened(services, root, source, named)?.body)
    }
    (Start::Copy, None) => Err(refused(
      "copy_of_required",
      Message::new("appMcp.errors.copyOfRequired"),
    )),
    (_, Some(_)) => Err(refused(
      "copy_of_not_taken",
      Message::new("appMcp.errors.copyOfNotTaken"),
    )),
    (Start::Starter, None) => Ok(starter.pipeline(assignables)),
    (Start::Empty, None) => {
      let empty = Pipeline::new(Vec::new()).expect("a pipeline without steps is a valid body");
      let kept = Kept {
        description: args.description.as_deref().unwrap_or_default(),
        sources: empty.sources().clone(),
        deliver: None,
        delivers: empty.delivers(),
      };
      body_of(args.steps.as_deref().unwrap_or_default(), kept, (assignables, then))
    }
  }
}

/// The pipeline of the project at `root` named by its id or, ignoring case, its name: a team project's among the
/// team's, as this session read them, a local project's among its own.
pub(crate) fn pipeline_named(services: &Services, root: &Path, named: &str) -> Result<ResourceId, ToolError> {
  let lowered = named.to_lowercase();
  let library = services.workspace().pipeline_library(root)?;
  // An id names one pipeline for certain; a name could also be another pipeline's id.
  let by_id = library.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| library.iter().find(|entry| entry.name.to_lowercase() == lowered))
    .map(|entry| entry.id)
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, named))
}

/// Pipeline `id` of the project at `root` as the editor opens it: a team project's from the team's configuration this
/// session read, a local project's from its database.
pub(crate) fn opened(
  services: &Services,
  root: &Path,
  id: ResourceId,
  named: &str,
) -> Result<OpenedPipeline, ToolError> {
  services
    .workspace()
    .open_pipeline(root, id)
    .map_err(|error| match error {
      OpenError::Store(error) => ToolError::from(error),
      OpenError::Missing => ToolError::not_found(Missing::Pipeline, named),
      OpenError::TeamNotRead(unread) => ToolError::from(unread),
    })
}

/// What the steps of a pipeline of the project at `root`, whose database is `db`, can run: a local project's published
/// profiles and tools, a team project's from the team's configuration this session read.
pub(crate) fn assignables(services: &Services, root: &Path, db: &ProjectDb) -> Result<Assignables, ToolError> {
  if db.sharing()?.is_none() {
    return Ok(Assignables::load(db)?);
  }
  let config = services.workspace().team_read(root).loaded()?;
  Ok(Assignables::of_team(db, &config)?)
}

/// The starter's steps in the app's words, and the profiles and tools it looks for.
fn starter_text(translate: &Translate) -> StarterText {
  StarterText::new(|step| {
    let [name, instructions, uses] = starter_keys(step).map(|key| translate.text(&Message::new(key)));
    let text = StarterStepText {
      name: Name::new(&name).expect("the starter's step names in the locale files are not blank"),
      instructions: PromptText::new(&instructions)
        .expect("the starter's instructions in the locale files use no placeholders"),
    };
    (text, uses)
  })
}

/// The keys of a starter step's name, instructions and the profile or tool it looks for.
const fn starter_keys(step: StarterStep) -> [&'static str; 3] {
  match step {
    StarterStep::Research => [
      "pipelines.starter.research.name",
      "pipelines.starter.research.instructions",
      "pipelines.starter.research.uses",
    ],
    StarterStep::Plan => [
      "pipelines.starter.plan.name",
      "pipelines.starter.plan.instructions",
      "pipelines.starter.plan.uses",
    ],
    StarterStep::Implement => [
      "pipelines.starter.implement.name",
      "pipelines.starter.implement.instructions",
      "pipelines.starter.implement.uses",
    ],
    StarterStep::RunChecks => [
      "pipelines.starter.runChecks.name",
      "pipelines.starter.runChecks.instructions",
      "pipelines.starter.runChecks.uses",
    ],
    StarterStep::Review => [
      "pipelines.starter.review.name",
      "pipelines.starter.review.instructions",
      "pipelines.starter.review.uses",
    ],
    StarterStep::CreatePullRequest => [
      "pipelines.starter.createPullRequest.name",
      "pipelines.starter.createPullRequest.instructions",
      "pipelines.starter.createPullRequest.uses",
    ],
  }
}

/// A pipeline of `project` as `get_pipeline` shows it, with its link.
pub(super) fn pipeline_json(opened: &OpenedPipeline, project: ProjectId, translate: &Translate) -> Value {
  let edit = PipelineEdit::new(opened.body.clone());
  let then = translate.text(&Message::new("pipelines.flow.then"));
  let problems: Vec<String> = edit
    .problems()
    .iter()
    .map(|problem| translate.text(&problem_view(&edit, problem, &then).message))
    .collect();
  let steps: Vec<Value> = opened
    .body
    .steps()
    .iter()
    .map(|step| step_json(step, &opened.body, &opened.assignables))
    .collect();
  // The Deliver step the pipeline ends in (ORC-19): its runs join the project's merge queue; null without one.
  let deliver = opened.body.deliver().map(|deliver| {
    json!({
      "mode": deliver.mode(),
      "method": deliver.method(),
      "integrator": deliver.integrator().map(|profile| profile.to_string()),
      "recheck": deliver.recheck(),
    })
  });
  json!({
    "id": opened.id.to_string(),
    "name": &*opened.name,
    "description": opened.body.description(),
    // A pipeline is stored only as published versions (D-032).
    "state": "published",
    "published_version": opened.version.get(),
    "steps": steps,
    "deliver": deliver,
    "delivers": Delivers::from(opened.body.delivers()),
    "problems": problems,
    "assignable": assignable_json(&opened.assignables),
    "link": Link::Pipeline(project, opened.id).path(),
  })
}

/// `step` of `body`, with what it may change of its profile and whether it merges the base branch when that moves:
/// its own choice (`takes_base_moves`, null for the default rule) and the outcome (`merges_moved_base`).
fn step_json(step: &Step, body: &Pipeline, assignables: &Assignables) -> Value {
  let mut value = json!({
    "key": step.key.as_str(),
    "name": step.name.as_str(),
    "instructions": step.instructions.as_str(),
    "runs_after": step.depends_on.iter().map(|key| key.as_str()).collect::<Vec<_>>(),
    "approval_gate": step.approval_gate,
    "on_failure": step.on_failure.as_ref().map(|route| json!({
      "return_to": route.return_to.as_str(),
      "retries": route.max_retries.get(),
    })),
    "budget": step.budget.map(|budget| budget.get()),
  });
  let executor = match &step.kind {
    StepKind::Agent {
      profile,
      repository,
      model,
      permissions,
    } => {
      let only = match &permissions.commands {
        StepCommands::AsProfile => None,
        StepCommands::Only(list) => Some(list.entries().iter().map(ToString::to_string).collect::<Vec<_>>()),
      };
      json!({
        "profile": profile.map(|id| named(id, assignables.profile(id).map(|choice| &*choice.name))),
        "repository": repository,
        "model": model.as_ref().map(|model| model.as_str()),
        "narrow": {
          "only_commands": only,
          "network_off": permissions.network == StepNetwork::Off,
          "ask_before_tool_calls": permissions.interactive_tool_approvals,
        },
        "takes_base_moves": permissions.takes_base_moves,
        "merges_moved_base": body.takes_base_moves(&step.key),
      })
    }
    StepKind::Tool { tool } => json!({
      "tool": tool.map(|id| named(id, assignables.tool(id).map(|choice| &*choice.name))),
    }),
  };
  if let (Value::Object(step), Value::Object(executor)) = (&mut value, executor) {
    step.extend(executor);
  }
  value
}

fn named(id: ResourceId, name: Option<&str>) -> Value {
  json!({ "id": id.to_string(), "name": name })
}

/// The published profiles and tools steps can use.
fn assignable_json(assignables: &Assignables) -> Value {
  let profiles: Vec<Value> = assignables
    .profiles
    .iter()
    .map(|profile| json!({ "id": profile.id.to_string(), "name": &*profile.name, "version": profile.version.get() }))
    .collect();
  let tools: Vec<Value> = assignables
    .tools
    .iter()
    .map(|tool| json!({ "id": tool.id.to_string(), "name": &*tool.name, "runnable": tool.runnable }))
    .collect();
  json!({ "profiles": profiles, "tools": tools })
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
