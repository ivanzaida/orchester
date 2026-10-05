//! `list_projects` and `list_pipelines`: the projects of the workspace, and the published pipeline
//! versions a project's tasks can run with, as All projects and the task form list them.

use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Translate;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  tasks::data::published_pipelines,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListProjects {}

/// Arguments of a tool about one project.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InProject {
  /// The project's id, or its name.
  pub project: String,
}

pub async fn list_projects(services: &Services, _: ListProjects) -> Result<Value, ToolError> {
  let workspace = services.workspace.clone();
  let entries = blocking(move || Ok(workspace.entries()?)).await?;
  let projects: Vec<Value> = entries
    .iter()
    .map(|entry| {
      json!({
        "id": entry.project.id.to_string(),
        "name": entry.project.name.as_str(),
        "folder": entry.project.repo_root.display().to_string(),
        "folder_missing": entry.folder_missing,
      })
    })
    .collect();
  Ok(json!({ "projects": projects }))
}

pub async fn list_pipelines(services: &Services, args: InProject, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let choices = blocking(move || {
    let project = services.project(&args.project)?;
    let db = orchester_store::ProjectDb::open(&project.repo_root)?;
    Ok(published_pipelines(&db)?)
  })
  .await?;
  let pipelines: Vec<Value> = choices
    .iter()
    .map(|choice| {
      let steps: Vec<Value> = choice
        .steps
        .iter()
        .map(|step| {
          json!({
            "name": &*step.name,
            "executor": translate.text(&step.executor),
            "approval_gate": step.approval_gate,
          })
        })
        .collect();
      json!({
        "id": choice.id.to_string(),
        "name": &*choice.name,
        "version": choice.version.get(),
        "steps": steps,
      })
    })
    .collect();
  Ok(json!({ "pipelines": pipelines }))
}
