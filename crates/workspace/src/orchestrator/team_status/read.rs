//! One reading of the watched team tasks from the team server, in a session of the project's link.

use std::num::NonZeroU32;

use orchester_api::v2::{MAX_PAGE_LIMIT, PageRequest, TaskFilters, TaskListRequest, TaskRead};
use orchester_core::{
  id::{ModuleId, TaskId},
  project::TaskKey,
};
use orchester_store::SharingLink;

use super::{TeamReading, TeamTask, Wants};
use crate::{
  sharing::Servers,
  team_reads::{TeamReadError, TeamSession},
};

/// What the team server says of `wants` now. A session that cannot be made leaves everything unread. Once the server
/// shows it cannot be asked at all ([`stops_pass`]) the rest of the pass is left unread; any other failure leaves only
/// the task, or the module, it happened on unread, so one task the server refuses never blocks the others.
pub(super) async fn pass(servers: &Servers, link: &SharingLink, wants: &Wants) -> TeamReading {
  let mut reading = TeamReading {
    covered: wants.team.iter().copied().collect(),
    ..TeamReading::default()
  };
  let session = match servers.team_session(link.server, link.project).await {
    Ok(session) => session,
    Err(error) => {
      tracing::info!(%error, "no team session to read the status of watched team tasks");
      reading.unreadable.extend(wants.tasks.iter().copied());
      reading.unreadable_modules.extend(wants.modules.iter().copied());
      return reading;
    }
  };
  let mut reader = Reader {
    session: &session,
    link,
    member_seen: false,
    stopped: false,
  };
  for &id in &wants.tasks {
    match reader.task(id).await {
      Ok(Some(task)) => {
        reading.tasks.insert(id, task);
      }
      // A team task the server does not have: its subscription is left to the person, as one whose task is gone.
      Ok(None) => {}
      Err(Unread) => {
        reading.unreadable.insert(id);
      }
    }
  }
  for &module in &wants.modules {
    match reader.module(module, &wants.team).await {
      Ok(found) => {
        reading.listed.insert(module);
        reading.tasks.extend(found);
      }
      Err(Unread) => {
        reading.unreadable_modules.insert(module);
      }
    }
  }
  reading
}

/// Whether a failure shows that no further request of the pass can be answered: the server cannot be reached, signed
/// this computer out, ended the membership, needs a newer desktop, or does not have the project itself (a `404` on a
/// task is told from that before it gets here).
const fn stops_pass(error: &TeamReadError) -> bool {
  error.ends_reads()
    || matches!(
      error,
      TeamReadError::Unavailable { .. } | TeamReadError::NeedsVersion { .. } | TeamReadError::NotFound
    )
}

/// A read that gave no answer; the cause was logged.
struct Unread;

struct Reader<'a> {
  session: &'a TeamSession,
  link: &'a SharingLink,
  /// The server answered for the caller's capabilities in this pass, so a `404` on a task is about the task, not the
  /// project or the member.
  member_seen: bool,
  /// A failure showed the server cannot be asked: every later read of the pass is left unread.
  stopped: bool,
}

impl Reader<'_> {
  /// The team task `id` as the server reads it; `None` when the server has no such task.
  async fn task(&mut self, id: TaskId) -> Result<Option<TeamTask>, Unread> {
    if self.stopped {
      return Err(Unread);
    }
    let result = self.fetch_task(id).await;
    self.settle(result, "a watched team task")
  }

  /// The tasks of `team` the server puts in `module`, by their ids: the pages of its list filtered to the module.
  async fn module(&mut self, module: ModuleId, team: &[TaskId]) -> Result<Vec<(TaskId, TeamTask)>, Unread> {
    if self.stopped {
      return Err(Unread);
    }
    let result = self.fetch_module(module, team).await;
    self.settle(result, "the tasks of a watched module")
  }

  /// Logs a failure, and stops the pass when the server cannot be asked at all.
  fn settle<T>(&mut self, result: Result<T, TeamReadError>, what: &str) -> Result<T, Unread> {
    result.map_err(|error| {
      tracing::info!(%error, "cannot read {what}");
      self.stopped = stops_pass(&error);
      Unread
    })
  }

  async fn fetch_task(&mut self, id: TaskId) -> Result<Option<TeamTask>, TeamReadError> {
    match self.session.authority().task(id).await {
      Ok(read) => {
        self.ensure_current()?;
        team_task(self.link, &read).map(Some)
      }
      Err(error) => match TeamReadError::from(error) {
        TeamReadError::NotFound => self.task_is_gone().await.map(|()| None),
        error => Err(error),
      },
    }
  }

  /// The server answered a task `404`. It is the task that is not there only when the caller's capabilities can be
  /// read: the same answer about the project or the member means no task can be read.
  async fn task_is_gone(&mut self) -> Result<(), TeamReadError> {
    if !self.member_seen {
      self
        .session
        .authority()
        .capabilities()
        .await
        .map_err(TeamReadError::from)?;
      self.member_seen = true;
    }
    self.ensure_current()
  }

  async fn fetch_module(&self, module: ModuleId, team: &[TaskId]) -> Result<Vec<(TaskId, TeamTask)>, TeamReadError> {
    let mut found = Vec::new();
    let mut after = None;
    loop {
      let request = TaskListRequest {
        filters: TaskFilters {
          modules: vec![Some(module)],
          ..TaskFilters::default()
        },
        page: PageRequest::new(after, Some(MAX_PAGE_LIMIT)).expect("the largest page limit is in range"),
        ..TaskListRequest::default()
      };
      let page = self.session.authority().tasks(&request).await?;
      self.ensure_current()?;
      for read in page.items.iter().filter(|read| team.contains(&read.task.id)) {
        found.push((read.task.id, team_task(self.link, read)?));
      }
      match page.next {
        Some(next) => after = Some(next),
        None => return Ok(found),
      }
    }
  }

  /// An answer from a session that ended meanwhile is another account's.
  fn ensure_current(&self) -> Result<(), TeamReadError> {
    if self.session.current() {
      Ok(())
    } else {
      Err(TeamReadError::SessionChanged)
    }
  }
}

fn team_task(link: &SharingLink, read: &TaskRead) -> Result<TeamTask, TeamReadError> {
  let number = NonZeroU32::new(read.task.number).ok_or(TeamReadError::Unreadable)?;
  Ok(TeamTask {
    key: TaskKey::new(link.key.clone(), number),
    title: read.task.content.title.clone(),
    status: read.task.content.workflow,
    module: read.task.content.module,
  })
}
