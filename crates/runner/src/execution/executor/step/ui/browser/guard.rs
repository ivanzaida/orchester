//! The browser driver's origin guard (`orchester_core::orchester_tools::ui::origins`): every request of a driver's tab
//! is paused before it leaves (CDP `Fetch`), let through when its URL is one of the services' origins or the page's
//! own content, and failed as blocked by the client otherwise. What it blocked is kept, newest last and at most
//! [`KEPT`] lines, for `ui_logs` and for the next answer of the driver, which says what was blocked since the last.

use std::{collections::VecDeque, sync::Arc};

use orchester_core::orchester_tools::ui::origins::AllowedOrigins;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::browser::{Tab, cdp::CdpEvent};

/// The most blocked requests kept.
const KEPT: usize = 200;
/// The longest URL a blocked line shows.
const MAX_URL_CHARS: usize = 500;

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
    let line = format!("blocked {method} {shown} ({kind}): not an origin of the services this step started");
    let mut blocked = self.blocked.lock();
    if blocked.lines.len() == KEPT {
      blocked.lines.pop_front();
    }
    blocked.lines.push_back(line);
    blocked.unseen = (blocked.unseen + 1).min(KEPT);
    false
  }
}

/// Answers each request `tab` pauses (`events` being its events from before `Fetch.enable`) as `guard` judges it, until
/// the tab is no longer listened to: it closed or the browser's connection ended.
pub(super) async fn follow(mut events: mpsc::UnboundedReceiver<CdpEvent>, tab: Tab, guard: Arc<Guard>) {
  while let Some(event) = events.recv().await {
    if event.method != "Fetch.requestPaused" {
      continue;
    }
    let Some(request) = event.params["requestId"].as_str() else {
      continue;
    };
    let answered = if guard.admits(&event.params) {
      tab.call("Fetch.continueRequest", json!({ "requestId": request })).await
    } else {
      tab
        .call(
          "Fetch.failRequest",
          json!({ "requestId": request, "errorReason": "BlockedByClient" }),
        )
        .await
    };
    // A request whose page went away meanwhile is not paused any more: nothing waits for the answer.
    if let Err(error) = answered {
      tracing::debug!(%error, "a paused request of the browser driver's tab was not answered");
    }
  }
}
