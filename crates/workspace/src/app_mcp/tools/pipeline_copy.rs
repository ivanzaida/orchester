//! `duplicate_pipeline`: what a person does in Pipelines with *New pipeline*, the editor and
//! *Publish*: a new pipeline under a new name with the steps of another pipeline's latest published
//! version, some agent steps given another published profile, checked as the editor checks its
//! draft and published as v1. A step keeps its repository access, model and permission narrowing when
//! its profile changes, as the editor's profile choice does.

use std::collections::BTreeMap;

use orchester_core::{
  name::Name,
  resource::{Pipeline, PipelineEdit, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Translate, projects::pipeline_json, task_write::pipeline_named};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::{self, Message},
  pipelines::{
    data::{Assignables, publish_in},
    fields::Executor,
    library::{CreateError, create_with},
    problems::{problem_view, publish_error, rule_message},
  },
  tasks::{
    data::{pinned_pipeline, published_pipelines},
    outside::Outside,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DuplicatePipeline {
  /// The project's id, or its name.
  pub project: String,
  /// The pipeline to copy, by its id or name; its latest published version is copied.
  pub pipeline: String,
  /// The new pipeline's name.
  pub name: String,
  /// Agent steps of the copy to run with another published agent profile: the step (its key, its
  /// name, or its number from 1 in pipeline order) to the profile's name.
  #[serde(default)]
  pub step_profiles: BTreeMap<String, String>,
}

pub async fn duplicate_pipeline(
  services: &Services,
  args: DuplicatePipeline,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let then = translate.text(&Message::new("pipelines.flow.then"));
  let (project, copy) = blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let mut db = ProjectDb::open(&root)?;
    let (source, body) = copied(&db, &args, &then)?;
    super::setup::widening::check_pipeline(&services, Some(&source), &body)?;
    let name =
      Name::new(&args.name).map_err(|error| ToolError::refused("invalid_name", vec![messages::name_error(&error)]))?;
    let id = create_with(&mut db, name, &body, Timestamp::now()).map_err(|error| create_error(&error))?;
    drop(db);
    let published = publish_in(&root, id);
    // The pipeline was made either way: the library lists it, published or as a draft.
    services.changes.record(open.project.id(), Outside::Pipelines);
    let (version, _) = published.map_err(|error| ToolError::refused("not_published", vec![publish_error(&error)]))?;
    // The task form offers the published version.
    services.changes.record(open.project.id(), Outside::Tasks);
    let copy = pinned_pipeline(&ProjectDb::open(&root)?, id, version)?
      .ok_or_else(|| ToolError::not_found(Missing::Pipeline, id.to_string()))?;
    Ok((open.project.id(), copy))
  })
  .await?;
  Ok(json!({ "pipeline": pipeline_json(&copy, project, translate) }))
}

/// The steps of the copy: the source's latest published version with the profiles given, refused
/// as the editor refuses publishing while it has problems.
/// The source's latest published version, and the copy made of it.
fn copied(db: &ProjectDb, args: &DuplicatePipeline, then: &str) -> Result<(Pipeline, Pipeline), ToolError> {
  let published = published_pipelines(db)?;
  let source = pipeline_named(&published, &args.pipeline)?;
  let version = db
    .latest_version::<Pipeline>(source.id)?
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, &args.pipeline))?;
  let assignables = Assignables::load(db)?;
  let mut edit = PipelineEdit::new(version.body().clone());
  for (step, profile) in &args.step_profiles {
    let key = step_named(&edit, step)?;
    if !matches!(edit.step(&key).map(|step| &step.kind), Some(StepKind::Agent { .. })) {
      return Err(ToolError::refused(
        "not_an_agent_step",
        vec![Message::new("appMcp.errors.notAnAgentStep").with("step", step.clone())],
      ));
    }
    let id = assignables
      .profile_named(profile)
      .ok_or_else(|| ToolError::not_found(Missing::Profile, profile))?;
    edit.update(&key, |step| Executor::Profile(id).apply(step));
  }
  let problems = edit.problems();
  if !problems.is_empty() {
    let reasons = problems
      .iter()
      .map(|problem| problem_view(&edit, problem, then).message)
      .collect();
    return Err(ToolError::refused("not_publishable", reasons));
  }
  let copy = edit
    .validate()
    .map_err(|error| ToolError::refused("not_publishable", vec![rule_message(&edit, &error)]))?;
  Ok((version.body().clone(), copy))
}

/// The step of `edit` named by its key, its name (ignoring case) or its number from 1.
fn step_named(edit: &PipelineEdit, named: &str) -> Result<StepKey, ToolError> {
  let steps = edit.steps();
  let lowered = named.to_lowercase();
  let by_number = named
    .parse::<usize>()
    .ok()
    .and_then(|number| number.checked_sub(1))
    .and_then(|index| steps.get(index));
  steps
    .iter()
    .find(|step| step.key.as_str() == named || step.name.as_str().to_lowercase() == lowered)
    .or(by_number)
    .map(|step| step.key.clone())
    .ok_or_else(|| ToolError::not_found(Missing::Step, named))
}

pub(super) fn create_error(error: &CreateError) -> ToolError {
  let code = match error {
    CreateError::Store(_) => "project_data",
    CreateError::NoSlug => "invalid_name",
    CreateError::SlugTaken { .. } => "name_taken",
  };
  ToolError::refused(code, vec![error.message()])
}
