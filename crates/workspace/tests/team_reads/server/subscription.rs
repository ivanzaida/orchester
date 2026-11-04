//! The fake's `GET …/subscription`, as the team server streams it: a `: connected` comment; without `Last-Event-ID`,
//! `ready` at the current cursor; with it, no `ready`, but `reset` at the current cursor when the test asked for one,
//! or else the batches after it. Then each new batch as it commits and a keep-alive while none does, until the
//! streams are dropped, the member is revoked (`revoked`, then the stream closes) or the client hangs up.

use std::{io::Write, net::TcpStream, time::Duration};

use serde_json::{Value, json};

use super::{Shared, State};
use crate::http::{Request, sse};

const KEEP_ALIVE: Duration = Duration::from_millis(100);

pub fn stream(shared: &Shared, request: &Request, token: &str, stream: &mut TcpStream) {
  let last = request.header("last-event-id").map(str::to_owned);
  let mut state = shared.state.lock().expect("state");
  state.subscribed.push(last.clone());
  let generation = state.streams;
  let mut cursor = state.cursor();
  let mut text = ": connected\n\n".to_owned();
  match last {
    None => text.push_str(&sse(&json!({"kind": "ready", "cursor": cursor.to_string()}))),
    Some(_) if std::mem::take(&mut state.reset_next) => {
      text.push_str(&sse(&json!({"kind": "reset", "cursor": cursor.to_string()})));
    }
    Some(last) => text.push_str(&batches_after(&state, last.parse().expect("a cursor"))),
  }
  drop(state);
  shared.changed.notify_all();
  let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
  if stream.write_all(head.as_bytes()).is_err() {
    return;
  }
  loop {
    if stream.write_all(text.as_bytes()).is_err() || stream.flush().is_err() {
      return;
    }
    let state = shared.state.lock().expect("state");
    let (state, _) = shared
      .changed
      .wait_timeout_while(state, KEEP_ALIVE, |state| {
        state.cursor() <= cursor && state.streams == generation && !revoked(state, token)
      })
      .expect("state");
    if state.streams != generation {
      return;
    }
    if revoked(&state, token) {
      drop(state);
      // The stream closes after it whether the client read it or not.
      let _ = stream.write_all(sse(&json!({"kind": "revoked"})).as_bytes());
      return;
    }
    text = batches_after(&state, cursor);
    cursor = state.cursor();
    if text.is_empty() {
      text = ": keep-alive\n\n".to_owned();
    }
  }
}

fn revoked(state: &State, token: &str) -> bool {
  state.revoked.iter().any(|revoked| revoked == token)
}

/// The `invalidated` events of the batches after `cursor`.
fn batches_after(state: &State, cursor: u64) -> String {
  state
    .batches
    .iter()
    .filter(|(batch, _)| *batch > cursor)
    .map(|(batch, scopes)| {
      let event: Value = json!({"kind": "invalidated", "cursor": batch.to_string(), "scopes": scopes});
      sse(&event)
    })
    .collect()
}
