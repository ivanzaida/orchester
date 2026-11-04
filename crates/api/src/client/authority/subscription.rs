//! A team project's scoped invalidation subscription (S11): `GET …/subscription`, resumed after a cursor with
//! `Last-Event-ID` and read as typed [`SubscriptionEvent`]s until it ends, always with a reason. Each SSE event is
//! named by its `kind` and its data is the event's JSON with that same `kind`. Keep-alive comments are skipped.
//! Reconnecting and its backoff are the caller's: the stream only says why it ended.

use std::collections::VecDeque;

use reqwest::{Method, Response, header::ACCEPT};

use super::{Authority, AuthorityCallError, answered};
use crate::{
  client::{
    ClientError,
    events::{EVENT_STREAM, LAST_EVENT_ID, STREAM_DEADLINE, is_event_stream},
  },
  sse::{SseEvent, SseParser},
  v2::{AuthorityErrorCode, EventCursor, SubscriptionEvent},
};

/// Why a subscription ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubscriptionEnd {
  /// The server closed the stream; the caller subscribes again.
  Closed,
  /// `401`: the server no longer accepts this computer's token.
  Unauthorized,
  /// The `revoked` event, or a `403` refusal to open: the caller lost access to the workspace.
  Revoked,
  /// `404`: the server has no such workspace for the caller.
  NotFound,
  /// The connection failed, broke off or stayed silent too long, or the server refused the stream otherwise (e.g.
  /// `429 rate_limited`, `503 unavailable`).
  Failed(AuthorityCallError),
  /// An event the contract does not read: no or an unknown kind, a name that is not its data's kind, data that is not
  /// its payload, or one longer than [`crate::sse::MAX_EVENT`].
  Unreadable,
}

/// An open subscription.
#[derive(Debug)]
pub struct Subscription {
  response: Response,
  parser: SseParser,
  pending: VecDeque<SseEvent>,
  ended: Option<SubscriptionEnd>,
}

impl Authority<'_> {
  /// `GET …/subscription`: `ready` with the stream's cursor first, then an `invalidated` event per committed batch
  /// after `last` (sent as `Last-Event-ID`), or `reset` when `last` is older than the server replays.
  pub async fn subscribe(&self, last: Option<EventCursor>) -> Result<Subscription, SubscriptionEnd> {
    let mut request = self
      .request(Method::GET, "/subscription")
      .map_err(|error| SubscriptionEnd::Failed(error.into()))?
      .header(ACCEPT, EVENT_STREAM)
      .timeout(STREAM_DEADLINE);
    if let Some(last) = last {
      request = request.header(LAST_EVENT_ID, last.to_string());
    }
    let response = answered(request).await.map_err(refused)?;
    if !is_event_stream(&response) {
      let status = response.status().as_u16();
      return Err(SubscriptionEnd::Failed(ClientError::NotOrchester { status }.into()));
    }
    Ok(Subscription {
      response,
      parser: SseParser::new(),
      pending: VecDeque::new(),
      ended: None,
    })
  }
}

impl Subscription {
  /// The next event, or why the stream ended; once it ended, the same reason again. `revoked` ends it.
  pub async fn next(&mut self) -> Result<SubscriptionEvent, SubscriptionEnd> {
    if let Some(end) = &self.ended {
      return Err(end.clone());
    }
    let next = self.read().await;
    if let Err(end) = &next {
      self.ended = Some(end.clone());
    }
    next
  }

  async fn read(&mut self) -> Result<SubscriptionEvent, SubscriptionEnd> {
    loop {
      if let Some(event) = self.pending.pop_front() {
        return typed(&event);
      }
      let chunk = self
        .response
        .chunk()
        .await
        .map_err(|error| SubscriptionEnd::Failed(ClientError::transport(&error).into()))?;
      // An event cut off by the end of the stream is no event, as the SSE standard says.
      let Some(chunk) = chunk else {
        return Err(SubscriptionEnd::Closed);
      };
      let events = self.parser.feed(&chunk).map_err(|_| SubscriptionEnd::Unreadable)?;
      self.pending.extend(events);
    }
  }
}

/// The SSE event as the contract's [`SubscriptionEvent`]: its data, whose `kind` is the event's name.
fn typed(event: &SseEvent) -> Result<SubscriptionEvent, SubscriptionEnd> {
  let name = event.name.as_deref().ok_or(SubscriptionEnd::Unreadable)?;
  let data: serde_json::Value = serde_json::from_str(&event.data).map_err(|_| SubscriptionEnd::Unreadable)?;
  if data.get("kind").and_then(serde_json::Value::as_str) != Some(name) {
    return Err(SubscriptionEnd::Unreadable);
  }
  match serde_json::from_value(data).map_err(|_| SubscriptionEnd::Unreadable)? {
    SubscriptionEvent::Revoked {} => Err(SubscriptionEnd::Revoked),
    event => Ok(event),
  }
}

/// The end of a subscription the server refused to open.
fn refused(error: AuthorityCallError) -> SubscriptionEnd {
  match error.code() {
    Some(AuthorityErrorCode::Unauthenticated) => SubscriptionEnd::Unauthorized,
    Some(AuthorityErrorCode::Forbidden | AuthorityErrorCode::MembershipRevoked) => SubscriptionEnd::Revoked,
    Some(AuthorityErrorCode::NotFound) => SubscriptionEnd::NotFound,
    _ => SubscriptionEnd::Failed(error),
  }
}
