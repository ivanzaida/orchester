//! What this computer fills in after Share, from what it had locally (`docs/design/team-configuration.md`, "What a
//! computer fills in"), so the sharer's runs stay ready: the connection each profile ran on is its pick, each team MCP
//! server that was on is on, the program path of one started by an absolute path is set for the team server, and the
//! value of each secret a server sends is kept under the secret's name. The values are read from the keychain before
//! anything is uploaded, so a keychain that refuses stops the Share with nothing sent.
//!
//! Share is the person's own action in the window, so setting these here widens nothing: the servers, programs and
//! secrets are the ones the profiles already ran with.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::ConnectionKind,
  id::{ResourceId, ServerId},
  resource::{AgentProfile, ResourceKind},
  secret::{SecretName, SecretValue, SecretVault, VaultError},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ConnectionPick, ProjectDb};

use super::{refresh_profile, share_mcp::McpPlan};
use crate::{sharing::SharingError, tasks::outside::Outside, workspace::Workspace};

/// The fill-ins of a Share, read from the project before it is uploaded. It holds secret values, so it has no `Debug`.
pub struct ShareFillIns {
  picks: Vec<(ResourceId, ConnectionPick)>,
  programs: Vec<(ResourceId, PathBuf)>,
  on: Vec<ResourceId>,
  secrets: Vec<(SecretName, SecretValue)>,
}

impl ShareFillIns {
  /// What the project at `root` fills in once it is shared. Blocking.
  pub fn read(root: &Path, vault: &dyn SecretVault) -> Result<Self, SharingError> {
    let db = ProjectDb::open(root)?;
    let plan = McpPlan::read(&db)?;
    let mut fill = Self {
      picks: picks(&db)?,
      programs: Vec::new(),
      on: Vec::new(),
      secrets: Vec::new(),
    };
    for server in plan.servers() {
      if let Some(path) = &server.program_path {
        fill.programs.push((server.resource, path.clone()));
      }
      if server.enabled {
        fill.on.push(server.resource);
      }
      for secret in &server.secrets {
        if fill.secrets.iter().any(|(name, _)| name == secret.name()) {
          continue;
        }
        match vault.read(&secret.vault_key(db.project().id())) {
          Ok(value) => fill.secrets.push((secret.name().clone(), value)),
          // The secret has no value here already, so there is none to carry; the team secret shows as missing.
          Err(VaultError::NotFound) => {
            tracing::warn!(secret = %secret.name(), "a secret of a shared MCP server has no value on this computer");
          }
          Err(error) => return Err(error.into()),
        }
      }
    }
    Ok(fill)
  }

  /// Keeps them for the project at `root`, shared on `server`. Blocking.
  pub fn apply(
    self,
    (workspace, vault): (&Workspace, &dyn SecretVault),
    root: &Path,
    server: ServerId,
  ) -> Result<(), SharingError> {
    let mut db = ProjectDb::open(root)?;
    for (profile, pick) in &self.picks {
      db.set_connection_pick(*profile, *pick)?;
      // The version the sharer kept still holds the local body it was published with; the pick makes it the team's.
      refresh_profile(&mut db, *profile)?;
    }
    for resource in &self.on {
      db.set_team_mcp_on_here(*resource, true)?;
    }
    for (name, value) in &self.secrets {
      db.set_team_secret_value(vault, name, value)?;
    }
    apply_programs(&mut workspace.db.lock(), server, &self.programs)?;
    workspace.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}

fn apply_programs(app: &mut AppDb, server: ServerId, programs: &[(ResourceId, PathBuf)]) -> Result<(), SharingError> {
  for (resource, path) in programs {
    app.set_mcp_server_program(server, *resource, path, Timestamp::now())?;
  }
  Ok(())
}

/// The connection of each profile's newest published version, which the profile keeps running on.
fn picks(db: &ProjectDb) -> Result<Vec<(ResourceId, ConnectionPick)>, SharingError> {
  let connections = db.connections()?;
  let mut picks = Vec::new();
  for profile in db.resources(ResourceKind::AgentProfile)? {
    let Some(newest) = db.latest_version::<AgentProfile>(profile.id())? else {
      continue;
    };
    let connection = newest.body().connection();
    let agent = connections
      .iter()
      .any(|found| found.id() == connection && matches!(found.kind(), ConnectionKind::Agent(_)));
    if agent {
      picks.push((
        profile.id(),
        ConnectionPick {
          connection,
          account: None,
        },
      ));
    }
  }
  Ok(picks)
}
