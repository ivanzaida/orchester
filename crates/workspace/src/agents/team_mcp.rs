//! What the panel of a team MCP server shows besides how it stands (`Agents · MCP servers` `yIsDi`, C4–C8 in `acFkz`):
//! what the team holds of it (how it runs, its arguments and folder, the tools that run without asking, the variables
//! and secret headers by name), who uses it, and what this computer fills in (the path of its program, the person's
//! choice to switch it on) (`docs/design/team-configuration.md`, "What a computer fills in"). A secret's value is never
//! here: whether it is set is the server's readiness.

use std::path::PathBuf;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer},
  secret::SecretName,
  store::ResourceStore,
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

/// The team's MCP servers a sheet is made for: those it has not archived, and each archived one that keeps a draft
/// here.
pub(super) fn listed<'a>(db: &ProjectDb, config: &'a TeamConfig) -> Result<Vec<&'a TeamResource>, StoreError> {
  let mut resources = config.resources(ResourceKind::McpServer);
  // An archived server whose draft is kept here is shown too: the draft stays to copy, discard or publish once the
  // server is restored.
  for (kept, _) in db.archived_with_draft(ResourceKind::McpServer)? {
    resources.extend(config.archived(kept.id()));
  }
  Ok(resources)
}

/// A sheet for each team MCP server whose latest version is one this computer reads ([`listed`]). `profiles` are the
/// team's profile sheets by resource, which say who uses it; `programs` the paths this computer set for the team
/// server.
pub(super) fn sheets(
  (db, config): (&ProjectDb, &TeamConfig),
  programs: &[McpServerProgram],
  profiles: &[(ResourceId, TeamProfileSheet)],
) -> Result<Vec<(ResourceId, TeamMcpSheet)>, StoreError> {
  let mut sheets = Vec::new();
  for resource in listed(db, config)? {
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
        edit: edit(db, resource, (number, body))?,
      },
    ));
  }
  Ok(sheets)
}

/// What an admin's edit of `resource` starts from: with the draft kept here while the project's database still holds
/// the resource and its versions, else from its `latest` version alone.
fn edit(
  db: &ProjectDb,
  resource: ResourceId,
  latest: (orchester_core::resource::VersionNumber, &TeamMcpServer),
) -> Result<Option<McpEdit>, StoreError> {
  if db.resource(resource)?.is_some() && db.latest_version::<TeamMcpServer>(resource)?.is_some() {
    return team_mcp_edit::of(db, resource);
  }
  Ok(team_mcp_edit::of_latest(latest))
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
