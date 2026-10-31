//! What a run's tab reported since it opened, as `browser_console` and `browser_network` read it
//! (`docs/design/browser.md`, "What the agent gets"): its console messages and uncaught exceptions, its requests with
//! their method, URL, status, type and size, never their headers or bodies, and the JavaScript dialogs the runner
//! dismissed for the agent and the files its page asked for, whose chooser the browser cancelled, which the next
//! answer on the tab reports. Each list keeps its newest entries only.

use std::{
  collections::VecDeque,
  sync::{Arc, Weak},
};

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
  cdp::{Cdp, CdpEvent, SessionId},
  tab::TabState,
};

/// The most console messages and requests a tab keeps, each.
pub const KEPT: usize = 1000;
/// The longest text of one message, URL or dialog kept.
const MAX_TEXT_CHARS: usize = 2000;
/// What the agent is told of a file chooser its page opened: agents never open the system's file dialog.
pub const FILE_ASKED: &str = "a file chooser was cancelled: the page asked for a file, and agents cannot choose files";

/// One console message, or an uncaught exception (`exception`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleMessage {
  /// `log`, `info`, `warning`, `error`, `debug`, … as the page or the browser reported it, or `exception`.
  pub level: String,
  pub text: String,
}

impl ConsoleMessage {
  /// An error or an uncaught exception.
  pub fn is_error(&self) -> bool {
    matches!(self.level.as_str(), "error" | "exception" | "assert")
  }
}

/// One request of the tab: what it asked and how it ended, without headers or bodies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
  id: String,
  pub method: String,
  pub url: String,
  /// The resource type: `Document`, `Script`, `XHR`, `Fetch`, `Image`, …
  pub kind: String,
  /// The response's status, once it came.
  pub status: Option<i64>,
  /// The bytes it took on the wire, once it finished.
  pub size: Option<u64>,
  /// Why it failed, when it did.
  pub failed: Option<String>,
}

/// What a tab reported, newest last.
#[derive(Default)]
pub struct TabLog {
  console: VecDeque<ConsoleMessage>,
  requests: VecDeque<Request>,
  dialogs: Vec<String>,
}

impl TabLog {
  pub fn console(&self) -> impl DoubleEndedIterator<Item = &ConsoleMessage> {
    self.console.iter()
  }

  pub fn requests(&self) -> impl DoubleEndedIterator<Item = &Request> {
    self.requests.iter()
  }

  /// The dialogs dismissed since the last answer on the tab said so.
  pub fn take_dialogs(&mut self) -> Vec<String> {
    std::mem::take(&mut self.dialogs)
  }

  /// Records what `event` reports.
  fn record(&mut self, event: &CdpEvent) {
    let params = &event.params;
    match event.method.as_str() {
      "Runtime.consoleAPICalled" => self.message(text_of(params, "type"), &arguments(&params["args"])),
      "Runtime.exceptionThrown" => {
        let details = &params["exceptionDetails"];
        let text = details["exception"]["description"]
          .as_str()
          .or_else(|| details["text"].as_str())
          .unwrap_or("an exception");
        self.message("exception".to_owned(), text);
      }
      "Log.entryAdded" => {
        let entry = &params["entry"];
        self.message(text_of(entry, "level"), entry["text"].as_str().unwrap_or_default());
      }
      "Page.javascriptDialogOpening" => self.dialogs.push(cut(&format!(
        "a {} dialog was dismissed: {}",
        text_of(params, "type"),
        text_of(params, "message")
      ))),
      "Page.fileChooserOpened" => self.dialogs.push(FILE_ASKED.to_owned()),
      "Network.requestWillBeSent" => self.request_sent(params),
      "Network.responseReceived" => {
        if let Some(request) = self.request(params) {
          request.status = params["response"]["status"].as_i64();
          request.kind = text_of(params, "type");
        }
      }
      "Network.loadingFinished" => {
        if let Some(request) = self.request(params) {
          // CDP reports the length as a number with a fraction; whole bytes are meant.
          request.size = params["encodedDataLength"].as_f64().map(|bytes| bytes.max(0.0) as u64);
        }
      }
      "Network.loadingFailed" => {
        if let Some(request) = self.request(params) {
          request.failed = Some(text_of(params, "errorText"));
        }
      }
      _ => {}
    }
  }

  fn message(&mut self, level: String, text: &str) {
    keep(&mut self.console, ConsoleMessage { level, text: cut(text) });
  }

  /// A new request; a redirect ends the request it follows with the redirect's status.
  fn request_sent(&mut self, params: &Value) {
    if let Some(status) = params["redirectResponse"]["status"].as_i64()
      && let Some(earlier) = self.request(params)
    {
      earlier.status = Some(status);
    }
    let request = &params["request"];
    keep(
      &mut self.requests,
      Request {
        id: text_of(params, "requestId"),
        method: text_of(request, "method"),
        url: cut(request["url"].as_str().unwrap_or_default()),
        kind: text_of(params, "type"),
        status: None,
        size: None,
        failed: None,
      },
    );
  }

  /// The latest request of the id `params` name.
  fn request(&mut self, params: &Value) -> Option<&mut Request> {
    let id = params["requestId"].as_str()?;
    self.requests.iter_mut().rev().find(|request| request.id == id)
  }
}

/// Records what the tab of `session` reports in `state`'s log, and dismisses each JavaScript dialog it opens, which
/// would block the page, until the tab is no longer listened to: it closed or is no longer a run's
/// (`RunTabs`), or the connection ended. A file chooser needs nothing here: the browser cancelled it as it opened.
/// `follower` is held until then.
pub(super) async fn follow(
  mut events: mpsc::UnboundedReceiver<CdpEvent>,
  cdp: Weak<Cdp>,
  session: SessionId,
  state: Arc<TabState>,
  follower: Arc<()>,
) {
  let _follower = follower;
  while let Some(event) = events.recv().await {
    state.log.lock().record(&event);
    if event.method != "Page.javascriptDialogOpening" {
      continue;
    }
    let Some(cdp) = cdp.upgrade() else {
      return;
    };
    let dismissed = cdp
      .call(
        Some(&session),
        "Page.handleJavaScriptDialog",
        json!({ "accept": false }),
      )
      .await;
    if let Err(error) = dismissed {
      tracing::warn!(%error, "a page's dialog was not dismissed");
    }
  }
}

fn keep<T>(list: &mut VecDeque<T>, entry: T) {
  if list.len() == KEPT {
    list.pop_front();
  }
  list.push_back(entry);
}

fn text_of(value: &Value, field: &str) -> String {
  value[field].as_str().unwrap_or_default().to_owned()
}

/// A console call's arguments as one line: each value as text, an object by its description.
fn arguments(args: &Value) -> String {
  let shown: Vec<String> = args
    .as_array()
    .into_iter()
    .flatten()
    .map(|arg| match &arg["value"] {
      Value::String(text) => text.clone(),
      Value::Null => arg["description"]
        .as_str()
        .or_else(|| arg["type"].as_str())
        .unwrap_or_default()
        .to_owned(),
      value => value.to_string(),
    })
    .collect();
  shown.join(" ")
}

/// `text` within [`MAX_TEXT_CHARS`] characters.
fn cut(text: &str) -> String {
  match text.char_indices().nth(MAX_TEXT_CHARS) {
    Some((end, _)) => format!("{}…", &text[..end]),
    None => text.to_owned(),
  }
}
