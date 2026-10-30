//! Following the browser for the window: each tab's screencast frames, loading and dialogs, and the browser's own
//! reports of tabs that changed or closed.

use std::{
  sync::{Arc, Weak},
  time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use orchester_ipc::browser::{BrowserDialog, BrowserEvent, DialogKind, TabOwner};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  sync::{mpsc, oneshot},
  time::Instant,
};

use super::{Window, tab_id};
use crate::browser::{
  cdp::{Cdp, CdpEvent, SessionId, TargetId},
  tabs::{ClosedBy, Tabs},
};

/// How long after a target closed in a browser's own window the rest of a closing window's tabs have to close too.
const WINDOW_CLOSING: Duration = Duration::from_millis(500);

/// One tab of the browser to follow: which, on what session, and whose.
pub(in crate::browser) struct Followed {
  pub(in crate::browser) target: TargetId,
  pub(in crate::browser) session: SessionId,
  pub(in crate::browser) owner: TabOwner,
}

/// Who learns that a browser in a window of its own closed that window or ended ([`follow_window_targets`]).
pub(in crate::browser) struct WindowWatch {
  pub(in crate::browser) cdp: Weak<Cdp>,
  pub(in crate::browser) closed: oneshot::Sender<()>,
}

/// Follows one tab's `events` until the tab is no longer kept: acknowledges and keeps its screencast frames, notes
/// whether its page loads, and tells the window of the dialogs of the person's tabs (an agent's are its tools').
pub(in crate::browser) async fn follow_tab(
  cdp: Arc<Cdp>,
  tab: Followed,
  mut events: mpsc::UnboundedReceiver<CdpEvent>,
  tabs: Arc<Mutex<Tabs>>,
  window: Arc<Window>,
) {
  let person = tab.owner == TabOwner::Person;
  while let Some(event) = events.recv().await {
    match event.method.as_str() {
      "Page.screencastFrame" => screencast_frame(&cdp, &tab, &window, &event.params).await,
      loading @ ("Page.frameStartedLoading" | "Page.frameStoppedLoading") => {
        // The main frame of a tab has the tab's id; its frames' loads are their own.
        let main = event.params.get("frameId").and_then(Value::as_str) == Some(tab.target.as_str());
        if main
          && tabs
            .lock()
            .set_loading(&tab.target, loading == "Page.frameStartedLoading")
        {
          window.changed();
        }
      }
      "Page.javascriptDialogOpening" if person => window.send(BrowserEvent::DialogOpened {
        tab: tab_id(&tab.target),
        dialog: dialog(&event.params),
      }),
      "Page.javascriptDialogClosed" if person => window.send(BrowserEvent::DialogClosed {
        tab: tab_id(&tab.target),
      }),
      _ => {}
    }
  }
}

/// Acknowledges a frame at once, so the browser sends the next as the page changes, and keeps it for the window
/// while its tab is shown.
async fn screencast_frame(cdp: &Cdp, tab: &Followed, window: &Window, params: &Value) {
  if let Some(frame) = params.get("sessionId").and_then(Value::as_u64)
    && let Err(error) = cdp
      .call(
        Some(&tab.session),
        "Page.screencastFrameAck",
        json!({ "sessionId": frame }),
      )
      .await
  {
    tracing::debug!(%error, "a screencast frame was not acknowledged");
  }
  let Some(data) = params.get("data").and_then(Value::as_str) else {
    tracing::warn!("a screencast frame came without its image");
    return;
  };
  let jpeg = match STANDARD.decode(data) {
    Ok(jpeg) => jpeg,
    Err(error) => {
      tracing::warn!(%error, "a screencast frame's image is not base64");
      return;
    }
  };
  let metadata = &params["metadata"];
  window.frame(
    &tab.target,
    (
      css_pixels(&metadata["deviceWidth"]),
      css_pixels(&metadata["deviceHeight"]),
    ),
    jpeg,
  );
}

/// A size of the frame's metadata in whole CSS pixels; 0 when the browser left it out.
fn css_pixels(value: &Value) -> u32 {
  // `as` saturates: a negative size is 0, an absurd one `u32::MAX`.
  value.as_f64().map_or(0, |pixels| pixels.round() as u32)
}

fn dialog(params: &Value) -> BrowserDialog {
  let text = |field: &str| params.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
  let kind = match params.get("type").and_then(Value::as_str) {
    Some("confirm") => DialogKind::Confirm,
    Some("prompt") => DialogKind::Prompt,
    Some("beforeunload") => DialogKind::BeforeUnload,
    _ => DialogKind::Alert,
  };
  BrowserDialog {
    kind,
    message: text("message"),
    default_prompt: text("defaultPrompt"),
  }
}

/// Follows the browser's own `events` until its connection ends: forgets each tab the browser reports closed that is
/// still kept, which neither its run nor the window closed, and tells the window when a kept tab changed its title or
/// address. A browser that ends is ended for the window too ([`Window::ended`]), unless another started since.
pub(in crate::browser) async fn follow_targets(
  mut events: mpsc::UnboundedReceiver<CdpEvent>,
  tabs: Arc<Mutex<Tabs>>,
  window: Arc<Window>,
  generation: u64,
) {
  while let Some(event) = events.recv().await {
    let Some(target) = target_of(&event) else {
      continue;
    };
    match event.method.as_str() {
      "Target.targetDestroyed" => forget_destroyed(&target, &tabs, &window),
      "Target.targetInfoChanged" => page_reported(&event, &target, &tabs, &window),
      _ => {}
    }
  }
  window.ended(generation);
}

/// [`follow_targets`] for a browser in a window of its own, which the person can close. The window's tabs are reported
/// closed one by one before anything else shows that it closed, and on macOS the browser even runs on without a window,
/// so a closed target is only taken for the person's single tab once [`WINDOW_CLOSING`] passed and the browser still
/// has a page. Otherwise, or when the connection ends, `watch` is told, with every tab still kept to reopen it.
pub(in crate::browser) async fn follow_window_targets(
  mut events: mpsc::UnboundedReceiver<CdpEvent>,
  tabs: Arc<Mutex<Tabs>>,
  window: Arc<Window>,
  watch: WindowWatch,
) {
  let mut destroyed = Vec::new();
  let mut settles = Instant::now();
  loop {
    let event = tokio::select! {
      event = events.recv() => event,
      () = tokio::time::sleep_until(settles), if !destroyed.is_empty() => {
        if !has_a_page(&watch.cdp).await {
          break;
        }
        for target in destroyed.drain(..) {
          forget_destroyed(&target, &tabs, &window);
        }
        continue;
      }
    };
    let Some(event) = event else {
      break;
    };
    let Some(target) = target_of(&event) else {
      continue;
    };
    match event.method.as_str() {
      "Target.targetDestroyed" => {
        destroyed.push(target);
        settles = Instant::now() + WINDOW_CLOSING;
      }
      "Target.targetInfoChanged" => page_reported(&event, &target, &tabs, &window),
      _ => {}
    }
  }
  if watch.closed.send(()).is_err() {
    tracing::debug!("nothing waits for the browser's window to close any more");
  }
}

/// Whether the browser answers that it has a page; one that does not answer has none to go on with.
async fn has_a_page(cdp: &Weak<Cdp>) -> bool {
  let Some(cdp) = cdp.upgrade() else {
    return false;
  };
  cdp
    .targets()
    .await
    .is_ok_and(|listed| listed.iter().any(|target| target.kind == "page"))
}

/// The target a browser's event is about.
fn target_of(event: &CdpEvent) -> Option<TargetId> {
  event
    .params
    .get("targetId")
    .or_else(|| event.params.pointer("/targetInfo/targetId"))
    .and_then(Value::as_str)
    .map(TargetId::from)
}

fn forget_destroyed(target: &TargetId, tabs: &Mutex<Tabs>, window: &Window) {
  if tabs.lock().closed_by(target, ClosedBy::Browser) {
    window.forget(target);
    window.changed();
  }
}

/// Keeps what the browser reports of `target`'s page, and tells the window when the tab is kept.
fn page_reported(event: &CdpEvent, target: &TargetId, tabs: &Mutex<Tabs>, window: &Window) {
  let info = |field: &str| event.params["targetInfo"][field].as_str().unwrap_or_default();
  if tabs.lock().reported(target, info("title"), info("url")) {
    window.changed();
  }
}
