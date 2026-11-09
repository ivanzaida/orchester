//! An app's own Chromium reached through the remote debugging port it listens on, for the `cdp` UI driver
//! (`docs/architecture/agent-testing.md`, "UI drivers"): an Electron app started with `--remote-debugging-port`, a
//! WebView2 app given it through `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`. One connection per app, its browser's
//! address read from the port's `/json/version`; each of its windows is a page target, attached once and reached as a
//! [`Tab`] whose console messages and requests are recorded as a run's tab's are ([`log::follow`]). It is the app's
//! browser, not the project's: nothing in it is blocked or denied, and Orchester opens and closes none of its pages.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{
  BrowserError,
  cdp::{Cdp, TargetId, TargetInfo},
  log,
  tab::{Tab, TabState},
};

/// How long the debugging port may take to say where the app's browser listens.
const VERSION_LIMIT: Duration = Duration::from_secs(5);
/// The path of a browser's own WebSocket on its debugging port.
const BROWSER_PATH: &str = "/devtools/browser/";

/// The connection to an app's browser on its debugging port, and the windows attached so far.
pub struct AppBrowser {
  cdp: Arc<Cdp>,
  pages: Mutex<HashMap<TargetId, Tab>>,
}

impl AppBrowser {
  /// Connects to the browser listening on `port` of the loopback interface.
  pub async fn connect(port: u16) -> Result<Self, BrowserError> {
    let address = websocket_address(port).await?;
    Ok(Self {
      cdp: Arc::new(Cdp::connect(&address).await?),
      pages: Mutex::default(),
    })
  }

  /// Whether the connection ended: the app closed it or exited.
  pub fn is_closed(&self) -> bool {
    self.cdp.is_closed()
  }

  /// The app's windows: its page targets, in the order its browser lists them.
  pub async fn pages(&self) -> Result<Vec<TargetInfo>, BrowserError> {
    let mut listed = self.cdp.targets().await?;
    listed.retain(|target| target.kind == "page");
    Ok(listed)
  }

  /// The window `target`, attached at its first use: from then on its console messages and requests are recorded.
  pub async fn tab(&self, target: &TargetId) -> Result<Tab, BrowserError> {
    if let Some(tab) = self.pages.lock().get(target) {
      return Ok(tab.clone());
    }
    let session = self.cdp.attach(target).await?;
    let state = Arc::new(TabState::default());
    tokio::spawn(log::follow(
      self.cdp.subscribe(Some(session.clone())),
      Arc::downgrade(&self.cdp),
      session.clone(),
      Arc::clone(&state),
      Arc::new(()),
    ));
    for domain in ["Page.enable", "Runtime.enable", "Network.enable", "Log.enable"] {
      self.cdp.call(Some(&session), domain, json!({})).await?;
    }
    let tab = Tab::new(target.clone(), Arc::clone(&self.cdp), session, state);
    Ok(self.pages.lock().entry(target.clone()).or_insert(tab).clone())
  }
}

/// The WebSocket address of the browser on `port`, as its `/json/version` names it, kept on `127.0.0.1:<port>`
/// whatever host the answer names.
async fn websocket_address(port: u16) -> Result<String, BrowserError> {
  let unreadable = |why: String| BrowserError::AppPort { port, why };
  // Reqwest's no-provider feature requires the same ring provider as the model clients.
  // Another client may already have installed one; losing that race is harmless.
  let _ = rustls::crypto::ring::default_provider().install_default();
  let client = reqwest::Client::builder()
    .timeout(VERSION_LIMIT)
    .redirect(reqwest::redirect::Policy::none())
    .no_proxy()
    .build()
    .map_err(|error| unreadable(error.to_string()))?;
  let answer = client
    .get(format!("http://127.0.0.1:{port}/json/version"))
    .send()
    .await
    .and_then(reqwest::Response::error_for_status)
    .map_err(|error| unreadable(error.to_string()))?
    .text()
    .await
    .map_err(|error| unreadable(error.to_string()))?;
  let version: Value = serde_json::from_str(&answer).map_err(|error| unreadable(error.to_string()))?;
  let named = version["webSocketDebuggerUrl"]
    .as_str()
    .ok_or_else(|| unreadable("its /json/version names no webSocketDebuggerUrl".to_owned()))?;
  let path = named
    .find(BROWSER_PATH)
    .map(|at| &named[at..])
    .filter(|path| !path.contains(char::is_whitespace))
    .ok_or_else(|| unreadable(format!("{named} is not a browser's address")))?;
  Ok(format!("ws://127.0.0.1:{port}{path}"))
}
