//! `get_pipeline`, `create_pipeline` and `publish_pipeline`: a pipeline as its editor opens it, a new
//! one made as New pipeline makes it (from the starter, empty, or a copy of another pipeline), and its
//! draft published as Publish does. The task form offers a pipeline once it is published.

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  permission::{StepCommands, StepNetwork},
  resource::{
    Deliverable, Draft, Pipeline, PipelineEdit, PromptText, PublishError, StarterStep, StarterStepText, Step, StepKind,
    Version,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Translate, pipeline_copy::create_error, wire::Delivers},
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
    library::{self, NewPipeline, StartFrom, StarterText, create_with},
    problems::{problem_view, publish_error},
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

/// What a new pipeline starts with.
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
  /// What its runs deliver: changes (repository changes, merged), report (the last step's report), outside (work done
  /// through MCP tools outside the repository) or changes_or_report; left out, changes (a copy: as its source).
  pub delivers: Option<Delivers>,
}

pub async fn get_pipeline(services: &Services, args: InPipeline, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, opened) = blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let id = pipeline_named(&db, &args.pipeline)?;
    Ok((open.project.id(), opened(&db, id, &args.pipeline)?))
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
  let (project, opened) = blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let name = Name::new(&args.name).map_err(|error| refused("invalid_name", messages::name_error(&error)))?;
    let id = match (args.from, &args.copy_of) {
      (Start::Copy, Some(named)) => {
        let source = pipeline_named(&db, named)?;
        let body = opened(&db, source, named)?.body;
        create_with(&mut db, name, &body, Timestamp::now())
      }
      (Start::Copy, None) => {
        return Err(refused(
          "copy_of_required",
          Message::new("appMcp.errors.copyOfRequired"),
        ));
      }
      (_, Some(_)) => {
        return Err(refused(
          "copy_of_not_taken",
          Message::new("appMcp.errors.copyOfNotTaken"),
        ));
      }
      (start, None) => {
        let start = if start == Start::Starter {
          StartFrom::Starter
        } else {
          StartFrom::Empty
        };
        library::create(&mut db, NewPipeline { name, start, starter }, Timestamp::now())
      }
    }
    .map_err(|error| create_error(&error))?;
    if let Some(delivers) = args.delivers {
      set_delivers(&mut db, (id, &args.name), delivers.into())?;
    }
    services.changes.record(open.project.id(), Outside::Pipelines);
    Ok((open.project.id(), opened(&db, id, &args.name)?))
  })
  .await?;
  Ok(json!({ "pipeline": pipeline_json(&opened, project, translate) }))
}

/// The draft of the new pipeline `id` (`named`), its runs delivering `delivers`.
fn set_delivers(db: &mut ProjectDb, (id, named): (ResourceId, &str), delivers: Deliverable) -> Result<(), ToolError> {
  let current = opened(db, id, named)?;
  if current.body.delivers() == delivers {
    return Ok(());
  }
  let draft = Draft::new(
    current.body.clone().with_delivers(delivers),
    current.based_on,
    Timestamp::now(),
  );
  db.save_draft_matching(id, &current.body, current.latest.as_ref().map(|v| v.number), &draft)?;
  Ok(())
}

pub async fn publish_pipeline(
  services: &Services,
  args: InPipeline,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, opened) = blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let id = pipeline_named(&db, &args.pipeline)?;
    let draft = db.draft::<Pipeline>(id)?.ok_or(StoreError::NoDraft(id));
    let draft = draft.map_err(|error| refused(publish_code(&error), publish_error(&error)))?;
    let latest = db.latest_version::<Pipeline>(id)?;
    services.draft_decided::<Pipeline>(&db, id)?;
    widening::check_pipeline(&services, latest.as_ref().map(Version::body), draft.body())?;
    // Published only while the draft is the one just checked.
    db.publish_draft_matching_version(id, draft.body(), latest.as_ref().map(Version::number), Timestamp::now())
      .map_err(|error| refused(publish_code(&error), publish_error(&error)))?;
    // The library lists the version, and the task form offers it.
    services.changes.record(open.project.id(), Outside::Pipelines);
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok((
      open.project.id(),
      opened(&ProjectDb::open(&open.repo_root)?, id, &args.pipeline)?,
    ))
  })
  .await?;
  Ok(json!({ "pipeline": pipeline_json(&opened, project, translate) }))
}

/// The project's pipeline named by its id or, ignoring case, its name, whether published or not.
pub(super) fn pipeline_named(db: &ProjectDb, named: &str) -> Result<ResourceId, ToolError> {
  let lowered = named.to_lowercase();
  let library = library::library(db)?;
  // An id names one pipeline for certain; a name could also be another pipeline's id.
  let by_id = library.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| library.iter().find(|entry| entry.name.to_lowercase() == lowered))
    .map(|entry| entry.id)
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, named))
}

pub(super) fn opened(db: &ProjectDb, id: ResourceId, named: &str) -> Result<OpenedPipeline, ToolError> {
  OpenedPipeline::load(db, id).map_err(|error| match error {
    OpenError::Store(error) => ToolError::from(error),
    OpenError::Missing => ToolError::not_found(Missing::Pipeline, named),
  })
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
  let state = match (opened.draft_saved_at, &opened.latest) {
    (Some(_), None) => "draft_only",
    (Some(_), Some(_)) => "published_with_draft",
    (None, _) => "published",
  };
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
    "state": state,
    "published_version": opened.latest.as_ref().map(|latest| latest.number.get()),
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

fn publish_code(error: &StoreError) -> &'static str {
  match error {
    StoreError::NoDraft(_) => "no_draft",
    StoreError::Publish(PublishError::StaleDraft { .. }) => "stale_draft",
    StoreError::DraftChanged(_) => "draft_changed",
    StoreError::Publish(PublishError::NoSteps | PublishError::UnassignedSteps(_)) => "not_publishable",
    StoreError::Publish(PublishError::VersionLimit) => "version_limit",
    _ => "project_data",
  }
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
