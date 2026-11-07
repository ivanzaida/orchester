//! Secret values the team's configuration names, given here.

use std::path::Path;

use orchester_core::secret::{SecretName, SecretValue, SecretVault};
use orchester_store::ProjectDb;

use super::{FillInError, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Gives the team secret `name` its value in the project at `repo_root`, replacing an earlier one; the value goes
  /// to `vault` and nowhere else. Neither the error nor its message holds the value.
  pub fn set_team_secret_value(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
    name: &SecretName,
    value: &SecretValue,
  ) -> Result<(), FillInError> {
    let mut db = ProjectDb::open(repo_root)?;
    team::server_of(&db)?;
    db.set_team_secret_value(vault, name, value)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}
