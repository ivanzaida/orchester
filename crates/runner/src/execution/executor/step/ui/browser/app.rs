//! The `cdp` UI driver (`docs/architecture/agent-testing.md`, "UI drivers"): an Electron or WebView2 app a service
//! started, reached over CDP through the remote debugging port it listens on — the service's own port
//! (`orchester_core::service::cdp`) — with the browser driver's page tools: the snapshot, the actions by ref, `ui_wait`
//! and the screenshot of the window's web content. Before `ui_open` attaches, the process listening on that port must
//! be one of the service's own tree, the program the run started or a process it started ([`RunTree::holds`]); any
//! other is refused, and with it every target of its browser. `ui_logs` is the window's console. On macOS every call
//! answers that the native driver is the one to use: WKWebView has no debugging port.

use std::{collections::BTreeMap, num::NonZeroU32, path::Path, sync::Arc};

use orchester_core::{
  connection::invocation::Platform,
  orchester_tools::UiCall,
  service::{ServiceName, cdp::cdp_supported},
};
use orchester_exec::{listener, run_processes::RunProcesses};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::{super::super::service::StepServices, act, not_running, snapshot, wait};
use crate::browser::{Tab, app::AppBrowser, cdp::TargetId};

/// The apps the `cdp` driver attached to in one invocation, one per service `ui_open` opened.
#[derive(Default)]
pub(in crate::execution::executor::step) struct AppTabs {
  apps: Mutex<BTreeMap<ServiceName, Attached>>,
}

/// An app's connection and the window `ui_open` chose.
struct Attached {
  app: Arc<AppBrowser>,
  window: TargetId,
}

/// What the driver's calls of one invocation reach.
pub(in crate::execution::executor::step::ui) struct AppUi<'a> {
  pub(in crate::execution::executor::step::ui) apps: &'a AppTabs,
  pub(in crate::execution::executor::step::ui) processes: &'a RunProcesses,
  pub(in crate::execution::executor::step::ui) services: &'a StepServices,
  /// The run's own storage, where screenshots go.
  pub(in crate::execution::executor::step::ui) artifacts: &'a Path,
}

impl AppUi<'_> {
  /// `ui_open(name)`: the app's first window, attached through its debugging port once its listener is checked.
  pub(in crate::execution::executor::step::ui) fn open(&self, name: &ServiceName) -> Result<Value, String> {
    cdp_supported(Platform::current(), name).map_err(|error| error.to_string())?;
    blocking(self.opened(name))
  }

  /// A call on the service `name`, which `ui_open` opened.
  pub(in crate::execution::executor::step::ui) fn run(
    &self,
    name: &ServiceName,
    call: &UiCall,
  ) -> Result<Value, String> {
    cdp_supported(Platform::current(), name).map_err(|error| error.to_string())?;
    blocking(self.carry_out(name, call))
  }

  async fn opened(&self, name: &ServiceName) -> Result<Value, String> {
    let port = self.services.port(name).ok_or_else(|| not_running(name))?;
    let owner = self.listener(name, port)?;
    let app = Arc::new(AppBrowser::connect(port).await.map_err(|error| error.to_string())?);
    let windows = app.pages().await.map_err(|error| error.to_string())?;
    let window = windows
      .iter()
      .find(|window| !window.url.starts_with("devtools://"))
      .ok_or_else(|| format!("the service `{name}` shows no window yet: call ui_open again once it shows one"))?;
    app.tab(&window.id).await.map_err(|error| error.to_string())?;
    let answer = json!({
      "service": name.as_str(),
      "driver": "cdp",
      "pid": owner,
      "port": port,
      "window": window.id.as_str(),
      "url": window.url,
      "title": window.title,
      "windows": windows
        .iter()
        .map(|window| json!({"window": window.id.as_str(), "url": window.url, "title": window.title}))
        .collect::<Vec<_>>(),
    });
    let window = window.id.clone();
    self.apps.apps.lock().insert(name.clone(), Attached { app, window });
    Ok(answer)
  }

  /// The pid listening on `port`, the debugging port of the service `name`, once it is a process of the service's own
  /// tree; refused for any other.
  fn listener(&self, name: &ServiceName, port: u16) -> Result<u32, String> {
    let root = self
      .services
      .pid(name)
      .and_then(NonZeroU32::new)
      .ok_or_else(|| not_running(name))?;
    let owner = listener::loopback_owner(port)
      .map_err(|error| format!("the debugging port {port} of the service `{name}` cannot be looked at: {error}"))?
      .ok_or_else(|| {
        format!(
          "nothing listens on the debugging port {port} of the service `{name}` yet: an Electron app's arguments \
           include --remote-debugging-port={{{{port}}}}, a WebView2 app gets it from Orchester; call ui_open again \
           once it listens"
        )
      })?;
    let tree = self
      .processes
      .tree(root)
      .map_err(|error| format!("the processes of the service `{name}` cannot be read: {error}"))?;
    if !tree.holds(owner) {
      return Err(format!(
        "refused: pid {owner} listens on the debugging port {port} of the service `{name}`, and it is not a process \
         of the service's own tree, so none of its targets is reached"
      ));
    }
    Ok(owner)
  }

  async fn carry_out(&self, name: &ServiceName, call: &UiCall) -> Result<Value, String> {
    match call {
      UiCall::Open(_) => self.opened(name).await,
      UiCall::Snapshot { query } => snapshot(&self.window(name).await?, query.as_deref()).await,
      UiCall::Act { target, action } => {
        act::act(&self.window(name).await?, target.as_ref(), action).await?;
        Ok(json!({"acted": action.as_str(), "ref": target}))
      }
      UiCall::Screenshot { target } => {
        let saved = act::screenshot(&self.window(name).await?, target.as_ref(), self.artifacts).await?;
        Ok(json!({"screenshot": saved}))
      }
      UiCall::Logs { lines, filter } => {
        let tab = self.window(name).await?;
        Ok(json!({"lines": console(&tab, *lines, filter.as_deref())}))
      }
      UiCall::Wait { condition, timeout_ms } => wait::wait(&self.window(name).await?, condition, *timeout_ms).await,
    }
  }

  /// The window `ui_open` chose for the service `name`, while its app's connection is open.
  async fn window(&self, name: &ServiceName) -> Result<Tab, String> {
    let (app, window) = {
      let apps = self.apps.apps.lock();
      let attached = apps
        .get(name)
        .ok_or_else(|| format!("the service `{name}` has no window open: call ui_open(\"{name}\") again"))?;
      (Arc::clone(&attached.app), attached.window.clone())
    };
    if app.is_closed() {
      return Err(format!(
        "the connection to the service `{name}` closed: the app exited or closed its debugging port; call \
         ui_open(\"{name}\") again"
      ));
    }
    app.tab(&window).await.map_err(|error| error.to_string())
  }
}

/// What `work` answers, carried out on the runtime this blocking call runs beside.
fn blocking(work: impl Future<Output = Result<Value, String>>) -> Result<Value, String> {
  let runtime = Handle::try_current().map_err(|error| format!("the app cannot be reached: {error}"))?;
  runtime.block_on(work)
}

/// The window's console messages, the latest `lines` of them, only those holding `filter` when it is given.
fn console(tab: &Tab, lines: u32, filter: Option<&str>) -> Vec<String> {
  let mut kept: Vec<String> = tab
    .log()
    .console()
    .map(|message| format!("console {}: {}", message.level, message.text))
    .filter(|line| filter.is_none_or(|filter| line.contains(filter)))
    .collect();
  kept.drain(..kept.len().saturating_sub(lines as usize));
  kept
}
