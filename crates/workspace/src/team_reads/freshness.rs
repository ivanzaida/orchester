//! Which reads of a team project are current (`docs/cloud-api.md`, "Team authority S11 subscriptions", client race
//! rule). The subscription moves a position ([`Live::seen`]) and an epoch, which every `reset` and every reconnect
//! raises. A read is issued at the position seen then and answers at a snapshot cursor of its own, or at least at that
//! position; an `invalidated` event for its scope with a later cursor while it is in flight makes it stale, and so does
//! a new epoch, so an in-flight read that finishes after an invalidation never makes an older revision current.
//! Invalidations between the consumer's looks coalesce, per scope, at their latest cursor ([`Changes`]).

use std::collections::{BTreeMap, HashMap};

use orchester_api::v2::{
  Capabilities, EventCursor, ModuleBurndownRead, ModuleProgressRead, ModuleSummary, Page, QueryScope, TaskBoardCounts,
  TaskRead, WorkspaceMetadata,
};

use super::TeamReadError;

/// What the subscription and the reads in flight have told so far.
#[derive(Default)]
pub(super) struct Live {
  /// Raised by every `reset` and every reconnect: a read of an older epoch is stale.
  epoch: u64,
  /// The latest cursor of the subscription; `None` before its first `ready`.
  seen: Option<EventCursor>,
  /// Why the last subscription attempt failed while there is no position yet: reads fail with it instead of waiting.
  unready: Option<TeamReadError>,
  next_read: u64,
  in_flight: HashMap<u64, InFlight>,
  /// The invalidations the consumer has not taken yet, each scope at its latest cursor.
  pending: BTreeMap<QueryScope, EventCursor>,
  /// The epoch of the last [`Changes`] taken.
  delivered: u64,
  ended: Option<TeamReadError>,
}

struct InFlight {
  scope: QueryScope,
  /// The latest invalidation of the read's scope that arrived while it was in flight.
  hit: Option<EventCursor>,
}

/// A read in flight, from [`Live::begin`] to [`Live::finish`].
pub(super) struct Ticket {
  id: u64,
  epoch: u64,
  /// The position the read was issued at: its answer holds at least what that position holds.
  floor: EventCursor,
}

/// How a read in flight finished.
pub(super) enum Finish {
  Current {
    taken: EventCursor,
    epoch: u64,
  },
  /// Invalidated in flight, or of an earlier epoch: read again.
  Stale,
}

/// What changed since the consumer last looked: a reset (a new epoch, so every earlier read is stale), and the scopes
/// invalidated since, each at its latest cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes {
  epoch: u64,
  reset: bool,
  scopes: BTreeMap<QueryScope, EventCursor>,
}

impl Changes {
  /// Whether every read made before is stale: the subscription was reset or connected again.
  pub const fn is_reset(&self) -> bool {
    self.reset
  }

  /// The scopes invalidated, each at its latest cursor.
  pub const fn scopes(&self) -> &BTreeMap<QueryScope, EventCursor> {
    &self.scopes
  }

  /// Whether a read of `scope` taken at `taken` in `epoch` is stale after these changes.
  pub(super) fn stales(&self, scope: &QueryScope, taken: EventCursor, epoch: u64) -> bool {
    epoch != self.epoch || self.scopes.get(scope).is_some_and(|cursor| *cursor > taken)
  }
}

impl Live {
  pub(super) const fn ended(&self) -> Option<&TeamReadError> {
    self.ended.as_ref()
  }

  pub(super) const fn seen(&self) -> Option<EventCursor> {
    self.seen
  }

  /// Ends the project's reads with `error`, unless they ended already; returns how they ended.
  pub(super) fn end(&mut self, error: TeamReadError) -> TeamReadError {
    self.ended.get_or_insert(error).clone()
  }

  /// The subscription's `ready` at `cursor`, sent only to a subscription opened without a cursor: it sets the
  /// position.
  pub(super) fn ready(&mut self, cursor: EventCursor) {
    self.unready = None;
    self.seen = Some(cursor);
  }

  /// A subscription opened. Once there is a position it is a reconnect, resumed from that position with no `ready`,
  /// after which no earlier read is trusted: a new epoch (`true`).
  pub(super) fn opened(&mut self) -> bool {
    let Some(seen) = self.seen else {
      return false;
    };
    self.reset(seen);
    true
  }

  /// A subscription attempt failed with `error`: until the first `ready`, reads fail with it instead of waiting.
  pub(super) fn failed(&mut self, error: TeamReadError) {
    if self.seen.is_none() {
      self.unready = Some(error);
    }
  }

  /// `reset` at `cursor`: a new epoch from `cursor` on, and the invalidations pending before it dropped.
  pub(super) fn reset(&mut self, cursor: EventCursor) {
    self.epoch += 1;
    self.seen = Some(cursor);
    self.pending.clear();
  }

  /// One `invalidated` batch; a batch at or below the position is a replay already counted (`false`).
  pub(super) fn invalidated(&mut self, cursor: EventCursor, scopes: &[QueryScope]) -> bool {
    if self.seen.is_some_and(|seen| cursor <= seen) {
      return false;
    }
    self.seen = Some(cursor);
    for scope in scopes {
      let latest = self.pending.entry(*scope).or_insert(cursor);
      *latest = (*latest).max(cursor);
    }
    for read in self.in_flight.values_mut() {
      if scopes.contains(&read.scope) {
        read.hit = Some(cursor);
      }
    }
    true
  }

  /// A read of `scope` starts: `None` while the subscription has no position yet and is still trying.
  pub(super) fn begin(&mut self, scope: QueryScope) -> Result<Option<Ticket>, TeamReadError> {
    if let Some(ended) = &self.ended {
      return Err(ended.clone());
    }
    let Some(floor) = self.seen else {
      return self.unready.clone().map_or(Ok(None), Err);
    };
    let id = self.next_read;
    self.next_read += 1;
    self.in_flight.insert(id, InFlight { scope, hit: None });
    Ok(Some(Ticket {
      id,
      epoch: self.epoch,
      floor,
    }))
  }

  /// The read of `ticket` answered, at its own snapshot `cursor` when it has one.
  pub(super) fn finish(&mut self, ticket: &Ticket, cursor: Option<EventCursor>) -> Result<Finish, TeamReadError> {
    let read = self.in_flight.remove(&ticket.id);
    if let Some(ended) = &self.ended {
      return Err(ended.clone());
    }
    let taken = cursor.map_or(ticket.floor, |cursor| cursor.max(ticket.floor));
    let invalidated = read.and_then(|read| read.hit).is_some_and(|hit| hit > taken);
    if ticket.epoch != self.epoch || invalidated {
      return Ok(Finish::Stale);
    }
    Ok(Finish::Current {
      taken,
      epoch: self.epoch,
    })
  }

  /// What changed since the last call: `None` when nothing did; the end once the reads ended.
  pub(super) fn take_changes(&mut self) -> Option<Result<Changes, TeamReadError>> {
    if let Some(ended) = &self.ended {
      return Some(Err(ended.clone()));
    }
    let reset = self.delivered != self.epoch;
    if !reset && self.pending.is_empty() {
      return None;
    }
    self.delivered = self.epoch;
    Some(Ok(Changes {
      epoch: self.epoch,
      reset,
      scopes: std::mem::take(&mut self.pending),
    }))
  }
}

/// The snapshot cursor a read answers at, when its model carries one.
pub(super) trait Taken {
  fn cursor(&self) -> Option<EventCursor>;
}

impl<T> Taken for Page<T> {
  fn cursor(&self) -> Option<EventCursor> {
    Some(self.cursor)
  }
}

impl Taken for TaskBoardCounts {
  fn cursor(&self) -> Option<EventCursor> {
    Some(self.cursor)
  }
}

impl Taken for ModuleBurndownRead {
  fn cursor(&self) -> Option<EventCursor> {
    Some(self.cursor)
  }
}

impl Taken for WorkspaceMetadata {
  fn cursor(&self) -> Option<EventCursor> {
    Some(self.cursor)
  }
}

/// Models without a cursor of their own: their read holds at least what the position it was issued at holds.
macro_rules! issued_at_position {
  ($($model:ty),*) => {
    $(impl Taken for $model {
      fn cursor(&self) -> Option<EventCursor> {
        None
      }
    })*
  };
}

issued_at_position!(TaskRead, ModuleSummary, ModuleProgressRead, Capabilities);
