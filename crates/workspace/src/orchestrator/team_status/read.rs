//! One reading of the watched team tasks from the team server, in a session of the project's link.

use std::num::NonZeroU32;

use orchester_core::{id::TaskId, project::TaskKey};
use orchester_store::SharingLink;

use super::{TeamReading, TeamTask};
use crate::{
  sharing::Servers,
  team_reads::{TeamReadError, TeamSession},
};

/// What the team server says of `tasks` now. A session that cannot be made, or a read that fails, leaves the task it
/// failed on and every one after it unread: the server cannot be trusted to answer them. A task the server does not
/// have is neither read nor unread; its subscription is left to the person, as one whose task is gone.
pub(super) async fn tasks(servers: &Servers, link: &SharingLink, tasks: &[TaskId]) -> TeamReading {
  let session = match servers.team_session(link.server, link.project).await {
    Ok(session) => session,
    Err(error) => {
      tracing::info!(%error, "no team session to read the status of watched team tasks");
      return TeamReading {
        unreadable: tasks.iter().copied().collect(),
        ..TeamReading::default()
      };
    }
  };
  let mut reading = TeamReading::default();
  for (at, &id) in tasks.iter().enumerate() {
    match one(&session, link, id).await {
      Ok(Some(task)) => {
        reading.tasks.insert(id, task);
      }
      Ok(None) => {}
      Err(error) => {
        tracing::info!(%error, "cannot read the status of a watched team task");
        reading.unreadable.extend(tasks[at..].iter().copied());
        break;
      }
    }
  }
  reading
}

/// The team task `id` as the server reads it; `None` when the server has no such task.
async fn one(session: &TeamSession, link: &SharingLink, id: TaskId) -> Result<Option<TeamTask>, TeamReadError> {
  let read = match session.authority().task(id).await {
    Ok(read) => read,
    Err(error) => {
      return match TeamReadError::from(error) {
        TeamReadError::NotFound => Ok(None),
        error => Err(error),
      };
    }
  };
  // An answer from a session that ended meanwhile is another account's.
  if !session.current() {
    return Err(TeamReadError::SessionChanged);
  }
  let number = NonZeroU32::new(read.task.number).ok_or(TeamReadError::Unreadable)?;
  Ok(Some(TeamTask {
    key: TaskKey::new(link.key.clone(), number),
    title: read.task.content.title,
    status: read.task.content.workflow,
    module: read.task.content.module,
  }))
}
