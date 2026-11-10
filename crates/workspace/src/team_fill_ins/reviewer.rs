//! The connection a team reviewer runs on here. A language reviewer runs on an agent connection of its CLI kind, as a
//! profile does; a decision reviewer on a decision connection. The pick is kept in `team_connection_picks` under the
//! reviewer's id, so it is this computer's and no connection id of it ever reaches the team.

use std::path::Path;

use orchester_core::{
  id::{ConnectionId, DecisionConnectionId, ResourceId},
  resource::{ResourceKind, TeamReviewer},
  run::Answerer,
  sharing::ConnectionRole,
  store::ConnectionStore,
};
use orchester_store::{ConnectionPick, ProjectDb};

use super::{FillInError, connection::check_fits, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Keeps `connection` as the agent connection the team language reviewer `reviewer` of the project at `repo_root`
  /// runs on here, in place of an earlier pick. The connection must exist and be of the CLI kind the reviewer's newest
  /// version names. A reviewer takes no account: the connection's own runs it.
  pub fn pick_team_reviewer_connection(
    &self,
    repo_root: &Path,
    reviewer: ResourceId,
    connection: ConnectionId,
  ) -> Result<(), FillInError> {
    let mut db = ProjectDb::open(repo_root)?;
    let named = self.team_reviewer(&db, repo_root, reviewer)?;
    let found = db
      .connection(connection)?
      .ok_or(FillInError::ConnectionNotFound(connection))?;
    if matches!(named, Some(TeamReviewer::Decision { .. })) {
      return Err(FillInError::ReviewerIsDecision(reviewer));
    }
    check_fits(&found, named.and_then(|body| body.cli()).map(ConnectionRole::Agent))?;
    db.set_connection_pick(
      reviewer,
      ConnectionPick {
        connection,
        account: None,
      },
    )?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }

  /// Keeps `connection` as the decision connection the team decision reviewer `reviewer` runs on here. The connection's
  /// server must be one the reviewer's destinations allow (08 d). Sending the project's requests to a server that is
  /// not on this computer is a person's alone (`by`), as choosing it for a reviewer of the project's own is.
  pub fn pick_team_decision_connection(
    &self,
    repo_root: &Path,
    reviewer: ResourceId,
    connection: DecisionConnectionId,
    by: Answerer,
  ) -> Result<(), FillInError> {
    let mut db = ProjectDb::open(repo_root)?;
    let named = self.team_reviewer(&db, repo_root, reviewer)?;
    let found = db
      .decision_connection(connection)?
      .ok_or(FillInError::DecisionConnectionNotFound(connection))?;
    if matches!(named, Some(TeamReviewer::Language { .. })) {
      return Err(FillInError::ReviewerIsLanguage(reviewer));
    }
    if named.is_some_and(|body| !body.allows_server(found.base_url())) {
      return Err(FillInError::ServerNotAllowed(reviewer));
    }
    if !found.is_on_this_computer() {
      team::require_person(by)?;
    }
    db.set_decision_pick(reviewer, connection)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}

impl Workspace {
  /// The body of the latest version of the team reviewer `reviewer` of the project at `repo_root`, whose database `db`
  /// is; `None` when it has none. A version that is not a team reviewer body (one that names a connection) is
  /// [`FillInError::ReviewerNotTeamForm`].
  fn team_reviewer(
    &self,
    db: &ProjectDb,
    repo_root: &Path,
    reviewer: ResourceId,
  ) -> Result<Option<TeamReviewer>, FillInError> {
    team::server_of(db)?;
    let config = team::config(self, repo_root)?;
    match team::require_resource(&config, reviewer, ResourceKind::TeamReviewer)?.body::<TeamReviewer>() {
      None => Ok(None),
      Some(Ok(body)) => Ok(Some(body)),
      Some(Err(_)) => Err(FillInError::ReviewerNotTeamForm(reviewer)),
    }
  }
}
