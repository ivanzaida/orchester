//! What the panel of a team tool shows besides how it stands (`Agents · Tools` `u9Hcc` A6, `yTVeE` M6): what the team
//! holds of it (what it runs or calls, its arguments, who may call it without asking, the secret it names), who uses
//! it, and who published the version this computer uses (`docs/design/team-configuration.md`, "Per resource kind"). A
//! secret's value is never here, and a team tool names its secret by id: it has no team form for a name yet
//! (`Undecided::TeamToolSecretById`).

use orchester_core::{
  id::{ResourceId, SecretId},
  resource::{ApprovalMode, HttpMethod, RequestTarget, ResourceKind, Tool, ToolAction},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{team_mcp::Published, team_profile::TeamProfileSheet, team_sheets};

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
  /// A request to a fixed address; the agent gives only `arguments`. `bearer`: it is sent with a bearer secret.
  Http {
    method: HttpMethod,
    url: String,
    bearer: bool,
    arguments: Vec<ToolArgument>,
  },
  /// A fixed query run with the credentials of a secret.
  Database { query: String },
}

/// The secret a tool names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolSecret {
  None,
  /// By the id it has in the project of the publisher: a team names its secrets, so a teammate's computer cannot say
  /// what it stands for.
  ById(SecretId),
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
  /// `None` while the server has not confirmed the version this computer uses.
  pub published: Option<Published>,
}

/// A sheet for each team tool that has a version cached here.
pub(super) fn sheets(
  db: &ProjectDb,
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<(ResourceId, TeamToolSheet)>, StoreError> {
  let mut sheets = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    let Some(version) = db.latest_version::<Tool>(resource.id())? else {
      continue;
    };
    let tool = version.body();
    sheets.push((
      resource.id(),
      TeamToolSheet {
        description: tool.description().to_owned(),
        runs: runs(tool),
        secret: tool.secret().map_or(ToolSecret::None, ToolSecret::ById),
        approval: tool.access().map(|access| access.approval()),
        used_by: team_sheets::used_by(db, profiles, |sheet| {
          sheet.tools.iter().any(|used| used.id == resource.id())
        })?,
        published: team_sheets::published::<Tool>(db, resource.id(), version.number())?,
      },
    ));
  }
  Ok(sheets)
}

fn runs(tool: &Tool) -> ToolRuns {
  match tool.action() {
    ToolAction::Command(command) => ToolRuns::Command {
      line: command.command_line(),
      timeout_secs: command.timeout().into(),
    },
    ToolAction::Http(request) => ToolRuns::Http {
      method: request.method(),
      url: request.url().to_owned(),
      bearer: tool.secret().is_some(),
      arguments: request
        .inputs()
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
    },
    ToolAction::DatabaseQuery(query) => ToolRuns::Database {
      query: query.query().to_owned(),
    },
  }
}
