//! The browser driver's origin guard (`orchester_core::orchester_tools::ui::origins`): every request of a driver's tab
//! is paused before it leaves (CDP `Fetch`), let through when its URL is one of the services' origins or the page's
//! own content, and failed as blocked by the client otherwise. What the page starts besides — a worker, a frame of
//! another process — is attached to as it starts and held there until the same guard is in place on it, and so on for
//! what that one starts ([`guard`]). A new window is never opened: each document the tab loads comes with a sandbox
//! that leaves out popups ([`SANDBOX`]), so the browser refuses `window.open` and `target=_blank` and logs it on the
//! page's console. What it blocked is kept, newest last and at most [`KEPT`] lines, for `ui_logs` and for the next
//! answer of the driver, which says what was blocked since the last.

use std::{
  collections::{HashMap, VecDeque},
  pin::Pin,
  sync::Arc,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use orchester_core::orchester_tools::ui::origins::AllowedOrigins;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::browser::{
  Tab,
  cdp::{CdpError, CdpEvent, SessionId, TargetId},
};

/// The most blocked requests kept.
const KEPT: usize = 200;
/// The longest URL a blocked line shows.
const MAX_URL_CHARS: usize = 500;
/// The policy each document of a driver's tab gets besides its own: everything a page of a web app does but opening
/// another window, a new tab of the browser this guard would not be on (`allow-popups` left out). A frame of the page
/// inherits it.
const SANDBOX: &str = "sandbox allow-forms allow-modals allow-pointer-lock allow-presentation allow-same-origin \
                       allow-scripts allow-top-navigation";
/// The target type of a dedicated worker, which has no `Fetch` of its own: its requests are its page's.
const DEDICATED_WORKER: &str = "worker";

/// What the driver's tabs of one invocation may open, and what they were refused.
#[derive(Default)]
pub(in crate::execution::executor::step) struct Guard {
  allowed: Mutex<AllowedOrigins>,
  blocked: Mutex<Blocked>,
}

#[derive(Default)]
struct Blocked {
  lines: VecDeque<String>,
  /// How many of the newest lines no answer said yet.
  unseen: usize,
}

impl Guard {
  /// From now on the tabs may open the origins of the services listening at `urls`.
  pub(super) fn allow(&self, urls: &[String]) {
    *self.allowed.lock() = AllowedOrigins::of_services(urls.iter().map(String::as_str));
  }

  /// The origins the tabs may open, in order.
  pub(super) fn origins(&self) -> Vec<String> {
    self.allowed.lock().iter().map(ToString::to_string).collect()
  }

  /// Every blocked request kept, newest last.
  pub(super) fn blocked(&self) -> Vec<String> {
    self.blocked.lock().lines.iter().cloned().collect()
  }

  /// The blocked requests no answer said yet, newest last; from now on they are said.
  pub(super) fn take_unseen(&self) -> Vec<String> {
    let mut blocked = self.blocked.lock();
    let unseen = blocked.unseen.min(blocked.lines.len());
    blocked.unseen = 0;
    blocked
      .lines
      .iter()
      .skip(blocked.lines.len() - unseen)
      .cloned()
      .collect()
  }

  /// Whether `request` (a `Fetch.requestPaused` event's parameters) may go on; a refused one is kept.
  fn admits(&self, request: &Value) -> bool {
    let url = request["request"]["url"].as_str().unwrap_or_default();
    if self.allowed.lock().allows(url) {
      return true;
    }
    let method = request["request"]["method"].as_str().unwrap_or("GET");
    let kind = request["resourceType"].as_str().unwrap_or("Other");
    let shown: String = url.chars().take(MAX_URL_CHARS).collect();
    self.refuse(format!(
      "blocked {method} {shown} ({kind}): not an origin of the services this step started"
    ));
    false
  }

  /// Keeps `line`, about something refused, as the newest.
  fn refuse(&self, line: String) {
    let mut blocked = self.blocked.lock();
    if blocked.lines.len() == KEPT {
      blocked.lines.pop_front();
    }
    blocked.lines.push_back(line);
    blocked.unseen = (blocked.unseen + 1).min(KEPT);
  }
}

/// Puts `guard` in place on the tab `tab`: its requests pause until the guard answers them, its documents are
/// sandboxed, and what its page starts is attached to, held until it is guarded the same way and only then let run.
pub(super) async fn guard(tab: &Tab, guard: &Arc<Guard>) -> Result<(), CdpError> {
  put_in_place(tab, guard, "page").await
}

/// [`guard`] on `target`, a target of type `kind`.
async fn put_in_place(target: &Tab, guard: &Arc<Guard>, kind: &str) -> Result<(), CdpError> {
  tokio::spawn(follow(target.events(), target.clone(), Arc::clone(guard)));
  if kind != DEDICATED_WORKER {
    let patterns = json!([
      { "urlPattern": "*", "requestStage": "Request" },
      { "urlPattern": "*", "resourceType": "Document", "requestStage": "Response" },
    ]);
    target.call("Fetch.enable", json!({ "patterns": patterns })).await?;
  }
  target
    .call(
      "Target.setAutoAttach",
      json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
    )
    .await
    .map(drop)
}

/// A guarded target's follower; boxed, as following a target guards the ones it starts, which are followed in turn.
type Following = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Answers each request `tab` pauses (`events` being its events from before `Fetch.enable`) as `guard` judges it, and
/// guards each target its page starts, until the tab is no longer listened to: it closed, or the browser's connection
/// ended. The targets it started are no longer listened to either then.
fn follow(mut events: mpsc::UnboundedReceiver<CdpEvent>, tab: Tab, guard: Arc<Guard>) -> Following {
  Box::pin(async move {
    let mut children = HashMap::new();
    while let Some(event) = events.recv().await {
      match event.method.as_str() {
        "Fetch.requestPaused" => answer(&tab, &guard, &event.params).await,
        "Target.attachedToTarget" => {
          if let Some((session, child)) = adopt(&tab, &guard, &event.params).await {
            children.insert(session, child);
          }
        }
        "Target.detachedFromTarget" => {
          let session = event.params["sessionId"].as_str().unwrap_or_default();
          if let Some(child) = children.remove(session) {
            child.unlisten();
          }
        }
        _ => {}
      }
    }
    for child in children.into_values() {
      child.unlisten();
    }
  })
}

/// Answers the paused request `paused`: before it leaves, lets it go on or fails it as blocked by the client, as
/// `guard` judges it; a document's answer goes on sandboxed ([`sandboxed`]), a redirect as it is, to a request judged
/// again.
async fn answer(tab: &Tab, guard: &Guard, paused: &Value) {
  let Some(request) = paused["requestId"].as_str() else {
    return;
  };
  let status = paused["responseStatusCode"].as_u64();
  let answered = if status.is_some_and(|status| !(300..400).contains(&status)) {
    sandboxed(tab, guard, request, paused).await
  } else if status.is_some() || paused.get("responseErrorReason").is_some() || guard.admits(paused) {
    tab.call("Fetch.continueRequest", json!({ "requestId": request })).await
  } else {
    fail(tab, request).await
  };
  // A request whose page went away meanwhile is not paused any more: nothing waits for the answer.
  if let Err(error) = answered {
    tracing::debug!(%error, "a paused request of the browser driver's tab was not answered");
  }
}

/// Answers the document `request`, answered as `paused` says, with its own body and headers and [`SANDBOX`] besides.
/// Only an answer given whole this way has its headers read again by the browser; one changed on its way is not. A
/// document whose body cannot be had is not let through: it would open new windows.
async fn sandboxed(tab: &Tab, guard: &Guard, request: &str, paused: &Value) -> Result<Value, CdpError> {
  let body = match tab.call("Fetch.getResponseBody", json!({ "requestId": request })).await {
    Ok(body) => body,
    Err(error) => {
      let url: String = paused["request"]["url"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(MAX_URL_CHARS)
        .collect();
      guard.refuse(format!(
        "blocked {url} (Document): it cannot be kept from opening new windows ({error})"
      ));
      return fail(tab, request).await;
    }
  };
  let text = body["body"].as_str().unwrap_or_default();
  let body = if body["base64Encoded"] == true {
    text.to_owned()
  } else {
    STANDARD.encode(text)
  };
  // The body comes decoded: the headers that described it as it came over the wire no longer do.
  let mut headers: Vec<Value> = paused["responseHeaders"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|header| {
      let name = header["name"].as_str().unwrap_or_default();
      !name.eq_ignore_ascii_case("content-encoding") && !name.eq_ignore_ascii_case("content-length")
    })
    .cloned()
    .collect();
  headers.push(json!({ "name": "Content-Security-Policy", "value": SANDBOX }));
  let mut answer = json!({
    "requestId": request,
    "responseCode": paused["responseStatusCode"],
    "responseHeaders": headers,
    "body": body,
  });
  if let Some(phrase) = paused["responseStatusText"]
    .as_str()
    .filter(|phrase| !phrase.is_empty())
  {
    answer["responsePhrase"] = json!(phrase);
  }
  tab.call("Fetch.fulfillRequest", answer).await
}

async fn fail(tab: &Tab, request: &str) -> Result<Value, CdpError> {
  tab
    .call(
      "Fetch.failRequest",
      json!({ "requestId": request, "errorReason": "BlockedByClient" }),
    )
    .await
}

/// The target `parent`'s page started, as `attached` (a `Target.attachedToTarget` event's parameters) says, by its
/// session: guarded, then let run. One that cannot be guarded stays held, never running, and is reported blocked.
async fn adopt(parent: &Tab, guard: &Arc<Guard>, attached: &Value) -> Option<(String, Tab)> {
  let info = &attached["targetInfo"];
  let (Some(session), Some(target)) = (attached["sessionId"].as_str(), info["targetId"].as_str()) else {
    return None;
  };
  let kind = info["type"].as_str().unwrap_or("target");
  let child = parent.child(TargetId::from(target), SessionId::from(session));
  if let Err(error) = put_in_place(&child, guard, kind).await {
    let shown: String = info["url"]
      .as_str()
      .unwrap_or_default()
      .chars()
      .take(MAX_URL_CHARS)
      .collect();
    guard.refuse(format!(
      "blocked {kind} {shown}: its requests cannot be guarded ({error})"
    ));
    return Some((session.to_owned(), child));
  }
  if attached["waitingForDebugger"] == true
    && let Err(error) = child.call("Runtime.runIfWaitingForDebugger", json!({})).await
  {
    tracing::debug!(%error, "a target the browser driver's tab started was not let run");
  }
  Some((session.to_owned(), child))
}
