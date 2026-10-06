//! What a test scripts: the turns the fake server plays, in order.

use std::time::Duration;

use serde_json::Value;

use super::Recorded;

/// A piece of a streamed answer.
#[derive(Clone, Debug)]
pub enum Part {
  /// One SSE event, written as `event: <type>` and `data: <json>`.
  Event(Value),
  /// Bytes as they are, for comments and odd framing.
  Raw(String),
  Pause(Duration),
  /// The connection is closed here, mid-answer.
  Drop,
}

#[derive(Clone, Debug)]
pub(super) enum Reply {
  Stream(Vec<Part>),
  Status {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
  },
  /// An error answer in the server's wire format.
  Error {
    status: u16,
    kind: String,
    message: String,
  },
}

/// Which requests a turn answers.
#[derive(Clone, Copy, Debug)]
pub(super) enum Route {
  /// Model requests, at the wire format's path.
  Model,
  /// Requests whose path starts with this.
  Path(&'static str),
}

type Expectation = Box<dyn Fn(&Recorded) + Send>;

pub(super) struct Turn {
  pub route: Route,
  pub expect: Option<Expectation>,
  pub reply: Reply,
  pub history_may_change: bool,
}

/// The turns a test expects, in order.
#[derive(Default)]
pub struct Script {
  pub(super) turns: Vec<Turn>,
  expect: Option<Expectation>,
  history_may_change: bool,
}

impl Script {
  pub fn new() -> Self {
    Self::default()
  }

  /// Checks the next turn's request.
  pub fn expect(mut self, check: impl Fn(&Recorded) + Send + 'static) -> Self {
    self.expect = Some(Box::new(check));
    self
  }

  /// The next turn's request may start a new history (a compaction, the thinking recovery).
  pub const fn history_may_change(mut self) -> Self {
    self.history_may_change = true;
    self
  }

  /// A model turn answered with a stream.
  pub fn reply_sse(self, parts: Vec<Part>) -> Self {
    self.push(Route::Model, Reply::Stream(parts))
  }

  /// A turn on `path` answered with `status`, `headers` and `body`.
  pub fn reply_status(self, path: &'static str, status: u16, headers: &[(&str, &str)], body: &str) -> Self {
    let headers = headers
      .iter()
      .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
      .collect();
    let reply = Reply::Status {
      status,
      headers,
      body: body.to_owned(),
    };
    self.push(Route::Path(path), reply)
  }

  /// A model turn answered with an error of `status` and the provider's error body.
  pub fn reply_error(self, status: u16, kind: &str, message: &str) -> Self {
    let reply = Reply::Error {
      status,
      kind: kind.to_owned(),
      message: message.to_owned(),
    };
    self.push(Route::Model, reply)
  }

  fn push(mut self, route: Route, reply: Reply) -> Self {
    self.turns.push(Turn {
      route,
      expect: self.expect.take(),
      reply,
      history_may_change: std::mem::take(&mut self.history_may_change),
    });
    self
  }
}
