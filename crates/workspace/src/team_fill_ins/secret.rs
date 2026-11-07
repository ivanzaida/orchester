//! Secret values the team's configuration names, given here.

use std::path::Path;

use orchester_core::{
  run::Answerer,
  secret::{SecretName, SecretValue, SecretVault},
};
use orchester_store::ProjectDb;

use super::{FillInError, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Gives the team secret `name` its value in the project at `repo_root`, replacing an earlier one; the value goes
  /// to `vault` and nowhere else. Neither the error nor its message holds the value. A secret's value is a person's
  /// alone (`by`): automation never types a secret, and the MCP tree and the log never hold one. Share sets the values
  /// its servers already ran with in its own action (`sharing::sync::config`), not through this.
  pub fn set_team_secret_value(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
    name: &SecretName,
    value: &SecretValue,
    by: Answerer,
  ) -> Result<(), FillInError> {
    team::require_person(by)?;
    let mut db = ProjectDb::open(repo_root)?;
    team::server_of(&db)?;
    db.set_team_secret_value(vault, name, value)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}
