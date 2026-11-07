//! The profiles and tools of a local project: what each asks of this computer, from the version it uses.

use std::{ffi::OsStr, path::Path};

use orchester_core::{
  connection::{
    ConnectionKind,
    invocation::Platform,
    mcp::{McpServer, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId, SecretId},
  resource::{AgentProfile, ResourceKind, Tool},
  secret::{SecretVault, VaultError},
  sharing::Needs,
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  facts::Found,
  needs::{Entry, Profile},
  standing::Undecided,
};

/// The profiles of a local project. A profile runs on the connection it names, which is its pick, and the program of
/// each MCP server it chooses that the agent's CLI starts must be a file: on the `PATH` the agent starts with (the
/// project's, else Orchester's search path) or in the project, as the runner looks it up. `repo_root` stands for the
/// worktree, which a run makes later. The servers stay unlisted as resources: a local server is on whenever its
/// profile is, and a server the project no longer has is left to the run, which says it is missing.
pub(super) fn profiles(db: &ProjectDb, repo_root: &Path) -> Result<Vec<Profile>, StoreError> {
  let servers = db.mcp_servers()?;
  let path = orchester_exec::search_path::search_path();
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
      let chosen = stdio_servers(version.body(), &servers);
      let ids: Vec<ConnectionId> = chosen.iter().map(|(id, _)| *id).collect();
      entry.found = chosen
        .iter()
        .filter(|(_, server)| program_exists(server, repo_root, path.as_deref()))
        .map(|(id, _)| Found::Program(*id))
        .collect();
      entry.needs = Ok(Needs::local_profile(resource.id(), provider, &ids));
      pick = Some(own);
    }
    profiles.push(Profile { entry, pick });
  }
  Ok(profiles)
}

/// The servers `profile` chooses that start a program, by id.
fn stdio_servers<'a>(profile: &AgentProfile, servers: &'a [McpServer]) -> Vec<(ConnectionId, &'a StdioServer)> {
  profile
    .mcp_servers()
    .ids()
    .iter()
    .filter_map(|id| {
      let server = servers.iter().find(|server| server.id() == *id)?;
      match server.transport() {
        McpTransport::Stdio(stdio) => Some((*id, stdio)),
        McpTransport::Http(_) => None,
      }
    })
    .collect()
}

fn program_exists(server: &StdioServer, repo_root: &Path, path: Option<&OsStr>) -> bool {
  server
    .program()
    .candidates(repo_root, path, Platform::current())
    .iter()
    .any(|candidate| candidate.is_file())
}

/// The tools of a local project. A tool that sends or connects with a project secret needs its value, which only the
/// keychain holds, so `vault` is asked for it; the value itself is dropped at once. A command tool holds no secret and
/// asks nothing.
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
  match vault.read(&secret.vault_key(db.project().id())) {
    Ok(_) => entry.found.push(Found::SecretValue(id)),
    Err(VaultError::NotFound) => {}
    Err(error) => {
      tracing::warn!(%tool, secret = %id, %error, "the keychain did not say whether a tool's secret has a value");
      entry.needs = Err(Undecided::SecretUnreadable(id));
    }
  }
  Ok(())
}
