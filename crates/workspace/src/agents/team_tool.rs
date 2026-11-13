//! What the panel of a team tool shows besides how it stands (`Agents · Tools` `u9Hcc` A6, `yTVeE` M6): what the team
//! holds of it (what it runs or calls, its arguments, who may call it without asking, the secret it names), who uses
//! it, and who published the version this computer uses (`docs/design/team-configuration.md`, "Per resource kind"). A
//! secret's value is never here: a team tool names its secret by name, and each computer fills in the value. A latest
//! version published before that form names it by the id it had on the publisher's computer, which is never shown
//! ([`ToolSecret::ById`], `Undecided::TeamToolSecretById`).

use std::collections::BTreeMap;

use orchester_core::{
  id::ResourceId,
  resource::{
    ApprovalMode, FieldName, HttpMethod, Pipeline, RequestTarget, ResourceKind, StepKey, TeamTool, TeamToolAction,
    Tool, ToolAction, ToolKind,
  },
  secret::SecretName,
};

use super::{
  team_body_edit::Latest,
  team_mcp::Published,
  team_profile::TeamProfileSheet,
  team_sheets::{self, latest_of},
};
use crate::team_config::{TeamConfig, TeamResource};

/// Where an HTTP tool puts the value of an argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentPlace {
  Query,
  Header,
  Body,
}

/// An argument an agent gives an HTTP tool, and where the request carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolArgument {
  pub name: String,
  pub place: ArgumentPlace,
}

/// What a tool runs or calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolRuns {
  /// A program with fixed arguments, as a person reads the command line, and how long it may run.
  Command { line: String, timeout_secs: u64 },
  /// A request to a fixed address; the agent gives only `arguments`. A tool that names a secret sends it as the
  /// bearer token ([`TeamToolSheet::secret`]).
  Http {
    method: HttpMethod,
    url: String,
    arguments: Vec<ToolArgument>,
  },
  /// A fixed query run with the credentials of a secret.
  Database { query: String },
}

/// The secret a tool names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolSecret {
  None,
  /// The team secret of this name, whose value each computer fills in.
  Named(SecretName),
  /// By the id it had in the project of the publisher, in a version published before the team's form: a teammate's
  /// computer cannot say what it stands for, and the id is not shown.
  ById,
}

/// One team tool as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamToolSheet {
  /// What the tool does, as its author wrote it; may be empty.
  pub description: String,
  pub runs: ToolRuns,
  pub secret: ToolSecret,
  /// When a person has to approve a call; `None` for a command, which has no access policy.
  pub approval: Option<ApprovalMode>,
  /// The names of the team's profiles that use it.
  pub used_by: Vec<String>,
  /// Who published its latest version.
  pub published: Option<Published>,
  /// What an admin's edit starts from: its latest version in the team's form, which names its secret by name. `None`
  /// for a version that names its secret by id ([`Self::edit`]).
  pub latest: Option<Latest<TeamTool>>,
  /// A latest version that names its secret by id, as this computer's tool, which an admin whose project has that
  /// secret publishes again by name; `None` for a version in the team's form.
  pub edit: Option<Latest<Tool>>,
}

/// A sheet for each team tool of `tools` whose latest version is a tool this computer reads: in the team's form, or in
/// the form before it.
pub(super) fn sheets(
  config: &TeamConfig,
  tools: &[&TeamResource],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Vec<(ResourceId, TeamToolSheet)> {
  let mut sheets = Vec::new();
  for resource in tools.iter().copied() {
    let Some(read) = read(resource) else {
      continue;
    };
    let mut sheet = match &read {
      Read::Team(tool) => team_sheet(resource, tool),
      Read::ById(tool) => by_id_sheet(resource, tool),
    };
    sheet.used_by = team_sheets::used_by(config, profiles, |sheet| {
      sheet.tools.iter().any(|used| used.id == resource.id)
    });
    sheets.push((resource.id, sheet));
  }
  sheets
}

/// A latest version as this computer reads it.
enum Read {
  Team(TeamTool),
  /// The local form a version published before the team's form holds, with its secret by id.
  ById(Tool),
}

/// The kind of the tool `resource`'s latest version, in either form.
pub(super) fn kind(resource: &TeamResource) -> Option<ToolKind> {
  Some(match read(resource)? {
    Read::Team(tool) => tool.kind(),
    Read::ById(tool) => tool.kind(),
  })
}

fn read(resource: &TeamResource) -> Option<Read> {
  if let Some(Ok(tool)) = resource.body::<TeamTool>() {
    return Some(Read::Team(tool));
  }
  match resource.body::<Tool>()? {
    Ok(tool) => Some(Read::ById(tool)),
    Err(_) => None,
  }
}

fn team_sheet(resource: &TeamResource, tool: &TeamTool) -> TeamToolSheet {
  let runs = match tool.action() {
    TeamToolAction::Command(command) => ToolRuns::Command {
      line: command.command_line(),
      timeout_secs: command.timeout().into(),
    },
    TeamToolAction::Http(request) => http_runs(request.method(), request.url(), request.inputs()),
    TeamToolAction::DatabaseQuery(query) => ToolRuns::Database {
      query: query.query().to_owned(),
    },
  };
  TeamToolSheet {
    description: tool.description().to_owned(),
    runs,
    secret: tool.secret().cloned().map_or(ToolSecret::None, ToolSecret::Named),
    approval: tool.access().map(|access| access.approval()),
    used_by: Vec::new(),
    published: team_sheets::published(resource),
    latest: latest_of(resource, tool),
    edit: None,
  }
}

fn by_id_sheet(resource: &TeamResource, tool: &Tool) -> TeamToolSheet {
  let runs = match tool.action() {
    ToolAction::Command(command) => ToolRuns::Command {
      line: command.command_line(),
      timeout_secs: command.timeout().into(),
    },
    ToolAction::Http(request) => http_runs(request.method(), request.url(), request.inputs()),
    ToolAction::DatabaseQuery(query) => ToolRuns::Database {
      query: query.query().to_owned(),
    },
  };
  TeamToolSheet {
    description: tool.description().to_owned(),
    runs,
    secret: tool.secret().map_or(ToolSecret::None, |_| ToolSecret::ById),
    approval: tool.access().map(|access| access.approval()),
    used_by: Vec::new(),
    published: team_sheets::published(resource),
    latest: None,
    edit: latest_of(resource, tool),
  }
}

fn http_runs(method: HttpMethod, url: &str, inputs: &BTreeMap<FieldName, RequestTarget>) -> ToolRuns {
  ToolRuns::Http {
    method,
    url: url.to_owned(),
    arguments: inputs
      .iter()
      .map(|(name, target)| ToolArgument {
        name: name.as_str().to_owned(),
        place: match target {
          RequestTarget::Query(_) => ArgumentPlace::Query,
          RequestTarget::Header(_) => ArgumentPlace::Header,
          RequestTarget::Body(_) => ArgumentPlace::Body,
        },
      })
      .collect(),
  }
}

/// A pipeline of the team whose steps a team tool's access may let call it (*Pipelines that call it*, A32 `gLLEv`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallingPipeline {
  pub id: ResourceId,
  pub name: String,
  /// Its latest version's steps in order, by key and name.
  pub steps: Vec<(StepKey, String)>,
}

/// The team's live pipelines whose latest version this computer reads, in the order `config` holds them.
pub fn calling_pipelines(config: &TeamConfig) -> Vec<CallingPipeline> {
  config
    .resources(ResourceKind::Pipeline)
    .into_iter()
    .filter_map(|resource| {
      let pipeline = resource.body::<Pipeline>()?.ok()?;
      Some(CallingPipeline {
        id: resource.id,
        name: resource.name.as_str().to_owned(),
        steps: pipeline
          .steps()
          .iter()
          .map(|step| (step.key.clone(), step.name.as_str().to_owned()))
          .collect(),
      })
    })
    .collect()
}
