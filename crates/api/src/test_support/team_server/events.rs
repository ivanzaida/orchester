//! The stand-in's `GET /api/v1/projects/{id}/events`, once a test opened the streams: a project's event stream that
//! opens and keeps alive, with no events, until the server stops answering or the client hangs up. A desktop reads the
//! team's configuration from the resource routes each time its stream opens (`docs/design/team-configuration-live.md`),
//! so an open stream is what lets it read what the stand-in holds; the stand-in's own changes are not sent as events.

use std::{io::Write, net::TcpStream, thread, time::Instant};

use parking_lot::Mutex;

use super::{
  State, TOKEN,
  authority::{KEEP_ALIVE, STREAM_POLL},
};

/// Whether the request `line` asks for the event stream of a project the stand-in shares, once its streams open.
pub(super) fn asked(line: &str, state: &Mutex<State>) -> bool {
  let Some(project) = line
    .strip_prefix("GET /api/v1/projects/")
    .and_then(|rest| rest.split_whitespace().next())
    .and_then(|path| path.split('?').next())
    .and_then(|path| path.strip_suffix("/events"))
  else {
    return false;
  };
  let state = state.lock();
  state.event_streams && state.projects.iter().any(|known| known["id"] == project)
}

/// Answers the stream: `401` without the stand-in's token, otherwise an open stream with a keep-alive each second.
pub(super) fn serve(stream: &mut TcpStream, headers: &str, state: &Mutex<State>) {
  if !headers.contains(&format!("authorization: bearer {TOKEN}")) {
    let refused = "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 24\r\n\
                   Connection: close\r\n\r\n{\"error\":\"unauthorized\"}";
    // The client reads the refusal or fails on its own; a broken connection needs nothing here.
    let _ = stream.write_all(refused.as_bytes());
    return;
  }
  let head =
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
  if stream.write_all(head.as_bytes()).is_err() || stream.flush().is_err() {
    return;
  }
  let mut quiet_since = Instant::now();
  loop {
    thread::sleep(STREAM_POLL);
    if state.lock().down {
      return;
    }
    if quiet_since.elapsed() >= KEEP_ALIVE {
      if stream.write_all(b": keep-alive\n\n").is_err() || stream.flush().is_err() {
        return;
      }
      quiet_since = Instant::now();
    }
  }
}
