//! A team project's reads straight from its server (`docs/architecture/team-authority-plan.md` §6, milestone P5): the
//! S10 bounded reads by area ([`tasks`], [`modules`], [`library`], [`metadata`]) and the S11 invalidation subscription
//! ([`follow`]), with no copy of team data on disk and no full-project download. The desktop's Tasks list and board
//! read team tasks through them (P6, its lurq queries); the other screens and team projects' event stream and cache
//! ([`crate::sharing::sync`]) follow in later milestones. [`listed`] turns a listing into the tasks the lists show.
//!
//! The sequence is race-safe ([`freshness`]): [`TeamProject::follow`] subscribes and takes the stream's cursor, reads
//! wait for that cursor and are stamped with the snapshot they answer at, an invalidation that arrives while a read is
//! in flight makes it read again, and a `reset` or a reconnect starts a new epoch in which every earlier read is stale.
//! [`TeamProject::changes`] coalesces bursts of invalidations for the consumer. A read or the subscription that finds
//! the session changed or the membership revoked ends the project's reads for good; nothing read before is returned.

mod error;
mod follow;
mod freshness;
pub mod library;
pub mod listed;
pub mod metadata;
pub mod modules;
pub mod tasks;

use std::sync::Arc;

use orchester_api::{
  client::{Authority, AuthorityCallError, Client},
  v2::{EventCursor, QueryScope},
};
use orchester_core::{id::ProjectId, secret::SecretValue};
use parking_lot::Mutex;
use tokio::sync::watch;

use self::freshness::{Finish, Live, Taken, Ticket};
pub use self::{error::TeamReadError, freshness::Changes};

/// How many times a read invalidated in flight is read again before [`TeamReadError::KeptChanging`].
const READ_ATTEMPTS: usize = 3;

/// The signed-in session a team project is read in: the server's client and token, and the session's generation as
/// its owner counts it. The owner raises the generation when the person signs out, switches account or server; the
/// reads and the subscription of the generation this session was made in then end with
/// [`TeamReadError::SessionChanged`].
pub struct TeamSession {
  client: Client,
  token: SecretValue,
  project: ProjectId,
  generation: watch::Receiver<u64>,
  bound: u64,
}

impl TeamSession {
  /// The session of `project` with `client` and `token`, in the generation `generation` holds now.
  pub fn new(client: Client, token: SecretValue, project: ProjectId, generation: watch::Receiver<u64>) -> Self {
    let bound = *generation.borrow();
    Self {
      client,
      token,
      project,
      generation,
      bound,
    }
  }

  /// Whether the session is still the one this was made in; a generation whose owner is gone has ended.
  fn current(&self) -> bool {
    self.generation.has_changed().is_ok() && *self.generation.borrow() == self.bound
  }

  fn authority(&self) -> Authority<'_> {
    self.client.authority(&self.token, self.project)
  }
}

/// One read's answer with what tells whether it is still current: the scope it belongs to, the cursor it was taken
/// at and the subscription's epoch then.
#[derive(Clone, Debug)]
pub struct TeamRead<T> {
  value: T,
  scope: QueryScope,
  taken: EventCursor,
  epoch: u64,
}

impl<T> TeamRead<T> {
  pub const fn value(&self) -> &T {
    &self.value
  }

  pub fn into_value(self) -> T {
    self.value
  }

  pub const fn scope(&self) -> QueryScope {
    self.scope
  }

  /// The stream position the answer holds everything up to.
  pub const fn taken(&self) -> EventCursor {
    self.taken
  }

  /// Whether `changes` make this read stale: a reset since, or an invalidation of its scope after it was taken.
  pub fn is_stale(&self, changes: &Changes) -> bool {
    changes.stales(&self.scope, self.taken, self.epoch)
  }
}

/// A team project's reads and its subscription in one session. Clones share them.
#[derive(Clone)]
pub struct TeamProject {
  inner: Arc<Inner>,
}

struct Inner {
  session: TeamSession,
  live: Mutex<Live>,
  /// Counts every change of `live` that a waiting read or consumer may care about.
  ticks: watch::Sender<u64>,
}

impl TeamProject {
  pub fn new(session: TeamSession) -> Self {
    Self {
      inner: Arc::new(Inner {
        session,
        live: Mutex::new(Live::default()),
        ticks: watch::channel(0).0,
      }),
    }
  }

  /// What changed since the last call, waiting until something did; the reason once the reads ended. Meant for one
  /// consumer: a call takes what it returns.
  pub async fn changes(&self) -> Result<Changes, TeamReadError> {
    let mut ticks = self.inner.ticks.subscribe();
    let mut generation = self.inner.session.generation.clone();
    loop {
      if !self.inner.session.current() {
        return Err(self.end(TeamReadError::SessionChanged));
      }
      if let Some(changes) = self.inner.live.lock().take_changes() {
        return changes;
      }
      tokio::select! {
        // The sender lives as long as `self`.
        _ = ticks.changed() => {}
        _ = generation.changed() => {}
      }
    }
  }

  /// Ends the reads with `error` unless they ended already, waking whoever waits; returns how they ended.
  fn end(&self, error: TeamReadError) -> TeamReadError {
    let ended = self.inner.live.lock().end(error);
    self.tick();
    ended
  }

  fn tick(&self) {
    self.inner.ticks.send_modify(|ticks| *ticks += 1);
  }

  /// Reads `scope` with `call`, again while it is invalidated in flight, at most [`READ_ATTEMPTS`] times. Once the
  /// session changed or the reads ended meanwhile, neither the answer nor its error is returned, only that end.
  async fn read<T, F, Fut>(&self, scope: QueryScope, call: F) -> Result<TeamRead<T>, TeamReadError>
  where
    T: Taken,
    F: Fn(Arc<Inner>) -> Fut,
    Fut: Future<Output = Result<T, AuthorityCallError>>,
  {
    for _ in 0..READ_ATTEMPTS {
      let ticket = self.ticket(scope).await?;
      let answer = call(self.inner.clone()).await;
      let finished = self
        .inner
        .live
        .lock()
        .finish(&ticket, answer.as_ref().ok().and_then(Taken::cursor));
      if !self.inner.session.current() {
        return Err(self.end(TeamReadError::SessionChanged));
      }
      let finished = finished?;
      let value = answer.map_err(|error| self.refused(error.into()))?;
      match finished {
        Finish::Current { taken, epoch } => {
          return Ok(TeamRead {
            value,
            scope,
            taken,
            epoch,
          });
        }
        Finish::Stale => tracing::debug!(?scope, "a team read was invalidated in flight and is read again"),
      }
    }
    Err(TeamReadError::KeptChanging)
  }

  /// A read of `scope` in flight, once the subscription has a position.
  async fn ticket(&self, scope: QueryScope) -> Result<Ticket, TeamReadError> {
    let mut ticks = self.inner.ticks.subscribe();
    let mut generation = self.inner.session.generation.clone();
    loop {
      if !self.inner.session.current() {
        return Err(self.end(TeamReadError::SessionChanged));
      }
      if let Some(ticket) = self.inner.live.lock().begin(scope)? {
        return Ok(ticket);
      }
      tokio::select! {
        // The sender lives as long as `self`.
        _ = ticks.changed() => {}
        _ = generation.changed() => {}
      }
    }
  }

  /// A read's refusal; one that ends the reads ends them.
  fn refused(&self, error: TeamReadError) -> TeamReadError {
    if error.ends_reads() {
      return self.end(error);
    }
    error
  }
}
