//! The connection (and account) a team profile runs on here.

use std::path::Path;

use orchester_core::{
  account::AccountCli,
  connection::{Connection, ConnectionKind},
  id::{AgentAccountId, ResourceId},
  resource::{AgentProfile, ResourceKind},
  sharing::ConnectionRole,
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ConnectionPick, ProjectDb, ReferenceTarget, StoreError};

use super::{FillInError, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Keeps `pick` as the connection, and the account when the connection needs one, the team profile `profile` of
  /// the project at `repo_root` runs on here, in place of an earlier pick. The connection must exist and, where the
  /// profile's CLI kind is known (its newest version names a connection of a role), be of that kind; the account must
  /// be a listed account of the connection's CLI.
  pub fn pick_team_connection(
    &self,
    repo_root: &Path,
    profile: ResourceId,
    pick: ConnectionPick,
  ) -> Result<(), FillInError> {
    let mut db = ProjectDb::open(repo_root)?;
    team::require_resource(&db, profile, ResourceKind::AgentProfile)?;
    let connection = db
      .connection(pick.connection)?
      .ok_or(FillInError::ConnectionNotFound(pick.connection))?;
    check_fits(&connection, profile_role(&db, profile)?)?;
    if let Some(account) = pick.account {
      self.check_account(&connection, account)?;
    }
    db.set_connection_pick(profile, pick)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }

  fn check_account(&self, connection: &Connection, account: AgentAccountId) -> Result<(), FillInError> {
    let listed = self
      .db
      .lock()
      .agent_accounts()?
      .into_iter()
      .find(|listed| listed.id() == account)
      .ok_or(FillInError::AccountNotFound(account))?;
    let cli = match connection.kind() {
      ConnectionKind::Agent(provider) => AccountCli::of(*provider),
      ConnectionKind::GitHub { .. } => None,
    };
    if cli == Some(listed.cli()) {
      Ok(())
    } else {
      Err(FillInError::AccountOfOtherCli(account))
    }
  }
}

/// The role of the connection the newest cached version of `profile` names, which is the profile's CLI kind until the
/// team names it directly; `None` when no version names one.
fn profile_role(db: &ProjectDb, profile: ResourceId) -> Result<Option<ConnectionRole>, StoreError> {
  let Some(latest) = db.latest_version::<AgentProfile>(profile)? else {
    return Ok(None);
  };
  let Some(cached) = db.cached_version::<AgentProfile>(profile, latest.number())? else {
    return Ok(None);
  };
  Ok(cached.references.iter().find_map(|reference| match reference.target {
    ReferenceTarget::Connection(_) => reference.role.as_deref().and_then(ConnectionRole::parse),
    _ => None,
  }))
}

/// Fails unless `connection` is of the `expected` role, or, when none is known, an agent connection.
fn check_fits(connection: &Connection, expected: Option<ConnectionRole>) -> Result<(), FillInError> {
  let role = ConnectionRole::of(connection.kind());
  let fits = match expected {
    Some(expected) => role == expected,
    None => matches!(role, ConnectionRole::Agent(_)),
  };
  if fits {
    Ok(())
  } else {
    Err(FillInError::ConnectionDoesNotFit {
      connection: connection.id(),
      expected,
    })
  }
}
