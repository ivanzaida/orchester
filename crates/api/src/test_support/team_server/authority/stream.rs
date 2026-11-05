//! The stand-in's S11 subscription: `ready`, each committed batch as `invalidated`, and keep-alives.

use std::{io::Write, net::TcpStream, thread, time::Instant};

use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{KEEP_ALIVE, STREAM_POLL};
use crate::test_support::team_server::State;

/// `GET …/subscription`: `ready` at the current cursor and the batches after `Last-Event-ID`, then each batch as it
/// commits and a keep-alive while none does, until the streams end, the server stops answering or the client hangs up.
pub(super) fn subscribe(stream: &mut TcpStream, headers: &str, state: &Mutex<State>) {
  let last = headers
    .lines()
    .find_map(|line| line.strip_prefix("last-event-id:"))
    .and_then(|value| value.trim().parse::<u64>().ok());
  let (generation, mut cursor) = {
    let state = state.lock();
    (state.authority.streams, state.authority.cursor())
  };
  let mut text = sse(&json!({"kind": "ready", "cursor": cursor.to_string()}));
  if let Some(last) = last {
    text.push_str(&batches_after(&state.lock(), last));
  }
  let head =
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
  if stream.write_all(head.as_bytes()).is_err() {
    return;
  }
  let mut quiet_since = Instant::now();
  loop {
    if !text.is_empty() {
      if stream.write_all(text.as_bytes()).is_err() || stream.flush().is_err() {
        return;
      }
      quiet_since = Instant::now();
    }
    thread::sleep(STREAM_POLL);
    let state = state.lock();
    if state.down || state.authority.streams != generation || state.authority.unavailable {
      return;
    }
    text = batches_after(&state, cursor);
    cursor = state.authority.cursor();
    if text.is_empty() && quiet_since.elapsed() >= KEEP_ALIVE {
      text = ": keep-alive\n\n".to_owned();
    }
  }
}

/// The `invalidated` events of the batches after `cursor`.
fn batches_after(state: &State, cursor: u64) -> String {
  state
    .authority
    .batches
    .iter()
    .filter(|(batch, _)| *batch > cursor)
    .map(|(batch, scopes)| sse(&json!({"kind": "invalidated", "cursor": batch.to_string(), "scopes": scopes})))
    .collect()
}

/// `event` as one SSE event: its kind as the name, its cursor as the id, itself as the data.
fn sse(event: &Value) -> String {
  let id = event
    .get("cursor")
    .and_then(Value::as_str)
    .map(|cursor| format!("id: {cursor}\n"))
    .unwrap_or_default();
  let kind = event["kind"].as_str().unwrap_or_default();
  format!("{id}event: {kind}\ndata: {event}\n\n")
}
