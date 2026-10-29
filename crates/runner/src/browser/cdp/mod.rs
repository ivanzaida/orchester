//! A small client of the Chrome DevTools Protocol (`docs/design/browser.md`, "Connection and dependency"): one
//! WebSocket per browser, on which each command goes out with an id and its answer is awaited by that id within a time
//! limit, and each event goes to whoever listens to its session. A tab's commands carry the [`SessionId`] that
//! `Target.attachToTarget {flatten: true}` gave; the browser's own carry none. Only about forty CDP methods are used,
//! so there is no CDP crate: commands, answers and events are JSON values.

mod connection;

use std::{
  collections::HashMap,
  fmt,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Duration,
};

use parking_lot::Mutex;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
  sync::{mpsc, oneshot},
  time::Instant,
};

/// How long the browser may take to answer a command, unless the caller gives a limit of its own.
pub const COMMAND_LIMIT: Duration = Duration::from_secs(30);
/// How long connecting to the browser's debugging address may take.
const CONNECT_LIMIT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CdpError {
  #[error("cannot connect to the browser's debugging address: {0}")]
  Connect(String),
  #[error("the browser refused {method}: {message} (code {code})")]
  Refused { method: String, code: i64, message: String },
  #[error("the browser did not answer {method} within {limit:?}")]
  TimedOut { method: String, limit: Duration },
  #[error("the browser did not report {event} within {limit:?}")]
  NoEvent { event: String, limit: Duration },
  #[error("the browser's answer to {method} has no {field}")]
  Unexpected { method: String, field: &'static str },
  #[error("the connection to the browser is closed")]
  Closed,
}

/// A target of the browser, such as a tab, by the id the browser gave it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(String);

/// The connection's attachment to one target: the commands for that target carry it, and its events come with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl TargetId {
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<&str> for TargetId {
  fn from(id: &str) -> Self {
    Self(id.to_owned())
  }
}

impl SessionId {
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for TargetId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// What the browser reported: the event's method, its parameters and the session of the target it is about, `None`
/// for the browser's own.
#[derive(Clone, Debug, PartialEq)]
pub struct CdpEvent {
  pub method: String,
  pub params: Value,
  pub session: Option<SessionId>,
}

/// A target as `Target.getTargets` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetInfo {
  pub id: TargetId,
  /// `page` for a tab; also `browser`, `service_worker` and others.
  pub kind: String,
  pub title: String,
  pub url: String,
}

/// The connection to one browser. Dropping it closes the WebSocket.
pub struct Cdp {
  outgoing: mpsc::UnboundedSender<String>,
  routes: Arc<Routes>,
  next_id: AtomicU64,
}

/// Where the connection hands what the browser says: answers to the commands awaiting them, events to their listeners.
#[derive(Default)]
struct Routes {
  state: Mutex<RouteState>,
}

#[derive(Default)]
struct RouteState {
  /// The connection ended: nothing more is answered or reported.
  closed: bool,
  awaiting: HashMap<u64, oneshot::Sender<Answer>>,
  listeners: Vec<Listener>,
}

/// A command's result, or the browser's error code and message.
type Answer = Result<Value, (i64, String)>;

struct Listener {
  session: Option<SessionId>,
  events: mpsc::UnboundedSender<CdpEvent>,
}

impl Cdp {
  /// Connects to the browser's WebSocket at `url` (`ws://127.0.0.1:<port>/devtools/browser/<id>`).
  pub async fn connect(url: &str) -> Result<Self, CdpError> {
    let (socket, _) = tokio::time::timeout(CONNECT_LIMIT, tokio_tungstenite::connect_async(url))
      .await
      .map_err(|_| CdpError::Connect(format!("no answer within {CONNECT_LIMIT:?}")))?
      .map_err(|error| CdpError::Connect(error.to_string()))?;
    let routes = Arc::new(Routes::default());
    let outgoing = connection::serve(socket, Arc::clone(&routes));
    Ok(Self {
      outgoing,
      routes,
      next_id: AtomicU64::new(1),
    })
  }

  /// Whether the connection ended: the browser closed it or exited.
  pub fn is_closed(&self) -> bool {
    self.routes.state.lock().closed
  }

  /// Sends `method` with `params`, to the target of `session` or to the browser itself, and waits up to
  /// [`COMMAND_LIMIT`] for its result.
  pub async fn call(&self, session: Option<&SessionId>, method: &str, params: Value) -> Result<Value, CdpError> {
    self.call_within(session, method, params, COMMAND_LIMIT).await
  }

  /// [`Cdp::call`] waiting up to `limit`.
  pub async fn call_within(
    &self,
    session: Option<&SessionId>,
    method: &str,
    params: Value,
    limit: Duration,
  ) -> Result<Value, CdpError> {
    let id = self.next_id.fetch_add(1, Ordering::Relaxed);
    let (answer, answered) = oneshot::channel();
    {
      let mut state = self.routes.state.lock();
      if state.closed {
        return Err(CdpError::Closed);
      }
      state.awaiting.insert(id, answer);
    }
    let mut message = json!({ "id": id, "method": method, "params": params });
    if let Some(session) = session {
      message["sessionId"] = json!(session.as_str());
    }
    if self.outgoing.send(message.to_string()).is_err() {
      self.routes.forget(id);
      return Err(CdpError::Closed);
    }
    match tokio::time::timeout(limit, answered).await {
      Ok(Ok(Ok(result))) => Ok(result),
      Ok(Ok(Err((code, message)))) => Err(CdpError::Refused {
        method: method.to_owned(),
        code,
        message,
      }),
      Ok(Err(_)) => Err(CdpError::Closed),
      Err(_) => {
        self.routes.forget(id);
        Err(CdpError::TimedOut {
          method: method.to_owned(),
          limit,
        })
      }
    }
  }

  /// The events of the target of `session` from now on, or the browser's own for `None`. The receiver ends with the
  /// connection; dropping it stops the listening.
  pub fn subscribe(&self, session: Option<SessionId>) -> mpsc::UnboundedReceiver<CdpEvent> {
    let (events, receiver) = mpsc::unbounded_channel();
    let mut state = self.routes.state.lock();
    // A closed connection reports nothing: the sender is dropped here, so the receiver ends at once.
    if !state.closed {
      state.listeners.push(Listener { session, events });
    }
    receiver
  }

  /// The browser's targets: its tabs and the rest.
  pub async fn targets(&self) -> Result<Vec<TargetInfo>, CdpError> {
    const METHOD: &str = "Target.getTargets";
    let answer = self.call(None, METHOD, json!({})).await?;
    let infos = answer
      .get("targetInfos")
      .and_then(Value::as_array)
      .ok_or_else(|| unexpected(METHOD, "targetInfos"))?;
    infos
      .iter()
      .map(|info| {
        Ok(TargetInfo {
          id: TargetId(text(info, METHOD, "targetId")?),
          kind: text(info, METHOD, "type")?,
          title: text(info, METHOD, "title")?,
          url: text(info, METHOD, "url")?,
        })
      })
      .collect()
  }

  /// Opens a new tab on `url` and attaches to it.
  pub async fn create_tab(&self, url: &str) -> Result<TargetId, CdpError> {
    const METHOD: &str = "Target.createTarget";
    let created = self.call(None, METHOD, json!({ "url": url })).await?;
    Ok(TargetId(text(&created, METHOD, "targetId")?))
  }

  /// Attaches to `target` on this connection: the session its commands carry.
  pub async fn attach(&self, target: &TargetId) -> Result<SessionId, CdpError> {
    const METHOD: &str = "Target.attachToTarget";
    let attached = self
      .call(None, METHOD, json!({ "targetId": target.as_str(), "flatten": true }))
      .await?;
    Ok(SessionId(text(&attached, METHOD, "sessionId")?))
  }

  /// Closes `target`.
  pub async fn close_target(&self, target: &TargetId) -> Result<(), CdpError> {
    self
      .call(None, "Target.closeTarget", json!({ "targetId": target.as_str() }))
      .await
      .map(drop)
  }
}

/// Waits up to `limit` for the first event of `events` named `method` that `matches` accepts.
pub async fn wait_for_event(
  events: &mut mpsc::UnboundedReceiver<CdpEvent>,
  method: &str,
  matches: impl Fn(&Value) -> bool,
  limit: Duration,
) -> Result<CdpEvent, CdpError> {
  let deadline = Instant::now() + limit;
  loop {
    match tokio::time::timeout_at(deadline, events.recv()).await {
      Ok(Some(event)) if event.method == method && matches(&event.params) => return Ok(event),
      Ok(Some(_)) => {}
      Ok(None) => return Err(CdpError::Closed),
      Err(_) => {
        return Err(CdpError::NoEvent {
          event: method.to_owned(),
          limit,
        });
      }
    }
  }
}

impl Routes {
  /// A command that is no longer awaited: its answer, if it comes, is dropped.
  fn forget(&self, id: u64) {
    self.state.lock().awaiting.remove(&id);
  }

  /// Hands one message of the browser to the command awaiting it or to the listeners of its session.
  fn deliver(&self, text: &str) {
    let message = match serde_json::from_str::<Value>(text) {
      Ok(message) => message,
      Err(error) => {
        tracing::warn!(%error, "the browser sent a message that is not JSON");
        return;
      }
    };
    if let Some(id) = message.get("id").and_then(Value::as_u64) {
      self.answer(id, &message);
    } else if let Some(method) = message.get("method").and_then(Value::as_str) {
      self.report(&CdpEvent {
        method: method.to_owned(),
        params: message.get("params").cloned().unwrap_or(Value::Null),
        session: message
          .get("sessionId")
          .and_then(Value::as_str)
          .map(|session| SessionId(session.to_owned())),
      });
    }
  }

  fn answer(&self, id: u64, message: &Value) {
    let answer = match message.get("error") {
      Some(error) => Err((
        error.get("code").and_then(Value::as_i64).unwrap_or_default(),
        error
          .get("message")
          .and_then(Value::as_str)
          .unwrap_or_default()
          .to_owned(),
      )),
      None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
    };
    let Some(awaiting) = self.state.lock().awaiting.remove(&id) else {
      tracing::debug!(id, "an answer came for a command no longer awaited");
      return;
    };
    // The caller stopped waiting (its time ran out or it was dropped); nobody needs the answer.
    let _ = awaiting.send(answer);
  }

  fn report(&self, event: &CdpEvent) {
    let mut state = self.state.lock();
    state
      .listeners
      .retain(|listener| listener.session != event.session || listener.events.send(event.clone()).is_ok());
  }

  /// The connection ended: every command still awaited fails as closed, and every listener's receiver ends.
  fn close(&self) {
    let mut state = self.state.lock();
    state.closed = true;
    state.awaiting.clear();
    state.listeners.clear();
  }
}

fn text(value: &Value, method: &str, field: &'static str) -> Result<String, CdpError> {
  value
    .get(field)
    .and_then(Value::as_str)
    .map(str::to_owned)
    .ok_or_else(|| unexpected(method, field))
}

fn unexpected(method: &str, field: &'static str) -> CdpError {
  CdpError::Unexpected {
    method: method.to_owned(),
    field,
  }
}
