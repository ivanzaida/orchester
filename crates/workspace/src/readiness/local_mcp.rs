//! The local MCP servers of a project, each on its own: what a server asks of this computer, which is its program when
//! a run starts one and the value of each project secret its variables and headers take
//! (`docs/design/team-configuration.md` rule 12). A profile that chooses a server counts the server's program among its
//! own needs ([`super::local`]); the hub of a local project lists each server with its own standing, so it can say what
//! to fix on the server itself.

use std::{ffi::OsString, path::Path};

use orchester_core::{
  connection::mcp::{McpServer, McpTransport},
  id::ConnectionId,
  secret::SecretVault,
  sharing::{Here, Needs, readiness},
  store::{McpServerStore, SecretStore, SettingsStore},
};
use orchester_store::ProjectDb;

use super::{
  ReadinessError, local,
  standing::{ReadinessStanding, Undecided},
};

/// A local MCP server that is not archived, and how it stands on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMcpReadiness {
  server: McpServer,
  standing: ReadinessStanding,
}

impl LocalMcpReadiness {
  pub const fn server(&self) -> &McpServer {
    &self.server
  }

  pub const fn id(&self) -> ConnectionId {
    self.server.id()
  }

  /// Ready, what is missing (the program, a secret's value), or why that cannot be said. Whether the server is on is
  /// not part of it ([`Self::group`]).
  pub const fn standing(&self) -> &ReadinessStanding {
    &self.standing
  }

  /// Where a list puts it: a server turned off is off here whatever else it lacks; else it needs the person or is
  /// ready.
  pub fn group(&self) -> super::Group {
    if !self.server.enabled() {
      super::Group::OffHere
    } else if self.standing.is_ready() {
      super::Group::Ready
    } else {
      super::Group::NeedsYouHere
    }
  }
}

/// Every local MCP server of the project that is not archived, by name. The keychain is asked whether each secret has a
/// value and never for the value (`SecretVault::exists`), so reading it again does not make the keychain prompt.
pub fn local_mcp_servers(
  db: &ProjectDb,
  repo_root: &Path,
  vault: &dyn SecretVault,
) -> Result<Vec<LocalMcpReadiness>, ReadinessError> {
  let archived: Vec<ConnectionId> = db.archived_mcp_servers()?.into_iter().map(|(id, _)| id).collect();
  let path = db
    .project_environment()?
    .get("PATH")
    .map(OsString::from)
    .or_else(orchester_exec::search_path::search_path);
  let mut found = Vec::new();
  for server in db
    .mcp_servers()?
    .into_iter()
    .filter(|server| !archived.contains(&server.id()))
  {
    let standing = standing_of(db, &server, (repo_root, path.as_deref()), vault)?;
    found.push(LocalMcpReadiness { server, standing });
  }
  Ok(found)
}

fn standing_of(
  db: &ProjectDb,
  server: &McpServer,
  (repo_root, path): (&Path, Option<&std::ffi::OsStr>),
  vault: &dyn SecretVault,
) -> Result<ReadinessStanding, ReadinessError> {
  let mut here = Here::nothing();
  let program = match server.transport() {
    McpTransport::Stdio(stdio) => {
      if let Some(found) = local::program_found(server.id(), stdio, (repo_root, path)) {
        here = found.add_to(here);
      }
      Some((server.id(), server.name().clone()))
    }
    McpTransport::Http(_) => None,
  };
  let mut secrets = Vec::new();
  for id in server.secrets() {
    let Some(secret) = db.secret(id)?.filter(|secret| !secret.is_archived()) else {
      return Ok(ReadinessStanding::Undecided(Undecided::SecretGone(id)));
    };
    match vault.exists(&secret.vault_key(db.project().id())) {
      Ok(true) => here = here.with_local_secret(id),
      Ok(false) => {}
      Err(error) => {
        tracing::warn!(server = %server.id(), secret = %id, %error, "the keychain did not say whether a server's secret has a value");
        return Ok(ReadinessStanding::Undecided(Undecided::SecretUnreadable(id)));
      }
    }
    secrets.push((id, secret.name().clone()));
  }
  Ok(ReadinessStanding::of(readiness(
    &Needs::local_mcp_server(program, secrets),
    &here,
  )))
}
