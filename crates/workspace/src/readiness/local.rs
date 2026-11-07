//! The profiles and tools of a local project: what each asks of this computer, from the version it uses.

use std::{
  ffi::{OsStr, OsString},
  path::Path,
};

use orchester_core::{
  connection::{
    ConnectionKind,
    invocation::Platform,
    mcp::{McpServer, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId, SecretId},
  resource::{AgentProfile, ResourceKind, Tool},
  secret::SecretVault,
  sharing::Needs,
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore, SettingsStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  facts::Found,
  needs::{Entry, Profile},
  standing::Undecided,
};

/// The profiles of a local project. A profile runs on the connection it names, which is its pick, and each MCP server
/// it chooses that is turned on and starts a program must have a program a run starts, looked up as the runner does:
/// a file on the `PATH` the agent starts with (the project's Environment, else Orchester's search path) or in the
/// project, and, for a fixed file, still outside the repository (`repo_root`, which also stands for the worktree a run
/// makes later). A server turned off in Connections is left out, as a run leaves it out. The servers stay unlisted as
/// resources, and a server the project no longer has is left to the run, which says it is missing.
pub(super) fn profiles(db: &ProjectDb, repo_root: &Path) -> Result<Vec<Profile>, StoreError> {
  let servers = db.mcp_servers()?;
  let path = db
    .project_environment()?
    .get("PATH")
    .map(OsString::from)
    .or_else(orchester_exec::search_path::search_path);
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let own = version.body().connection();
    let provider = db.connection(own)?.and_then(|connection| match connection.kind() {
      ConnectionKind::Agent(provider) => Some(*provider),
      ConnectionKind::GitHub { .. } => None,
    });
    let mut entry = Entry::new(&resource, version.number(), Err(Undecided::ConnectionGone(own)));
    let mut pick = None;
    if let Some(provider) = provider {
      let chosen = started_servers(version.body(), &servers);
      let named: Vec<_> = chosen
        .iter()
        .map(|(server, _)| (server.id(), server.name().clone()))
        .collect();
      entry.found = chosen
        .iter()
        .filter_map(|(server, stdio)| program_found(server.id(), stdio, (repo_root, path.as_deref())))
        .collect();
      entry.needs = Ok(Needs::local_profile(resource.id(), provider, &named));
      pick = Some(own);
    }
    profiles.push(Profile { entry, pick });
  }
  Ok(profiles)
}

/// The servers `profile` chooses that a run starts as a program: turned on, and with a program to start.
fn started_servers<'a>(profile: &AgentProfile, servers: &'a [McpServer]) -> Vec<(&'a McpServer, &'a StdioServer)> {
  profile
    .mcp_servers()
    .ids()
    .iter()
    .filter_map(|id| {
      let server = servers.iter().find(|server| server.id() == *id)?;
      match server.transport() {
        McpTransport::Stdio(stdio) if server.enabled() => Some((server, stdio)),
        McpTransport::Stdio(_) | McpTransport::Http(_) => None,
      }
    })
    .collect()
}

/// What a run would find for `stdio`: the first candidate that is a file, which a fixed file must also have its place
/// outside the repository. Nothing when there is no such file (the need then reports it as not found).
fn program_found(
  server: ConnectionId,
  stdio: &StdioServer,
  (repo_root, path): (&Path, Option<&OsStr>),
) -> Option<Found> {
  let found = stdio
    .program()
    .candidates(repo_root, path, Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_file())?;
  Some(match stdio.program().check_place(&found, repo_root) {
    Ok(()) => Found::Program(server),
    Err(_) => Found::ProgramMoved(server),
  })
}

/// The tools of a local project. A tool that sends or connects with a project secret needs its value, which only the
/// keychain holds, so `vault` is asked whether it is there; the value is never read, since reading can make the
/// keychain ask the person each time readiness is refreshed. A command tool holds no secret and asks nothing.
pub(super) fn tools(db: &ProjectDb, vault: &dyn SecretVault) -> Result<Vec<Entry>, StoreError> {
  let mut tools = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    let Some(version) = db.latest_version::<Tool>(resource.id())? else {
      continue;
    };
    let mut entry = Entry::new(&resource, version.number(), Ok(Needs::tool(Vec::new())));
    if let Some(secret) = version.body().secret() {
      secret_value(db, vault, resource.id(), secret, &mut entry)?;
    }
    tools.push(entry);
  }
  Ok(tools)
}

fn secret_value(
  db: &ProjectDb,
  vault: &dyn SecretVault,
  tool: ResourceId,
  id: SecretId,
  entry: &mut Entry,
) -> Result<(), StoreError> {
  let Some(secret) = db.secret(id)? else {
    entry.needs = Err(Undecided::SecretGone(id));
    return Ok(());
  };
  entry.needs = Ok(Needs::local_tool(id, secret.name().clone()));
  if secret.is_archived() {
    return Ok(());
  }
  match vault.exists(&secret.vault_key(db.project().id())) {
    Ok(true) => entry.found.push(Found::SecretValue(id)),
    Ok(false) => {}
    Err(error) => {
      tracing::warn!(%tool, secret = %id, %error, "the keychain did not say whether a tool's secret has a value");
      entry.needs = Err(Undecided::SecretUnreadable(id));
    }
  }
  Ok(())
}
