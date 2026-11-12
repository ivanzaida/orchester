//! What the panel of a team MCP server shows besides how it stands (`Agents · MCP servers` `yIsDi`, C4–C8 in `acFkz`):
//! what the team holds of it (how it runs, its arguments and folder, the tools that run without asking, the variables
//! and secret headers by name), who uses it, and what this computer fills in (the path of its program, the person's
//! choice to switch it on) (`docs/design/team-configuration.md`, "What a computer fills in"). A secret's value is never
//! here: whether it is set is the server's readiness.

use std::path::PathBuf;

use orchester_core::{
  id::ResourceId,
  resource::{TeamMcpLaunch, TeamMcpServer},
  secret::SecretName,
  time::Timestamp,
};
use orchester_store::{McpServerProgram, ProjectDb, StoreError};

use super::{
  team_mcp_edit::{self, McpEdit},
  team_profile::TeamProfileSheet,
  team_sheets,
};
use crate::team_config::{TeamConfig, TeamResource};

/// Who published the version this computer uses, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
  pub by: String,
  pub at: Timestamp,
}

/// A header a remote server is sent a secret in: `name: <prefix><value of the secret>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretHeader {
  pub name: String,
  pub secret: SecretName,
  pub prefix: Option<String>,
}

/// How the team's server runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpRuns {
  /// A program each computer starts: its name (never a path), arguments, the folder it starts in, relative to the
  /// run's worktree, and the variables it needs, each filled from the team secret of its own name.
  Program {
    program: String,
    args: Vec<String>,
    folder: Option<String>,
    variables: Vec<SecretName>,
  },
  Remote {
    url: String,
    headers: Vec<SecretHeader>,
  },
}

/// The path of a program this computer set, for every project of the team server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramPath {
  pub path: PathBuf,
  pub set_at: Timestamp,
}

/// One team MCP server as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamMcpSheet {
  pub runs: McpRuns,
  /// The tools that run without asking.
  pub exempt_tools: Vec<String>,
  /// The names of the team's profiles that choose it.
  pub used_by: Vec<String>,
  /// The secrets it is given the values of, in order, each once.
  pub secrets: Vec<SecretName>,
  /// The path set on this computer; none for a remote server and while none is set.
  pub program: Option<ProgramPath>,
  /// The person's choice to use it here: on, off, or `None` while nobody has decided.
  pub switch: Option<bool>,
  /// Who published its latest version.
  pub published: Option<Published>,
  /// What an admin's edit starts from; `None` for a server the edit form cannot hold.
  pub edit: Option<McpEdit>,
}

/// A sheet for each team MCP server of `servers` whose latest version is one this computer reads. `profiles` are the
/// team's profile sheets by resource, which say who uses it; `programs` the paths this computer set for the team
/// server.
pub(super) fn sheets(
  (db, config): (&ProjectDb, &TeamConfig),
  servers: &[&TeamResource],
  programs: &[McpServerProgram],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<(ResourceId, TeamMcpSheet)>, StoreError> {
  let mut sheets = Vec::new();
  for resource in servers.iter().copied() {
    let (Some(number), Some(Ok(body))) = (resource.number(), resource.body::<TeamMcpServer>()) else {
      continue;
    };
    let body = &body;
    let used_by = team_sheets::used_by(config, profiles, |sheet| {
      sheet.mcp_servers.iter().any(|used| used.server.id == resource.id)
    });
    let resource = resource.id;
    sheets.push((
      resource,
      TeamMcpSheet {
        runs: runs(body.launch()),
        exempt_tools: body
          .exempt_tools()
          .tools()
          .iter()
          .map(|tool| tool.as_str().to_owned())
          .collect(),
        used_by,
        secrets: body.secrets(),
        program: programs
          .iter()
          .find(|set| set.resource == resource)
          .map(|set| ProgramPath {
            path: set.program.clone(),
            set_at: set.set_at,
          }),
        switch: db.team_mcp_switch(resource)?,
        published: config.resource(resource).and_then(team_sheets::published),
        edit: team_mcp_edit::of_latest((number, body)),
      },
    ));
  }
  Ok(sheets)
}

pub(super) fn runs(launch: &TeamMcpLaunch) -> McpRuns {
  match launch {
    TeamMcpLaunch::Program(program) => McpRuns::Program {
      program: program.program().as_str().to_owned(),
      args: program.args().iter().map(|arg| arg.as_str().to_owned()).collect(),
      folder: program.working_directory().map(|folder| folder.as_str().to_owned()),
      variables: program.secrets(),
    },
    TeamMcpLaunch::Remote(remote) => McpRuns::Remote {
      url: remote.url().as_str().to_owned(),
      headers: remote
        .secret_headers()
        .iter()
        .map(|header| SecretHeader {
          name: header.header().as_str().to_owned(),
          secret: header.secret().clone(),
          prefix: header.prefix().map(|prefix| prefix.as_str().to_owned()),
        })
        .collect(),
    },
  }
}
