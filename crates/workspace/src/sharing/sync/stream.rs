//! Following one shared project's event stream (`docs/architecture/server.md`, "Live updates"): connected from the
//! cache's cursor, or without one the first time, read until it ends, and connected again after a wait that starts at
//! 1 s and doubles to 60 s, with jitter, the connection monitor's [`Schedule`]. A stream that opened starts the waits
//! over. `401` stops it signed out; `403` and `revoked` stop it and the list reads *not a member*.

use std::{path::Path, time::Duration};

use orchester_api::{
  Event, SharedTask,
  client::{EventStream, StreamEnd},
};
use orchester_core::id::TaskId;

use super::apply::{self, Applied, Pages};
use crate::{
  background,
  sharing::{Schedule, SharingError, connection},
};

/// A shared project's server as its stream reads it, signed in as the person.
pub trait Source: Send + Sync {
  /// Opens the project's stream after `cursor`; without one the snapshot comes first.
  fn open(&self, cursor: Option<u64>) -> impl Future<Output = Result<EventStream, StreamEnd>> + Send;

  /// Reads `task` as the server has it now.
  fn task(&self, task: TaskId) -> impl Future<Output = Result<SharedTask, SharingError>> + Send;

  /// The cache took a change from the stream.
  fn changed(&self);
}

/// The waits between connections: the system clock in the app, a settable one in tests.
pub trait Clock: Send + Sync {
  fn wait(&self, wait: Duration) -> impl Future<Output = ()> + Send;

  /// A random share of the jitter, 0 to 1.
  fn jitter(&self) -> f64;
}

/// Tokio's timer and a random jitter.
pub struct SystemClock;

impl Clock for SystemClock {
  async fn wait(&self, wait: Duration) {
    tokio::time::sleep(wait).await;
  }

  fn jitter(&self) -> f64 {
    connection::jitter()
  }
}

/// Why a stream stopped for good; until then it connects again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
  /// `401`: the server no longer accepts this computer's token.
  SignedOut,
  /// `403` or `revoked`: the person is no longer a member, which the cache now says.
  NotMember,
}

/// How one connection ended.
enum Ending {
  Stream(StreamEnd),
  /// The cache could not be read or written; the stream connects again from the cursor it has.
  Cache(SharingError),
}

/// Follows the stream of the project whose cache is at `root` until it stops for good.
pub async fn follow(source: &impl Source, root: &Path, clock: &impl Clock) -> Stopped {
  let mut failures = 0;
  loop {
    let (opened, ending) = connect(source, root).await;
    match ending {
      Ending::Stream(StreamEnd::Unauthorized) => return Stopped::SignedOut,
      Ending::Stream(end) if apply::ends_membership(&end) => {
        let root = root.to_owned();
        match background::run(move || apply::not_member(&root)).await {
          Ok(Ok(())) => source.changed(),
          Ok(Err(error)) => tracing::error!(%error, "cannot record that the person is no longer a member"),
          // The blocking pool is shutting down with the window.
          Err(_) => {}
        }
        return Stopped::NotMember;
      }
      Ending::Stream(end) => tracing::debug!(?end, "a project's event stream ended"),
      Ending::Cache(error) => tracing::warn!(%error, "a project's events could not be applied"),
    }
    failures = if opened { 1 } else { failures + 1 };
    clock.wait(Schedule::DEFAULT.backoff(failures, clock.jitter())).await;
  }
}

/// One connection from the cache's cursor, read until it ends: whether it opened, and how it ended.
async fn connect(source: &impl Source, root: &Path) -> (bool, Ending) {
  let cursor = match blocking(root, apply::cursor).await {
    Ok(cursor) => cursor,
    Err(error) => return (false, Ending::Cache(error)),
  };
  match source.open(cursor).await {
    Ok(mut stream) => (true, read(source, root, &mut stream).await),
    Err(end) => (false, Ending::Stream(end)),
  }
}

/// Applies the events of `stream` until it ends.
async fn read(source: &impl Source, root: &Path, stream: &mut EventStream) -> Ending {
  let mut pages: Option<Pages> = None;
  loop {
    let event = match stream.next().await {
      Ok(event) => event,
      Err(end) => return Ending::Stream(end),
    };
    let applied = match event {
      Event::SnapshotBegin(begin) => {
        pages = Some(Pages::new(begin));
        continue;
      }
      Event::SnapshotTasks(page) => {
        let Some(pages) = pages.as_mut() else {
          return Ending::Stream(StreamEnd::Unreadable);
        };
        pages.extend(page.tasks);
        continue;
      }
      Event::SnapshotEnd(end) => {
        let Some(pages) = pages.take() else {
          return Ending::Stream(StreamEnd::Unreadable);
        };
        blocking(root, move |root| {
          apply::snapshot(root, &pages, end.cursor).map(|()| Applied::Changed)
        })
        .await
      }
      event => blocking(root, move |root| apply::delta(root, &event)).await,
    };
    let result = match applied {
      Ok(Applied::Unchanged) => continue,
      Ok(Applied::Changed) => Ok(()),
      Ok(Applied::Read { task, cursor }) => read_again(source, root, task, cursor).await,
      Err(error) => Err(error),
    };
    match result {
      Ok(()) => source.changed(),
      Err(SharingError::SignedOut) => return Ending::Stream(StreamEnd::Unauthorized),
      Err(SharingError::Client(error)) => return Ending::Stream(StreamEnd::Failed(error)),
      Err(error) => return Ending::Cache(error),
    }
  }
}

/// Reads `task` again and writes it with `cursor`; a task the server no longer has only moves the cursor.
async fn read_again(source: &impl Source, root: &Path, task: TaskId, cursor: u64) -> Result<(), SharingError> {
  match source.task(task).await {
    Ok(shared) => blocking(root, move |root| apply::read(root, &shared, cursor)).await,
    Err(error) if error.code() == Some(orchester_api::ErrorCode::NotFound) => {
      blocking(root, move |root| apply::skip(root, cursor)).await
    }
    Err(error) => Err(error),
  }
}

async fn blocking<T: Send + 'static>(
  root: &Path,
  work: impl FnOnce(&Path) -> Result<T, SharingError> + Send + 'static,
) -> Result<T, SharingError> {
  let root = root.to_owned();
  background::run(move || work(&root)).await?
}
