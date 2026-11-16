//! What every fill-in checks first: that the project is a team's and that what it names is the team's, in the team's
//! configuration as its team server was read this session.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, ServerId},
  resource::ResourceKind,
  run::Answerer,
};
use orchester_store::ProjectDb;

use super::FillInError;
use crate::{
  team_config::{TeamConfig, TeamRead, TeamResource},
  workspace::Workspace,
};

/// The team server the project is shared on.
pub(super) fn server_of(db: &ProjectDb) -> Result<ServerId, FillInError> {
  db.sharing()?.map(|link| link.server).ok_or(FillInError::NotTeamProject)
}

/// The configuration of the team project at `repo_root` read this session; refused while it is being read or could not
/// be read.
pub(super) fn config(workspace: &Workspace, repo_root: &Path) -> Result<Arc<TeamConfig>, FillInError> {
  match workspace.team_read(repo_root) {
    TeamRead::Read { config, .. } => Ok(config),
    TeamRead::Reading => Err(FillInError::TeamReading),
    TeamRead::Unreachable(unreached) => Err(FillInError::TeamUnreachable(unreached)),
  }
}

/// `resource`, which must be a team resource of `kind` the team has not archived.
pub(super) fn require_resource(
  config: &TeamConfig,
  resource: ResourceId,
  kind: ResourceKind,
) -> Result<&TeamResource, FillInError> {
  config
    .live(resource)
    .filter(|found| found.kind == kind)
    .ok_or(FillInError::NotTeamResource { resource, kind })
}

/// Fails unless `by` is the person.
pub(super) const fn require_person(by: Answerer) -> Result<(), FillInError> {
  match by {
    Answerer::Person => Ok(()),
    Answerer::Automation => Err(FillInError::PersonOnly),
  }
}
