//! The fake model server (`docs/architecture/native-agent.md` §8, layer 2): a tokio listener on
//! `127.0.0.1:0` that answers scripted turns over plain HTTP/1.1, one request per connection.
//!
//! It checks what the design requires of every conversation: each Messages request's `system` and
//! `tools` are the same JSON text as the previous request's, and the previous request's messages are a
//! prefix of its own, message by message (append-only history), unless the turn says the history may
//! change. It echoes the
//! cache: a `cache_read_input_tokens` of `"echo"` in a scripted `message_start` becomes the size of the
//! prefix shared with the previous request. `GET /v1/models` and `GET /v1/models/{id}` are answered from
//! a model catalog unless a scripted turn is waiting for that path.

use std::{
  collections::VecDeque,
  net::SocketAddr,
  panic::{self, AssertUnwindSafe},
  sync::Arc,
  time::Duration,
};

use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  task::JoinHandle,
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
enum Reply {
  Stream(Vec<Part>),
  Status {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
  },
}

type Expectation = Box<dyn Fn(&Recorded) + Send>;

struct Turn {
  path: &'static str,
  expect: Option<Expectation>,
  reply: Reply,
  history_may_change: bool,
}

/// The turns a test expects, in order.
#[derive(Default)]
pub struct Script {
  turns: Vec<Turn>,
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

  /// A Messages turn answered with a stream.
  pub fn reply_sse(self, parts: Vec<Part>) -> Self {
    self.push("/v1/messages", Reply::Stream(parts))
  }

  /// A turn on `path` answered with `status`, `headers` and `body`.
  pub fn reply_status(self, path: &'static str, status: u16, headers: &[(&str, &str)], body: &str) -> Self {
    let headers = headers
      .iter()
      .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
      .collect();
    self.push(
      path,
      Reply::Status {
        status,
        headers,
        body: body.to_owned(),
      },
    )
  }

  /// A Messages turn answered with an error of `status` and the provider's error body.
  pub fn reply_error(self, status: u16, kind: &str, message: &str) -> Self {
    let body = json!({"type": "error", "error": {"type": kind, "message": message}}).to_string();
    self.reply_status("/v1/messages", status, &[], &body)
  }

  fn push(mut self, path: &'static str, reply: Reply) -> Self {
    self.turns.push(Turn {
      path,
      expect: self.expect.take(),
      reply,
      history_may_change: std::mem::take(&mut self.history_may_change),
    });
    self
  }
}

#[derive(Default)]
struct State {
  turns: VecDeque<Turn>,
  models: Vec<Value>,
  page_size: usize,
  requests: Vec<Recorded>,
  violations: Vec<String>,
  last_messages: Option<Value>,
}

/// A running fake server.
pub struct FakeServer {
  address: SocketAddr,
  state: Arc<Mutex<State>>,
  task: JoinHandle<()>,
}

impl FakeServer {
  /// A server playing `script`, with `models` as its catalog, listed `page_size` per page.
  pub async fn start(script: Script, models: Vec<Value>, page_size: usize) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind the fake server");
    let address = listener.local_addr().expect("the fake server's address");
    let state = Arc::new(Mutex::new(State {
      turns: script.turns.into(),
      models,
      page_size,
      ..State::default()
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

  /// The Messages requests so far.
  pub fn messages_requests(&self) -> Vec<Recorded> {
    self
      .requests()
      .into_iter()
      .filter(|request| request.path == "/v1/messages")
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
/// a violation.
fn answer(request: &Recorded, state: &Mutex<State>) -> Reply {
  let mut state = state.lock();
  state.requests.push(request.clone());
  let scripted = state
    .turns
    .front()
    .is_some_and(|turn| request.path.starts_with(turn.path));
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
    if request.path == "/v1/messages" {
      check_history(&mut state, request, turn.history_may_change);
    }
    return echo_cache(turn.reply, &state);
  }
  if request.method == "GET" && request.path.starts_with("/v1/models") {
    return catalog(&state, &request.path);
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

/// Records a violation when `request` does not continue the previous Messages request.
fn check_history(state: &mut State, request: &Recorded, may_change: bool) {
  let body = request.json();
  if let (Some(previous), false) = (&state.last_messages, may_change) {
    for field in ["system", "tools"] {
      if text(&previous[field]) != text(&body[field]) {
        state
          .violations
          .push(format!("`{field}` changed within the conversation"));
      }
    }
    let before = previous["messages"].as_array().cloned().unwrap_or_default();
    let now = body["messages"].as_array().cloned().unwrap_or_default();
    let prefix = now.len() >= before.len() && before.iter().zip(&now).all(|(a, b)| text(a) == text(b));
    if !prefix {
      state
        .violations
        .push("the history was edited, not appended to".to_owned());
    }
  }
  state.last_messages = Some(body);
}

/// `value` as JSON text.
fn text(value: &Value) -> String {
  value.to_string()
}

/// `reply` with an echoed cache read: the bytes of the messages the request shares with the previous
/// one, as tokens.
fn echo_cache(reply: Reply, state: &State) -> Reply {
  let Reply::Stream(parts) = reply else {
    return reply;
  };
  let cached = shared_prefix_tokens(state);
  let parts = parts
    .into_iter()
    .map(|part| match part {
      Part::Event(mut event) if event["message"]["usage"]["cache_read_input_tokens"] == json!("echo") => {
        event["message"]["usage"]["cache_read_input_tokens"] = json!(cached);
        Part::Event(event)
      }
      other => other,
    })
    .collect();
  Reply::Stream(parts)
}

fn shared_prefix_tokens(state: &State) -> u64 {
  let requests: Vec<&Recorded> = state
    .requests
    .iter()
    .filter(|request| request.path == "/v1/messages")
    .collect();
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
