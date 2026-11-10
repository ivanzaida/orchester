//! Following one shared project's event stream (`docs/architecture/server.md`, "Live updates"): connected from the
//! cache's cursor, or without one the first time, read until it ends, and connected again after a wait that starts at
//! 1 s and doubles to 60 s, with jitter, the connection monitor's [`Schedule`]. A stream that opened starts the waits
//! over. `401` stops it signed out; `403` and `revoked` stop it and the list reads *not a member*. Each step says the
//! project's [`SyncState`]: *live* once the stream opened, *reconnecting* or *offline* while it waits, *not a member*.
//! While the stream is open, this computer's catalog is reported beside it ([`catalog`]).

use std::{path::Path, time::Duration};

use orchester_api::{
  CatalogReport, Event, SharedResource, SharedTask, Versions,
  client::{EventStream, StreamEnd},
};
use orchester_core::{
  id::{ResourceId, TaskId},
  resource::VersionNumber,
  time::Timestamp,
};

use super::{
  SyncState,
  apply::{self, Applied, Pages},
  catalog::{self, CATALOG_CHECK},
  live,
};
use crate::{
  background,
  messages::Message,
  sharing::{Schedule, SharingError, connection},
  team_config::TeamConfigs,
};

/// A shared project's server as its stream reads it, signed in as the person.
pub trait Source: Send + Sync {
  /// Opens the project's stream after `cursor`; without one the snapshot comes first.
  fn open(&self, cursor: Option<u64>) -> impl Future<Output = Result<EventStream, StreamEnd>> + Send;

  /// Reads `task` as the server has it now.
  fn task(&self, task: TaskId) -> impl Future<Output = Result<SharedTask, SharingError>> + Send;

  /// Reads every resource of the project as the server has it now.
  fn resources(&self) -> impl Future<Output = Result<Vec<SharedResource>, SharingError>> + Send;

  /// Reads one page of `resource`'s versions after `after`, from v1 without it.
  fn versions(
    &self,
    resource: ResourceId,
    after: Option<VersionNumber>,
  ) -> impl Future<Output = Result<Versions, SharingError>> + Send;

  /// Sends this computer's whole catalog for the project (`PUT …/catalog`).
  fn report_catalog(&self, report: CatalogReport) -> impl Future<Output = Result<(), SharingError>> + Send;

  /// The team's `resource` was archived on the server and is removed here now: the runs that pin it stop. `by` is who
  /// archived it, when the server said (a snapshot does not).
  fn archived(&self, resource: ResourceId, by: Option<&str>) -> impl Future<Output = ()> + Send;

  /// The cache took a change from the stream.
  fn changed(&self);

  /// The project's stream reached `state`.
  fn state(&self, state: SyncState);

  /// Where the team's configuration read from the server is kept this session ([`live`]).
  fn configs(&self) -> &TeamConfigs;

  /// What to tell the person when the server could not be reached for `error`.
  fn unreached(&self, error: &SharingError) -> Message;

  /// Ends when the person asks to reach the server again now (*Try again*): a stream that waits to connect again
  /// connects at once.
  fn retried(&self) -> impl Future<Output = ()> + Send;
}

/// The waits between connections: the system clock in the app, a settable one in tests.
pub trait Clock: Send + Sync {
  fn wait(&self, wait: Duration) -> impl Future<Output = ()> + Send;

  /// A random share of the jitter, 0 to 1.
  fn jitter(&self) -> f64;

  /// How often an open stream reads the catalog again to report a change ([`CATALOG_CHECK`]).
  fn catalog_check(&self) -> Duration {
    CATALOG_CHECK
  }
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
  // When the connects that failed in a row began; offline counts from it.
  let mut trouble_since = None;
  loop {
    let (opened, ending) = connect(source, root, clock).await;
    if let Some(error) = unreached(&ending) {
      live::unreached(source, root, &error);
    }
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
        source.state(SyncState::NotMember);
        return Stopped::NotMember;
      }
      Ending::Stream(end) => tracing::debug!(?end, "a project's event stream ended"),
      Ending::Cache(error) => tracing::warn!(%error, "a project's events could not be applied"),
    }
    if opened {
      (failures, trouble_since) = (1, None);
    } else {
      failures += 1;
    }
    let since = *trouble_since.get_or_insert_with(Timestamp::now);
    source.state(SyncState::after_failure(failures, since));
    tokio::select! {
      () = clock.wait(Schedule::DEFAULT.backoff(failures, clock.jitter())) => {}
      () = source.retried() => tracing::debug!("a project's stream connects again as asked"),
    }
  }
}

/// Why the team's configuration cannot be read after a connection ended as `ending`; `None` for an ending that says
/// nothing of the server (the cache failed here).
fn unreached(ending: &Ending) -> Option<SharingError> {
  match ending {
    Ending::Stream(StreamEnd::Unauthorized) => Some(SharingError::SignedOut),
    Ending::Stream(StreamEnd::Forbidden | StreamEnd::Revoked) => Some(SharingError::NotMember),
    Ending::Stream(StreamEnd::Failed(error)) => Some(SharingError::Client(error.clone())),
    Ending::Stream(StreamEnd::Closed | StreamEnd::Unreadable) => Some(SharingError::Offline),
    Ending::Cache(_) => None,
  }
}

/// One connection from the cache's cursor, read until it ends: whether it opened, and how it ended. While it is open,
/// the catalog is reported on connect and after each change `clock`'s checks find.
async fn connect(source: &impl Source, root: &Path, clock: &impl Clock) -> (bool, Ending) {
  let cursor = match blocking(root, apply::cursor).await {
    Ok(cursor) => cursor,
    Err(error) => return (false, Ending::Cache(error)),
  };
  tracing::info!(?cursor, "a project's event stream connects");
  match source.open(cursor).await {
    Ok(mut stream) => {
      source.state(SyncState::Live);
      let ending = tokio::select! {
        ending = resume(source, root, &mut stream) => ending,
        never = catalog::keep_reported(source, root, clock.catalog_check()) => match never {},
      };
      (true, ending)
    }
    Err(end) => (false, Ending::Stream(end)),
  }
}

/// Reads the team's configuration into memory, then applies the events of `stream` until it ends. A configuration that
/// cannot be read now does not hold the tasks' deltas: the next connect reads it again.
async fn resume(source: &impl Source, root: &Path, stream: &mut EventStream) -> Ending {
  // A read refused because the session or the membership just ended is said unreachable here; the stream itself then
  // ends as the server says (`revoked`, `401`, `403`), which tells signed out from not a member.
  match live::fill(source, root).await {
    Ok(()) => {
      source.changed();
      stop_archived(source, root).await;
    }
    Err(error) => {
      tracing::warn!(%error, "the team's configuration could not be read; the next connect reads it");
      live::unreached(source, root, &error);
    }
  }
  read(source, root, stream).await
}

/// Stops the runs here that pin a resource the team archived, as the configuration read when the stream opened says:
/// one archived while this computer was not following the project stops its runs now. A run stopped already is not
/// running, so nothing is stopped twice; the server's read does not say who archived it.
async fn stop_archived(source: &impl Source, root: &Path) {
  let archived: Vec<_> = match source.configs().read(root).config() {
    Some(config) => config.all_archived().map(|resource| resource.id).collect(),
    None => return,
  };
  for resource in archived {
    source.archived(resource, None).await;
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
        snapshot(root, pages, end.cursor).await
      }
      event => match live::follow(source, root, &event).await {
        Ok(changed) => {
          if changed {
            source.changed();
          }
          // A resource archived only now stops the runs here that pin it.
          if let (true, Event::ResourceArchived(delta)) = (changed, &event)
            && delta.change.archived
          {
            source
              .archived(delta.change.resource, Some(&delta.by.display_name))
              .await;
          }
          blocking(root, move |root| apply::delta(root, &event)).await
        }
        Err(error) => Err(error),
      },
    };
    let result = match applied {
      Ok(Applied::Unchanged) => continue,
      Ok(Applied::Changed) => Ok(()),
      Ok(Applied::Read { task, cursor }) => read_again(source, root, task, cursor).await,
      Err(error) => Err(error),
    };
    match result {
      Ok(()) => source.changed(),
      Err(error) => return ended(error),
    }
  }
}

/// How a connection ends on `error` reading the server or the cache.
fn ended(error: SharingError) -> Ending {
  match error {
    SharingError::SignedOut => Ending::Stream(StreamEnd::Unauthorized),
    SharingError::Client(error) => Ending::Stream(StreamEnd::Failed(error)),
    error => Ending::Cache(error),
  }
}

/// Takes the snapshot at `cursor`; the team's configuration it names was read into memory when the stream opened.
async fn snapshot(root: &Path, pages: Pages, cursor: u64) -> Result<Applied, SharingError> {
  tracing::info!(
    resources = pages.resources().len(),
    cursor,
    "a project's snapshot ended"
  );
  blocking(root, move |root| {
    apply::snapshot(root, &pages, cursor).map(|()| Applied::Changed)
  })
  .await
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

pub(super) async fn blocking<T: Send + 'static>(
  root: &Path,
  work: impl FnOnce(&Path) -> Result<T, SharingError> + Send + 'static,
) -> Result<T, SharingError> {
  let root = root.to_owned();
  background::run(move || work(&root)).await?
}
