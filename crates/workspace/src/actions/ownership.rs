//! Who owns an action's project and target, read from trusted local context: the project database's link to its
//! server project and the identities of its tasks, never from the action or a cached read. A project without a link
//! is local. A linked project is the team's, and its actions go to the server of the session given for it; without
//! one nothing happens, rather than a local change.

use orchester_api::v2::{AuthorityRef, Subject};
use orchester_core::{
  id::{ProjectId, ServerId, TaskId},
  task::identity::{TaskIdentity, TaskOwner},
};
use orchester_store::{ProjectDb, StoreError};

use super::ActionError;
use crate::team_reads::TeamSession;

pub(super) enum ProjectOwner {
  Local,
  Team(TeamLink),
}

/// The server project a team project is linked to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TeamLink {
  pub(crate) server: ServerId,
  pub(crate) project: ProjectId,
}

/// The owner of the project of `db`, checked against `session` for a team project: the session must be of the very
/// server and server project the project is linked to.
pub(super) fn project_owner(db: &ProjectDb, session: Option<&TeamSession>) -> Result<ProjectOwner, ActionError> {
  let Some(link) = db.sharing()? else {
    return Ok(ProjectOwner::Local);
  };
  let session = session.ok_or(ActionError::NoTeamSession)?;
  if session.server() != link.server || session.project() != link.project {
    return Err(ActionError::OtherProject);
  }
  if !session.current() {
    return Err(ActionError::SessionChanged);
  }
  Ok(ProjectOwner::Team(TeamLink {
    server: link.server,
    project: link.project,
  }))
}

/// The server's id of the team task this project names `task`: its recorded team identity's, or `task` itself for a
/// team task this computer never recorded. A task of this computer is never the team's.
pub(crate) fn team_task(db: &ProjectDb, link: TeamLink, task: TaskId) -> Result<TaskId, ActionError> {
  match db.task_identity(task)? {
    None => Ok(task),
    Some(TaskIdentity {
      owner: TaskOwner::Local,
      ..
    }) => Err(ActionError::LocalTask(task)),
    Some(TaskIdentity {
      owner: TaskOwner::Team { task: team, .. },
      ..
    }) => {
      if team.server != link.server || team.project != link.project {
        return Err(StoreError::OtherServerProject.into());
      }
      Ok(team.task)
    }
  }
}

/// `subject` with the server's id of its task.
pub(super) fn team_subject(db: &ProjectDb, link: TeamLink, subject: Subject) -> Result<Subject, ActionError> {
  match subject {
    Subject::Task { id } => Ok(Subject::Task {
      id: team_task(db, link, id)?,
    }),
    Subject::Module { .. } => Ok(subject),
  }
}

/// `authority` with the server's id of its subject's task.
pub(super) fn team_authority(
  db: &ProjectDb,
  link: TeamLink,
  authority: AuthorityRef,
) -> Result<AuthorityRef, ActionError> {
  Ok(AuthorityRef {
    subject: team_subject(db, link, authority.subject)?,
    ..authority
  })
}
