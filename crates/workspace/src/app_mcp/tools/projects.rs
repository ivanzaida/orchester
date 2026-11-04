//! `list_projects`, `set_code_index` and `list_pipelines`: the projects of the workspace with whether
//! each uses the code index and its limits, that setting changed as Project settings › Context changes it, and the
//! published pipeline versions a project's tasks can run with, as All projects and the task form list
//! them.

use std::path::Path;

use orchester_core::{code_index::CodeIndexSetting, id::ProjectId, store::SettingsStore};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{InProjects, across},
  limits,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  code_index,
  links::Link,
  tasks::{
    data::{PipelineChoice, published_pipelines},
    outside::Outside,
  },
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

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetCodeIndex {
  /// The project's id, or its name.
  pub project: String,
  /// Whether agents that start from now on get the code index (*Use code index*).
  pub enabled: bool,
}

pub async fn list_projects(services: &Services, _: ListProjects) -> Result<Value, ToolError> {
  let workspace = services.workspace.clone();
  let listed = blocking(move || {
    let entries = workspace.entries()?;
    let settings = entries
      .iter()
      .map(|entry| {
        let root = &entry.project.repo_root;
        if entry.folder_missing {
          (None, None)
        } else {
          (code_index_of(root), limits::limits_of(root))
        }
      })
      .collect::<Vec<_>>();
    Ok((entries, settings))
  })
  .await?;
  let projects: Vec<Value> = listed
    .0
    .iter()
    .zip(listed.1)
    .map(|(entry, (index, project_limits))| {
      json!({
        "id": entry.project.id.to_string(),
        "name": entry.project.name.as_str(),
        "folder": entry.project.repo_root.display().to_string(),
        "folder_missing": entry.folder_missing,
        "code_index": index.map(CodeIndexSetting::is_on),
        "limits": project_limits.map(limits::limits_json),
        "link": Link::Project(entry.project.id).path(),
      })
    })
    .collect();
  Ok(json!({ "projects": projects }))
}

/// The code index setting of the project at `root`, read without changing its database; `None` when
/// it cannot be read before the project is opened (its data waits for a migration) or reading failed,
/// which the log says.
fn code_index_of(root: &Path) -> Option<CodeIndexSetting> {
  match ProjectDb::open_read_only(root).and_then(|db| db.code_index()) {
    Ok(setting) => Some(setting),
    Err(StoreError::SchemaTooOld { .. }) => None,
    Err(error) => {
      tracing::warn!(%error, project = %root.display(), "cannot read a project's code index setting");
      None
    }
  }
}

pub async fn set_code_index(services: &Services, args: SetCodeIndex) -> Result<Value, ToolError> {
  let services = services.clone();
  let setting = if args.enabled {
    CodeIndexSetting::On
  } else {
    CodeIndexSetting::Off
  };
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    code_index::save_setting_in(&root, setting)?;
    services.changes.record(open.project.id(), Outside::CodeIndex);
    Ok(json!({ "code_index": code_index::setting_in(&root)?.is_on() }))
  })
  .await
}

pub async fn list_pipelines(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || across(&services, args.project.as_deref(), |_, db| Ok(published_pipelines(db)?))).await?;
  Ok(read.json("pipelines", translate, |open, choices| {
    choices
      .iter()
      .map(|choice| pipeline_json(choice, open.project.id(), translate))
      .collect()
  }))
}

/// A published pipeline of `project` as the tools give it: id, name, version, steps and its link.
pub(super) fn pipeline_json(choice: &PipelineChoice, project: ProjectId, translate: &Translate) -> Value {
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
    "link": Link::Pipeline(project, choice.id).path(),
  })
}
