//! The `browser` UI driver (`docs/architecture/agent-testing.md`, "UI drivers"): a project's web service seen and used
//! in a tab of the run's own in Orchester's Browser — the project's browser the runner drives over CDP
//! ([`crate::browser::ProjectBrowser`]), which the person may watch in the Browser window — not in a browser of its
//! own. `ui_open` opens one tab per service, where the service's `ui_start` says or else at its URL; its origin guard
//! ([`guard`]) is in place before the first page loads, so the tab opens only the origins of the services the step
//! started and every other navigation and request is blocked and reported. The snapshot is the page's accessibility
//! tree with refs of DOM nodes ([`snapshot`]); actions go by ref through the tab's own input ([`act`]); `ui_wait`
//! also takes a selector, a text or an address ([`wait`]); screenshots are saved in the run's storage; `ui_logs` is the
//! page's console errors, its failed requests and what was blocked. The tabs close with the step's services.
//!
//! The browser tools' calls are async, the `ui_*` calls run off the async workers: each call blocks on its own work.

mod act;
mod guard;
mod snapshot;
mod wait;

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{id::RunId, orchester_tools::UiCall, service::ServiceName};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use self::{guard::Guard, snapshot::Snapshot};
use super::super::service::StepServices;
use crate::browser::{BrowserError, ProjectBrowser, Request, Tab, cdp::TargetId};

/// The browser driver's tabs of one invocation, one per service `ui_open` opened, and their guard.
pub(in crate::execution::executor::step) struct WebTabs {
  browser: Arc<ProjectBrowser>,
  run: RunId,
  tabs: Mutex<BTreeMap<ServiceName, TargetId>>,
  guard: Arc<Guard>,
}

impl WebTabs {
  /// No tab yet; they open in `browser`, as `run`'s.
  pub(in crate::execution::executor::step) fn new(browser: Arc<ProjectBrowser>, run: RunId) -> Self {
    Self {
      browser,
      run,
      tabs: Mutex::default(),
      guard: Arc::default(),
    }
  }
}

/// What the driver's calls of one invocation reach.
pub(super) struct WebUi<'a> {
  pub(super) tabs: &'a WebTabs,
  pub(super) services: &'a StepServices,
  /// The run's own storage, where screenshots go.
  pub(super) artifacts: &'a Path,
}

impl WebUi<'_> {
  /// `ui_open(name)`: the service's tab, on the page its UI starts at.
  pub(super) fn open(&self, name: &ServiceName) -> Result<Value, String> {
    self.blocking(self.opened(name))
  }

  /// A call on the service `name`, which `ui_open` opened.
  pub(super) fn run(&self, name: &ServiceName, call: &UiCall) -> Result<Value, String> {
    self.blocking(self.carry_out(name, call))
  }

  /// What `work` answers, carried out on the runtime this blocking call runs beside, followed by what the tabs'
  /// guard blocked since the last answer.
  fn blocking(&self, work: impl Future<Output = Result<Value, String>>) -> Result<Value, String> {
    let runtime = Handle::try_current().map_err(|error| format!("the browser cannot be reached: {error}"))?;
    let mut answer = runtime.block_on(work)?;
    let blocked = self.tabs.guard.take_unseen();
    if !blocked.is_empty() {
      answer["blocked"] = json!(blocked);
    }
    Ok(answer)
  }

  async fn opened(&self, name: &ServiceName) -> Result<Value, String> {
    let start = self.services.ui_place(name).ok_or_else(|| not_running(name))??;
    self.tabs.guard.allow(&self.services.urls());
    let tab = self.tab_for(name).await?;
    self
      .tabs
      .browser
      .load(self.tabs.run, &tab, &start)
      .await
      .map_err(|error| said(&error))?;
    let info = tab.info().await.map_err(|error| error.to_string())?;
    Ok(json!({
      "service": name.as_str(),
      "driver": "browser",
      "tab": info.id.as_str(),
      "url": info.url,
      "title": info.title,
      "allowed_origins": self.tabs.guard.origins(),
    }))
  }

  /// The service's tab: the one opened before while it is still the run's, else a new one, guarded before anything
  /// loads in it and closed when the invocation's services end.
  async fn tab_for(&self, name: &ServiceName) -> Result<Tab, String> {
    let (browser, run) = (&self.tabs.browser, self.tabs.run);
    let known = self.tabs.tabs.lock().get(name).cloned();
    if let Some(target) = known
      && let Ok(tab) = browser.tab(run, Some(&target)).await
    {
      return Ok(tab);
    }
    let target = browser.open_blank_tab(run).await.map_err(|error| said(&error))?;
    if let Err(why) = self.guard(&target).await {
      if let Err(error) = browser.close_tab(run, &target).await {
        tracing::warn!(%error, "a browser driver's tab that could not be guarded was not closed");
      }
      return Err(why);
    }
    self.tabs.tabs.lock().insert(name.clone(), target.clone());
    let tab = browser.tab(run, Some(&target)).await.map_err(|error| said(&error));
    let (closing, runtime) = (Arc::clone(browser), Handle::current());
    self.services.on_end(Box::new(move || {
      runtime.spawn(async move {
        // The person may have closed it, or the run's end closed it first.
        if let Err(error) = closing.close_tab(run, &target).await {
          tracing::debug!(%error, "a browser driver's tab was not closed with its step");
        }
      });
    }));
    tab
  }

  /// Puts the origin guard in place on the tab `target`: its requests pause until the guard answers them.
  async fn guard(&self, target: &TargetId) -> Result<(), String> {
    let (browser, run) = (&self.tabs.browser, self.tabs.run);
    let tab = browser.tab(run, Some(target)).await.map_err(|error| said(&error))?;
    let follower = browser.tab(run, Some(target)).await.map_err(|error| said(&error))?;
    tokio::spawn(guard::follow(tab.events(), follower, Arc::clone(&self.tabs.guard)));
    tab
      .call("Fetch.enable", json!({}))
      .await
      .map(drop)
      .map_err(|error| format!("the tab's requests cannot be guarded: {error}"))
  }

  async fn carry_out(&self, name: &ServiceName, call: &UiCall) -> Result<Value, String> {
    match call {
      UiCall::Open(_) => self.opened(name).await,
      UiCall::Snapshot { query } => snapshot(&self.opened_tab(name).await?, query.as_deref()).await,
      UiCall::Act { target, action } => {
        act::act(&self.opened_tab(name).await?, target.as_ref(), action).await?;
        Ok(json!({"acted": action.as_str(), "ref": target}))
      }
      UiCall::Screenshot { target } => {
        let saved = act::screenshot(&self.opened_tab(name).await?, target.as_ref(), self.artifacts).await?;
        Ok(json!({"screenshot": saved}))
      }
      UiCall::Logs { lines, filter } => {
        let tab = self.opened_tab(name).await?;
        Ok(json!({"lines": self.logs(&tab, *lines, filter.as_deref())}))
      }
      UiCall::Wait { condition, timeout_ms } => wait::wait(&self.opened_tab(name).await?, condition, *timeout_ms).await,
    }
  }

  /// The tab `ui_open` opened for the service `name`, its guard allowing the origins of the services running now.
  async fn opened_tab(&self, name: &ServiceName) -> Result<Tab, String> {
    self.tabs.guard.allow(&self.services.urls());
    let target = self
      .tabs
      .tabs
      .lock()
      .get(name)
      .cloned()
      .ok_or_else(|| format!("the service `{name}` has no tab open: call ui_open(\"{name}\") again"))?;
    self
      .tabs
      .browser
      .tab(self.tabs.run, Some(&target))
      .await
      .map_err(|error| said(&error))
  }

  /// The page's console errors, its failed requests and what the guard blocked, the latest `lines` of them, only
  /// those holding `filter` when it is given. What the guard blocked is said once, by its own line, not again as a
  /// failed request or the browser's console line about it.
  fn logs(&self, tab: &Tab, lines: u32, filter: Option<&str>) -> Vec<String> {
    let mut kept: Vec<String> = {
      let log = tab.log();
      let console = log
        .console()
        .filter(|message| message.is_error() && !message.text.contains(BLOCKED_BY_GUARD))
        .map(|message| format!("console {}: {}", message.level, message.text));
      console.chain(log.requests().filter_map(failed_request)).collect()
    };
    kept.extend(self.tabs.guard.blocked());
    kept.retain(|line| filter.is_none_or(|filter| line.contains(filter)));
    kept.drain(..kept.len().saturating_sub(lines as usize));
    kept
  }
}

/// How the browser names a request the guard failed (`net::ERR_BLOCKED_BY_CLIENT`, with `.Inspector` after it in
/// Chromium's words for a request failed over CDP).
const BLOCKED_BY_GUARD: &str = "net::ERR_BLOCKED_BY_CLIENT";

/// A request that failed or was answered 4xx or 5xx, as a log line; `None` for any other, and for one the guard
/// blocked.
fn failed_request(request: &Request) -> Option<String> {
  let (method, url) = (&request.method, &request.url);
  match (&request.failed, request.status) {
    (Some(reason), _) if reason.starts_with(BLOCKED_BY_GUARD) => None,
    (Some(reason), _) => Some(format!("request {method} {url} failed: {reason}")),
    (None, Some(status)) if status >= 400 => Some(format!("request {method} {url}: {status}")),
    _ => None,
  }
}

/// The page's tree, or only the elements `query` names.
async fn snapshot(tab: &Tab, query: Option<&str>) -> Result<Value, String> {
  let tree = tab
    .call("Accessibility.getFullAXTree", json!({}))
    .await
    .map_err(|error| error.to_string())?;
  let nodes = tree["nodes"].as_array().map_or(&[][..], Vec::as_slice);
  let snapshot = Snapshot::of(nodes);
  Ok(match query {
    Some(query) => {
      let (matches, truncated) = snapshot.matches(query);
      json!({"matches": matches, "truncated": truncated})
    }
    None => {
      let (tree, truncated) = snapshot.tree();
      json!({"tree": tree, "truncated": truncated})
    }
  })
}

fn not_running(name: &ServiceName) -> String {
  format!("the service `{name}` is not running in this step: start it with start_service(\"{name}\") first")
}

/// What the agent is told of `error`: a browser that cannot be used says what to install.
fn said(error: &BrowserError) -> String {
  match error {
    BrowserError::Unavailable(unavailable) => format!(
      "{unavailable}. The browser UI driver opens web pages in Orchester's Browser: install Microsoft Edge or Google \
       Chrome, or choose another Chromium-based browser (Brave, Chromium) in Settings › Browser"
    ),
    error => error.to_string(),
  }
}
