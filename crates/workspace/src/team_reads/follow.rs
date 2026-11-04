//! Following a team project's invalidation subscription (S11): subscribed from the last cursor seen, read until it
//! ends, and subscribed again after the connection monitor's backoff ([`Schedule`]); a subscription that opened starts
//! the waits over. Every `ready` after the first is a reconnect and, like `reset`, starts a new epoch. `401`, `403`,
//! `revoked` and `404` end the reads for good, and so does a changed session, which is checked while the stream is
//! open and between connections.

use orchester_api::{
  client::SubscriptionEnd,
  v2::{EventCursor, SubscriptionEvent},
};

use super::{TeamProject, TeamReadError};
use crate::sharing::{Schedule, sync::Clock};

/// How one subscription ended.
enum Ending {
  Stream(SubscriptionEnd),
  /// The session changed, or a read ended the project's reads.
  Ended(TeamReadError),
}

impl TeamProject {
  /// Follows the project's subscription until the reads end; returns why. Reads wait for its first `ready`, or fail
  /// with why the subscription cannot open while it has none.
  pub async fn follow(&self, clock: &impl Clock) -> TeamReadError {
    let mut failures = 0;
    loop {
      let (opened, ending) = self.connect().await;
      let end = match ending {
        Ending::Ended(error) => error,
        Ending::Stream(
          end @ (SubscriptionEnd::Unauthorized | SubscriptionEnd::Revoked | SubscriptionEnd::NotFound),
        ) => end.into(),
        Ending::Stream(end) => {
          tracing::debug!(?end, "a team project's subscription ended and connects again");
          self.inner.live.lock().failed(end.into());
          self.tick();
          failures = if opened { 1 } else { failures + 1 };
          if let Some(ended) = self.wait(clock, failures).await {
            return ended;
          }
          continue;
        }
      };
      return self.end(end);
    }
  }

  /// One subscription from the last cursor seen, read until it ends: whether it opened, and how it ended.
  async fn connect(&self) -> (bool, Ending) {
    if let Some(ended) = self.ended() {
      return (false, Ending::Ended(ended));
    }
    let last = self.inner.live.lock().seen();
    let mut subscription = match self.inner.session.authority().subscribe(last).await {
      Ok(subscription) => subscription,
      Err(end) => return (false, Ending::Stream(end)),
    };
    let mut generation = self.inner.session.generation.clone();
    loop {
      let event = tokio::select! {
        event = subscription.next() => event,
        _ = generation.changed() => Err(SubscriptionEnd::Closed),
      };
      if !self.inner.session.current() {
        return (true, Ending::Ended(TeamReadError::SessionChanged));
      }
      if let Some(ended) = self.ended() {
        return (true, Ending::Ended(ended));
      }
      match event {
        Ok(event) => self.apply(event),
        Err(end) => return (true, Ending::Stream(end)),
      }
    }
  }

  /// Takes one event of the subscription. The client reports `revoked` as the stream's end; it ends the reads here too.
  fn apply(&self, event: SubscriptionEvent) {
    let changed = {
      let mut live = self.inner.live.lock();
      match event {
        SubscriptionEvent::Ready { cursor } => {
          live.ready(cursor);
          true
        }
        SubscriptionEvent::Invalidated { cursor, scopes } => live.invalidated(cursor, &scopes),
        SubscriptionEvent::Reset { cursor } => {
          live.reset(cursor);
          true
        }
        SubscriptionEvent::Revoked {} => {
          live.end(TeamReadError::MembershipRevoked);
          true
        }
      }
    };
    if changed {
      self.tick();
    }
  }

  /// Waits out the backoff after the `failures`-th failure in a row; the end when the session changes meanwhile.
  async fn wait(&self, clock: &impl Clock, failures: u32) -> Option<TeamReadError> {
    let mut generation = self.inner.session.generation.clone();
    tokio::select! {
      () = clock.wait(Schedule::DEFAULT.backoff(failures, clock.jitter())) => {}
      _ = generation.changed() => {}
    }
    if !self.inner.session.current() {
      return Some(self.end(TeamReadError::SessionChanged));
    }
    self.ended()
  }

  fn ended(&self) -> Option<TeamReadError> {
    self.inner.live.lock().ended().cloned()
  }

  /// The subscription's latest cursor; `None` before its first `ready`.
  pub fn cursor(&self) -> Option<EventCursor> {
    self.inner.live.lock().seen()
  }
}
