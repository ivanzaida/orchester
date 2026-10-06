//! The fake model server (`docs/architecture/native-agent.md` §8, layer 2): a tokio listener on
//! `127.0.0.1:0` that answers scripted turns over plain HTTP/1.1, one request per connection, in either
//! wire format ([`Wire`]). The `anthropic_wire` and `responses_wire` tests share it.
//!
//! It checks what the design requires of every conversation: each model request's frozen fields
//! (`system` and `tools`, or `instructions` and `tools`) are the same JSON text as the previous
//! request's, and the previous request's history (`messages`, or `input`) is a prefix of its own, entry
//! by entry (append-only history), unless the turn says the history may change. It echoes the cache: a
//! cache read of `"echo"` in a scripted event (`message.usage.cache_read_input_tokens`, or
//! `response.usage.input_tokens_details.cached_tokens`) becomes the size of the prefix shared with the
//! previous request. `GET /v1/models` and `GET /v1/models/{id}` are answered from a model catalog unless
//! a scripted turn is waiting for that path.

mod script;
mod wire;

use std::{
  collections::VecDeque,
  net::SocketAddr,
  panic::{self, AssertUnwindSafe},
  sync::Arc,
};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  task::JoinHandle,
};

use self::script::{Reply, Route, Turn};
pub use self::{
  script::{Part, Script},
  wire::Wire,
};

/// One request as the server got it.
#[derive(Clone, Debug)]
pub struct Recorded {
  pub method: String,
  pub path: String,
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl Recorded {
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  pub fn json(&self) -> Value {
    serde_json::from_slice(&self.body).expect("the request body is JSON")
  }
}

struct State {
  wire: Wire,
  turns: VecDeque<Turn>,
  models: Vec<Value>,
  page_size: usize,
  requests: Vec<Recorded>,
  violations: Vec<String>,
  last_request: Option<Value>,
}

/// A running fake server.
pub struct FakeServer {
  address: SocketAddr,
  state: Arc<Mutex<State>>,
  task: JoinHandle<()>,
}

impl FakeServer {
  /// An Anthropic Messages server playing `script`, with `models` as its catalog, listed `page_size`
  /// per page.
  pub async fn start(script: Script, models: Vec<Value>, page_size: usize) -> Self {
    Self::start_wire(Wire::Messages, script, models, page_size).await
  }

  /// A server of `wire` playing `script`, with `models` as its catalog, listed `page_size` per page.
  pub async fn start_wire(wire: Wire, script: Script, models: Vec<Value>, page_size: usize) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind the fake server");
    let address = listener.local_addr().expect("the fake server's address");
    let state = Arc::new(Mutex::new(State {
      wire,
      turns: script.turns.into(),
      models,
      page_size,
      requests: Vec::new(),
      violations: Vec::new(),
      last_request: None,
    }));
    let shared = Arc::clone(&state);
    let task = tokio::spawn(async move {
      while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(serve(stream, Arc::clone(&shared)));
      }
    });
    Self { address, state, task }
  }

  pub fn base_url(&self) -> String {
    format!("http://{}", self.address)
  }

  pub fn requests(&self) -> Vec<Recorded> {
    self.state.lock().requests.clone()
  }

  /// The model requests so far.
  pub fn model_requests(&self) -> Vec<Recorded> {
    let path = self.state.lock().wire.path();
    self
      .requests()
      .into_iter()
      .filter(|request| request.path == path)
      .collect()
  }

  /// Asserts that every scripted turn was played and that no request broke the rules.
  pub fn finish(&self) {
    let state = self.state.lock();
    assert!(state.violations.is_empty(), "violations: {:#?}", state.violations);
    assert!(
      state.turns.is_empty(),
      "{} scripted turns were not played",
      state.turns.len()
    );
  }
}

impl Drop for FakeServer {
  fn drop(&mut self) {
    self.task.abort();
  }
}

async fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>) {
  let Some(request) = read_request(&mut stream).await else {
    return;
  };
  let reply = answer(&request, &state);
  // A client that hung up early is the client's business; the test sees what it got.
  match reply {
    Reply::Status { status, headers, body } => {
      let mut head = format!(
        "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
      );
      for (key, value) in headers {
        head.push_str(&format!("{key}: {value}\r\n"));
      }
      head.push_str("\r\n");
      stream.write_all(head.as_bytes()).await.ok();
      stream.write_all(body.as_bytes()).await.ok();
    }
    Reply::Stream(parts) => stream_parts(&mut stream, parts).await,
    Reply::Error { .. } => unreachable!("`answer` writes an error as a status"),
  }
  stream.shutdown().await.ok();
}

async fn stream_parts(stream: &mut TcpStream, parts: Vec<Part>) {
  let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
  if stream.write_all(head.as_bytes()).await.is_err() {
    return;
  }
  for part in parts {
    let bytes = match part {
      Part::Event(event) => {
        let kind = event["type"].as_str().unwrap_or("message").to_owned();
        format!("event: {kind}\ndata: {event}\n\n")
      }
      Part::Raw(text) => text,
      Part::Pause(duration) => {
        tokio::time::sleep(duration).await;
        continue;
      }
      Part::Drop => return,
    };
    if stream.write_all(bytes.as_bytes()).await.is_err() {
      return;
    }
    stream.flush().await.ok();
  }
}

/// The reply to `request`: the next scripted turn for its path, the model catalog, or a 500 recorded as
/// a violation. An error reply is written out in the server's wire format.
fn answer(request: &Recorded, state: &Mutex<State>) -> Reply {
  let mut state = state.lock();
  match reply(request, &mut state) {
    Reply::Error { status, kind, message } => Reply::Status {
      status,
      headers: Vec::new(),
      body: state.wire.error_body(&kind, &message),
    },
    other => other,
  }
}

fn reply(request: &Recorded, state: &mut State) -> Reply {
  state.requests.push(request.clone());
  let model_path = state.wire.path();
  let scripted = state.turns.front().is_some_and(|turn| match turn.route {
    Route::Model => request.path == model_path,
    Route::Path(path) => request.path.starts_with(path),
  });
  if scripted {
    let turn = state.turns.pop_front().expect("a turn is waiting");
    if let Some(expect) = &turn.expect
      && let Err(panic) = panic::catch_unwind(AssertUnwindSafe(|| expect(request)))
    {
      let message = panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
        .unwrap_or_default();
      state.violations.push(format!("expectation failed: {message}"));
    }
    if request.path == model_path {
      check_history(state, request, turn.history_may_change);
    }
    return echo_cache(turn.reply, state);
  }
  if request.method == "GET" && request.path.starts_with("/v1/models") {
    return catalog(state, &request.path);
  }
  state
    .violations
    .push(format!("unexpected request {} {}", request.method, request.path));
  Reply::Status {
    status: 500,
    headers: Vec::new(),
    body: "{}".to_owned(),
  }
}

/// Records a violation when `request` does not continue the previous model request.
fn check_history(state: &mut State, request: &Recorded, may_change: bool) {
  let body = request.json();
  let wire = state.wire;
  if let (Some(previous), false) = (&state.last_request, may_change) {
    for field in wire.frozen() {
      if text(&previous[field]) != text(&body[field]) {
        state
          .violations
          .push(format!("`{field}` changed within the conversation"));
      }
    }
    let before = previous[wire.history()].as_array().cloned().unwrap_or_default();
    let now = body[wire.history()].as_array().cloned().unwrap_or_default();
    let prefix = now.len() >= before.len() && before.iter().zip(&now).all(|(a, b)| text(a) == text(b));
    if !prefix {
      state
        .violations
        .push("the history was edited, not appended to".to_owned());
    }
  }
  state.last_request = Some(body);
}

/// `value` as JSON text.
fn text(value: &Value) -> String {
  value.to_string()
}

/// `reply` with an echoed cache read: the bytes the request shares with the previous one, as tokens.
fn echo_cache(reply: Reply, state: &State) -> Reply {
  let Reply::Stream(parts) = reply else {
    return reply;
  };
  let cached = shared_prefix_tokens(state);
  let pointer = state.wire.cache_read();
  let parts = parts
    .into_iter()
    .map(|mut part| {
      if let Part::Event(event) = &mut part
        && let Some(read) = event.pointer_mut(pointer)
        && *read == json!("echo")
      {
        *read = json!(cached);
      }
      part
    })
    .collect();
  Reply::Stream(parts)
}

fn shared_prefix_tokens(state: &State) -> u64 {
  let path = state.wire.path();
  let requests: Vec<&Recorded> = state.requests.iter().filter(|request| request.path == path).collect();
  let [.., previous, current] = requests.as_slice() else {
    return 0;
  };
  let shared = previous
    .body
    .iter()
    .zip(&current.body)
    .take_while(|(a, b)| a == b)
    .count();
  u64::try_from(shared / 4).expect("a request fits u64")
}

fn catalog(state: &State, path: &str) -> Reply {
  let (route, query) = path.split_once('?').unwrap_or((path, ""));
  let json_reply = |status: u16, body: Value| Reply::Status {
    status,
    headers: vec![("content-type".to_owned(), "application/json".to_owned())],
    body: body.to_string(),
  };
  if let Some(id) = route.strip_prefix("/v1/models/") {
    return match state.models.iter().find(|model| model["id"] == json!(id)) {
      Some(model) => json_reply(200, model.clone()),
      None => json_reply(
        404,
        json!({"type": "error", "error": {"type": "not_found_error", "message": "no model"}}),
      ),
    };
  }
  let after = query
    .split('&')
    .find_map(|pair| pair.strip_prefix("after_id="))
    .map(str::to_owned);
  let start = after
    .and_then(|after| state.models.iter().position(|model| model["id"] == json!(after)))
    .map_or(0, |at| at + 1);
  let page: Vec<Value> = state.models.iter().skip(start).take(state.page_size).cloned().collect();
  let has_more = start + page.len() < state.models.len();
  let last_id = page.last().map(|model| model["id"].clone());
  json_reply(200, json!({"data": page, "has_more": has_more, "last_id": last_id}))
}

async fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
  let mut buffer = Vec::new();
  let head_end = loop {
    let mut chunk = [0_u8; 4096];
    let read = stream.read(&mut chunk).await.ok()?;
    if read == 0 {
      return None;
    }
    buffer.extend_from_slice(&chunk[..read]);
    if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
      break at;
    }
  };
  let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
  let mut lines = head.split("\r\n");
  let mut first = lines.next()?.split(' ');
  let (method, path) = (first.next()?.to_owned(), first.next()?.to_owned());
  let headers: Vec<(String, String)> = lines
    .filter_map(|line| line.split_once(':'))
    .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
    .collect();
  let length: usize = headers
    .iter()
    .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
    .and_then(|(_, value)| value.parse().ok())
    .unwrap_or(0);
  let mut body = buffer[head_end + 4..].to_vec();
  while body.len() < length {
    let mut chunk = [0_u8; 4096];
    let read = stream.read(&mut chunk).await.ok()?;
    if read == 0 {
      break;
    }
    body.extend_from_slice(&chunk[..read]);
  }
  Some(Recorded {
    method,
    path,
    headers,
    body,
  })
}
