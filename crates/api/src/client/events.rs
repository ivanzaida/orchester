//! A shared project's event stream (`docs/architecture/server.md`, "Live updates", "Events"): `GET …/events`, resumed
//! from a cursor with `Last-Event-ID` and read as typed [`Event`]s until it ends, always with a reason. Keep-alive
//! comments are skipped. Reconnecting and its backoff are the caller's: the stream only says why it ended.

use std::{collections::VecDeque, time::Duration};

use orchester_core::{id::ProjectId, secret::SecretValue};
use reqwest::{
  Method, Response,
  header::{ACCEPT, CONTENT_TYPE},
};

use super::{Client, ClientError, answered, bearer};
use crate::{
  Event,
  sse::{SseEvent, SseParser},
};

/// How long a stream may stay silent: the server sends a keep-alive every 20 s, so two missed ones and some slack.
pub(super) const STREAM_IDLE: Duration = Duration::from_secs(45);
/// A stream has no deadline as a whole (tokio takes the longest duration as its far future); [`STREAM_IDLE`] ends one
/// on which nothing arrives.
const STREAM_DEADLINE: Duration = Duration::MAX;
const LAST_EVENT_ID: &str = "Last-Event-ID";
const EVENT_STREAM: &str = "text/event-stream";

/// Why a project's event stream ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEnd {
  /// The server closed the stream; the caller connects again from its cursor.
  Closed,
  /// `401`: the server signed this computer out.
  Unauthorized,
  /// `403`: the person is no longer a member of the project.
  Forbidden,
  /// The `revoked` event: the session or the person's membership ended while connected.
  Revoked,
  /// The connection failed, broke off or stayed silent past [`STREAM_IDLE`], or the server refused the stream
  /// otherwise (e.g. `429 rate_limited`).
  Failed(ClientError),
  /// An event the contract does not read: no or an unknown kind, data that is not its payload, or one longer than
  /// [`crate::sse::MAX_EVENT`].
  Unreadable,
}

/// An open event stream.
#[derive(Debug)]
pub struct EventStream {
  response: Response,
  parser: SseParser,
  pending: VecDeque<SseEvent>,
  ended: Option<StreamEnd>,
}

impl Client {
  /// `GET /projects/{id}/events`: the project's events after `cursor`, sent as `Last-Event-ID`. Without a cursor, or
  /// with one older than the server keeps, the snapshot comes first.
  pub async fn events(
    &self,
    token: &SecretValue,
    project: ProjectId,
    cursor: Option<u64>,
  ) -> Result<EventStream, StreamEnd> {
    let path = format!("/projects/{project}/events");
    let mut request = self
      .request(Method::GET, &path)
      .map_err(StreamEnd::Failed)?
      .header(ACCEPT, EVENT_STREAM)
      .timeout(STREAM_DEADLINE);
    if let Some(cursor) = cursor {
      request = request.header(LAST_EVENT_ID, cursor.to_string());
    }
    let response = answered(bearer(request, token)).await.map_err(refused)?;
    if !is_event_stream(&response) {
      let status = response.status().as_u16();
      return Err(StreamEnd::Failed(ClientError::NotOrchester { status }));
    }
    Ok(EventStream {
      response,
      parser: SseParser::new(),
      pending: VecDeque::new(),
      ended: None,
    })
  }
}

impl EventStream {
  /// The next event, or why the stream ended; once it ended, the same reason again.
  pub async fn next(&mut self) -> Result<Event, StreamEnd> {
    if let Some(end) = &self.ended {
      return Err(end.clone());
    }
    let next = self.read().await;
    if let Err(end) = &next {
      self.ended = Some(end.clone());
    }
    next
  }

  async fn read(&mut self) -> Result<Event, StreamEnd> {
    loop {
      if let Some(event) = self.pending.pop_front() {
        return typed(event);
      }
      let chunk = self
        .response
        .chunk()
        .await
        .map_err(|error| StreamEnd::Failed(ClientError::transport(&error)))?;
      // An event cut off by the end of the stream is no event, as the SSE standard says.
      let Some(chunk) = chunk else {
        return Err(StreamEnd::Closed);
      };
      let events = self.parser.feed(&chunk).map_err(|_| StreamEnd::Unreadable)?;
      self.pending.extend(events);
    }
  }
}

/// The SSE event as the contract's [`Event`]: its name is the kind, its data the kind's payload.
fn typed(event: SseEvent) -> Result<Event, StreamEnd> {
  let kind = event.name.ok_or(StreamEnd::Unreadable)?;
  let data: serde_json::Value = serde_json::from_str(&event.data).map_err(|_| StreamEnd::Unreadable)?;
  let event =
    serde_json::from_value(serde_json::json!({ "kind": kind, "data": data })).map_err(|_| StreamEnd::Unreadable)?;
  match event {
    Event::Revoked(_) => Err(StreamEnd::Revoked),
    event => Ok(event),
  }
}

/// The end of a stream the server refused to open.
fn refused(error: ClientError) -> StreamEnd {
  match error {
    ClientError::Api { status: 401, .. } => StreamEnd::Unauthorized,
    ClientError::Api { status: 403, .. } => StreamEnd::Forbidden,
    error => StreamEnd::Failed(error),
  }
}

fn is_event_stream(response: &Response) -> bool {
  response
    .headers()
    .get(CONTENT_TYPE)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(';').next())
    .is_some_and(|media| media.trim().eq_ignore_ascii_case(EVENT_STREAM))
}
