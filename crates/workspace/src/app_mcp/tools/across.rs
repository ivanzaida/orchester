//! Reads across projects (`docs/design/orchestrator-toolset.md`, "Tools"): a list tool answers for the project
//! it names or, without one, for every project the app knows. Each item names its project. A project that
//! cannot be read is listed apart, with the error a call naming it would get, and the others are answered for.
//!
//! A list tool only reads: every project's database is opened only to read it, so a read migrates and writes
//! nothing, and a project this version has not opened yet (its data waits for a migration) stays untouched, for
//! an older build of the app to keep opening it. It is listed as unreadable until a person opens it.

use orchester_store::{ProjectDb, RegisteredProject, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Translate;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, registered, there},
  },
  messages::Message,
  workspace::OpenProject,
};

/// Arguments of a list tool that reads across projects.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InProjects {
  /// The project's id, or its name; left out, every project the app knows.
  #[serde(default)]
  pub project: Option<String>,
}

/// What a read found in each project it read, in the order the app lists them (most recently opened first),
/// and the projects it could not read.
pub(super) struct Across<T> {
  read: Vec<(OpenProject, T)>,
  unreadable: Vec<(RegisteredProject, ToolError)>,
}

/// `read` of project `named`, whose error is the call's; or, with none named, of every project the app knows,
/// where a project's error lists it apart. Each database is opened only to read it. Blocks on SQLite.
pub(super) fn across<T>(
  services: &Services,
  named: Option<&str>,
  mut read: impl FnMut(&OpenProject, &ProjectDb) -> Result<T, ToolError>,
) -> Result<Across<T>, ToolError> {
  let projects = services.workspace.projects()?;
  let mut across = Across {
    read: Vec::new(),
    unreadable: Vec::new(),
  };
  if let Some(named) = named {
    let found = read_one(registered(&projects, named)?, &mut read)?;
    across.read.push(found);
    return Ok(across);
  }
  for project in projects {
    match read_one(&project, &mut read) {
      Ok(found) => across.read.push(found),
      Err(error) => across.unreadable.push((project, error)),
    }
  }
  Ok(across)
}

/// `read` of `project`, its database opened only to read it.
fn read_one<T>(
  project: &RegisteredProject,
  read: &mut impl FnMut(&OpenProject, &ProjectDb) -> Result<T, ToolError>,
) -> Result<(OpenProject, T), ToolError> {
  there(project)?;
  let db = ProjectDb::open_read_only(&project.repo_root).map_err(|error| match error {
    StoreError::SchemaTooOld { .. } => ToolError::refused(
      "project_not_opened_yet",
      vec![Message::new("appMcp.errors.projectNotOpenedYet").with("project", project.name.as_str().to_owned())],
    ),
    other => other.into(),
  })?;
  let open = OpenProject {
    project: db.project().clone(),
    repo_root: project.repo_root.clone(),
  };
  let found = read(&open, &db)?;
  Ok((open, found))
}

impl<T> Across<T> {
  /// The items `items` makes of each project's reading, each naming its project.
  pub(super) fn items(&self, mut items: impl FnMut(&OpenProject, &T) -> Vec<Value>) -> Vec<Value> {
    self
      .read
      .iter()
      .flat_map(|(project, found)| {
        items(project, found).into_iter().map(|mut item| {
          item["project"] = project_json(project);
          item
        })
      })
      .collect()
  }

  /// The projects that could not be read, each with its error in `translate`d words.
  pub(super) fn unreadable(&self, translate: &Translate) -> Vec<Value> {
    self
      .unreadable
      .iter()
      .map(|(project, error)| {
        json!({
          "id": project.id.to_string(),
          "name": project.name.as_str(),
          "error": error.to_json(|message| translate.text(message))["error"],
        })
      })
      .collect()
  }

  /// The result: the items under `key`, and the projects that could not be read.
  pub(super) fn json(
    &self,
    key: &str,
    translate: &Translate,
    items: impl FnMut(&OpenProject, &T) -> Vec<Value>,
  ) -> Value {
    json!({ key: self.items(items), "unreadable_projects": self.unreadable(translate) })
  }
}

/// Refused: `argument` names something of one project, so the call names the project too.
pub(super) fn needs_project(argument: &str) -> ToolError {
  ToolError::InvalidArguments(format!(
    "`{argument}` names something of one project: give `project` too"
  ))
}

/// `items` gathered from several projects, oldest first by the time their `field` holds; an item without one
/// keeps its place after the others.
pub(super) fn oldest_first(items: &mut [Value], field: &str) {
  items.sort_by_key(|item| {
    let at = item[field].as_i64();
    (at.is_none(), at)
  });
}

/// A project as an item names it.
fn project_json(project: &OpenProject) -> Value {
  json!({ "id": project.project.id().to_string(), "name": project.project.name().as_str() })
}
