//! `add_project`, `get_project`, `set_project_environment` and `set_repository_agent_files`: a project
//! added as project setup adds it, read as Project settings shows it, and the settings of its
//! Environment and General tabs changed as they change them.

use std::{collections::BTreeMap, path::PathBuf};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  integration::{base::BaseBranchSetting, branch_head},
  resource::{CommandEnvironment, ResourceKind, VariableName},
  store::{ConnectionStore, RepositoryStore, ResourceStore, SettingsStore},
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Translate, limits::limits_json, projects::InProject},
  widening,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  messages::{self, Message},
  project_setup::{self, Creation, ExpectedAccount, GitHubLink, ProjectDraft, creatable, github_connection_name},
  repository::{
    self, DetectTarget,
    init::{LocalRepository, read_local},
  },
  tools::form::variable_error,
  workspace::OpenProject,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AddProject {
  /// The project's name; its key (KON) is suggested from it.
  pub name: String,
  /// The absolute path of the folder at the top of the git repository the project works in.
  pub folder: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetProjectEnvironment {
  /// The project's id, or its name.
  pub project: String,
  /// Every variable the project adds, replacing those it has.
  pub variables: Vec<Variable>,
}

#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Variable {
  pub name: String,
  pub value: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetAgentFiles {
  /// The project's id, or its name.
  pub project: String,
  /// Whether agents load the repository's own agent files.
  pub enabled: bool,
}

pub async fn add_project(services: &Services, args: AddProject, translate: &Translate) -> Result<Value, ToolError> {
  let programs = services.programs.clone();
  let (new, detection) = blocking(move || {
    let folder = PathBuf::from(&args.folder);
    if !folder.is_absolute() {
      return Err(refused(
        "folder_not_absolute",
        Message::new("appMcp.errors.folderNotAbsolute"),
      ));
    }
    let draft = ProjectDraft {
      name: &args.name,
      repository: Some(&folder),
    }
    .validate();
    if let Err(errors) = &draft {
      return Err(ToolError::refused(
        "invalid_name",
        errors.name.iter().cloned().collect(),
      ));
    }
    let detection = repository::detect(&DetectTarget {
      repo_root: folder.clone(),
      connections: Vec::new(),
      program: None,
    });
    let new = creatable(draft, Some(&detection.state(None)))
      .ok_or_else(|| store_refusal(&StoreError::NotAGitRepository(folder.clone())))?;
    // As setup holds Create back: runs need git, and a commit to branch from.
    match read_local(programs.runner(), &folder) {
      LocalRepository::GitMissing => Err(refused("git_not_found", Message::new("appMcp.errors.gitNotFound"))),
      LocalRepository::NoCommits { .. } => Err(refused("no_commits", Message::new("appMcp.errors.noCommits"))),
      LocalRepository::NotRepository | LocalRepository::Commits { .. } | LocalRepository::Unread => {
        Ok((new, detection))
      }
    }
  })
  .await?;
  // As setup does: the account gh acts as now is the one the project expects.
  let link = detection.github().map(|found| GitHubLink {
    connection_name: translate.text(&github_connection_name(found.repo().host())),
    expected: ExpectedAccount::acting(&new.repo_root, &detection).map(|expected| expected.login().clone()),
    detection: detection.clone(),
  });
  let created = project_setup::create(&services.workspace, &services.activity, Creation { new, link })
    .await
    .map_err(|failure| ToolError::refused(failure.cause.code(), vec![failure.message]))?;
  let github = match created.link {
    None => json!({ "linked": null, "problem": null }),
    Some(Ok(link)) => json!({ "linked": link.map(|link| link.repo().to_string()), "problem": null }),
    Some(Err(problem)) => {
      let error = ToolError::refused("not_linked", vec![problem]);
      json!({ "linked": null, "problem": error.to_json(|message| translate.text(message))["error"] })
    }
  };
  Ok(json!({ "project": summary(&created.project), "github": github }))
}

fn summary(open: &OpenProject) -> Value {
  json!({
    "id": open.project.id().to_string(),
    "key": open.project.key().as_str(),
    "name": open.project.name().as_str(),
    "folder": open.repo_root.display().to_string(),
    "link": Link::Project(open.project.id()).path(),
  })
}

pub async fn get_project(services: &Services, args: InProject) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let mut project = summary(&open);
    let resources = |kind| db.resources(kind).map(|listed| listed.len());
    let settings = json!({
      "limits": limits_json(db.project_limits()?),
      "code_index": db.code_index()?.is_on(),
      "environment": environment_json(&db.project_environment()?),
      "repository_agent_files": db.repository_agent_files()?.is_on(),
      "base_branch": base_branch_json(&services, &open, &db.base_branch_setting()?),
      "github": db.repository_link()?.map(|link| link.repo().to_string()),
      "connections": db.connections()?.len(),
      "profiles": resources(ResourceKind::AgentProfile)?,
      "pipelines": resources(ResourceKind::Pipeline)?,
      "instructions": resources(ResourceKind::Instruction)?,
    });
    if let (Value::Object(project), Value::Object(settings)) = (&mut project, settings) {
      project.extend(settings);
    }
    Ok(json!({ "project": project }))
  })
  .await
}

/// Project settings › General › Base branch: `branch` is the branch set, `null` while new runs take the branch the
/// checkout has checked out; `missing` says the repository no longer has the branch set, so new runs are refused
/// (`null` when git cannot tell).
fn base_branch_json(services: &Services, open: &OpenProject, setting: &BaseBranchSetting) -> Value {
  let Some(branch) = setting.branch() else {
    return json!({ "branch": null, "missing": false });
  };
  let head = branch_head(
    &services.programs.runner(),
    std::path::Path::new(crate::tasks::changes::GIT),
    (&open.repo_root, branch),
  );
  let missing = match head {
    Ok(head) => Some(head.is_none()),
    Err(error) => {
      tracing::warn!(%error, "cannot tell whether the project's base branch exists");
      None
    }
  };
  json!({ "branch": branch.as_str(), "missing": missing })
}

pub async fn set_environment(services: &Services, args: SetProjectEnvironment) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let environment = environment(args.variables)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let current = db.project_environment()?;
    widening::allowed(&services, changed_variables(&current, &environment))?;
    db.save_project_environment(&environment)?;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::ProjectSettings);
    Ok(json!({ "environment": environment_json(&db.project_environment()?) }))
  })
  .await
}

pub(super) use crate::environment::widenings as changed_variables;

/// The variables as Project settings › Environment checks them.
pub(super) fn environment(variables: Vec<Variable>) -> Result<CommandEnvironment, ToolError> {
  let invalid = |error| ToolError::refused("invalid_variable", vec![variable_error(&error)]);
  let mut named = BTreeMap::new();
  for variable in variables {
    let name = VariableName::new(variable.name.trim()).map_err(invalid)?;
    if named.insert(name, variable.value).is_some() {
      let message = Message::new("tools.errors.duplicateVariable").with("name", variable.name);
      return Err(ToolError::refused("invalid_variable", vec![message]));
    }
  }
  CommandEnvironment::new(named).map_err(invalid)
}

pub(super) fn environment_json(environment: &CommandEnvironment) -> Value {
  let variables: Vec<Value> = environment
    .iter()
    .map(|(name, value)| json!({ "name": name.to_string(), "value": value }))
    .collect();
  json!(variables)
}

pub async fn set_agent_files(services: &Services, args: SetAgentFiles) -> Result<Value, ToolError> {
  let services = services.clone();
  let setting = if args.enabled {
    RepositoryAgentFiles::On
  } else {
    RepositoryAgentFiles::Off
  };
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    // Turning them on puts what the repository says in every agent's prompt; turning them off is always taken.
    if crate::prompt_changes::agent_files_change_prompts(db.repository_agent_files()?.is_on(), args.enabled) {
      widening::prompt_allowed(&services, vec![crate::prompt_changes::repository_agent_files()])?;
    }
    db.save_repository_agent_files(setting)?;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::ProjectSettings);
    Ok(json!({ "repository_agent_files": db.repository_agent_files()?.is_on() }))
  })
  .await
}

/// A project that cannot be set up in the folder, as setup words it under the folder field.
fn store_refusal(error: &StoreError) -> ToolError {
  let code = match error {
    StoreError::NotAGitRepository(_) => "not_a_git_repository",
    StoreError::AlreadyInitialized(_) => "already_initialized",
    _ => "project_data",
  };
  ToolError::refused(code, vec![messages::store_error(error)])
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
