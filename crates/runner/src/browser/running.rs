//! A started browser: its process, its one CDP connection and its tabs, the runs' and the person's; how it starts,
//! opens a tab, loads a page and stops.

use std::{fs, path::Path, sync::Arc, time::Duration};

use orchester_exec::browser::{self as program, BrowserProgram, DevToolsActivePort};
use orchester_ipc::browser::{BrowserMode, TabOwner};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{task::JoinHandle, time::Instant};

use super::{
  BrowserError,
  cdp::{self, Cdp, CdpError, SessionId, TargetId},
  log,
  process::BrowserProcess,
  tab::TabState,
  tabs::Tabs,
  window::{self, Window},
};
use crate::supervisor::ProcessExit;

/// How long the browser may take from its start to listening on its debugging port.
const START_LIMIT: Duration = Duration::from_secs(30);
/// How often the start looks for the debugging port's file.
const PORT_POLL: Duration = Duration::from_millis(50);
/// How long a page may take to load.
pub const LOAD_LIMIT: Duration = Duration::from_secs(30);
/// The permissions a page may ask for that are denied, by their names in the Permissions API.
const DENIED_PERMISSIONS: [&str; 4] = ["camera", "microphone", "geolocation", "notifications"];
/// How long `Browser.close` may take to be answered, and the browser to exit after it.
const CLOSE_LIMIT: Duration = Duration::from_secs(5);
const EXIT_GRACE: Duration = Duration::from_secs(5);

pub(super) struct Running {
  process: BrowserProcess,
  pub(super) cdp: Arc<Cdp>,
  pub(super) tabs: Arc<Mutex<Tabs>>,
  window: Arc<Window>,
  /// Sends the window the browser's state while it runs ([`window::publish`]).
  _publisher: Publisher,
}

/// Ends the publisher of a browser's state with the browser.
struct Publisher(JoinHandle<()>);

impl Drop for Publisher {
  fn drop(&mut self) {
    self.0.abort();
  }
}

impl Running {
  /// Starts `found` on `profile`, headless or in a window of its own as `mode` says, and connects to it. A browser
  /// that does not get as far as the connection is ended.
  pub(super) async fn start(
    found: &BrowserProgram,
    profile: &Path,
    mode: BrowserMode,
    window: &Arc<Window>,
  ) -> Result<Self, BrowserError> {
    fs::create_dir_all(profile).map_err(|error| {
      BrowserError::Start(format!(
        "the profile folder {} cannot be made: {error}",
        profile.display()
      ))
    })?;
    DevToolsActivePort::remove(profile)
      .map_err(|error| BrowserError::Start(format!("an earlier browser's debugging port file stays: {error}")))?;
    let command = match mode {
      BrowserMode::Headless => program::headless(found, profile)?,
      BrowserMode::Headed => program::headed(found, profile)?,
    };
    let process = BrowserProcess::start(&command)?;
    tracing::info!(browser = found.name(), ?mode, "the project's browser started");
    let port = devtools_port(profile, &process).await?;
    let cdp = Arc::new(Cdp::connect(&port.websocket_url()).await?);
    let tabs = Arc::new(Mutex::new(Tabs::new(&cdp)));
    let targets = cdp.subscribe(None);
    cdp
      .call(None, "Target.setDiscoverTargets", json!({ "discover": true }))
      .await?;
    block(&cdp).await?;
    let generation = window.started();
    tokio::spawn(window::follow_targets(
      targets,
      Arc::clone(&tabs),
      Arc::clone(window),
      generation,
    ));
    let publisher = Publisher(tokio::spawn(window::publish(
      Arc::clone(&cdp),
      Arc::clone(&tabs),
      Arc::clone(window),
      mode,
    )));
    window.changed();
    Ok(Self {
      process,
      cdp,
      tabs,
      window: Arc::clone(window),
      _publisher: publisher,
    })
  }

  /// Whether the browser still runs and its connection is open.
  pub(super) fn is_alive(&self) -> bool {
    self.process.exit().is_none() && !self.cdp.is_closed()
  }

  /// Opens a blank tab of `owner`'s and attaches to it; its events are followed for the window from the start, and a
  /// run's tab's console messages, requests and dialogs are recorded for its tools ([`log::follow`]). A tab that could
  /// not be attached is closed again.
  pub(super) async fn open(&self, owner: TabOwner) -> Result<(TargetId, SessionId), BrowserError> {
    let target = self.cdp.create_tab("about:blank").await?;
    let session = match self.cdp.attach(&target).await {
      Ok(session) => session,
      Err(error) => {
        if let Err(closing) = self.cdp.close_target(&target).await {
          tracing::warn!(%closing, "a tab that could not be attached was not closed");
        }
        return Err(error.into());
      }
    };
    let state = Arc::new(TabState::default());
    let window_follower = tokio::spawn(window::follow_tab(
      Arc::clone(&self.cdp),
      window::Followed {
        target: target.clone(),
        session: session.clone(),
        owner,
      },
      self.cdp.subscribe(Some(session.clone())),
      Arc::clone(&self.tabs),
      Arc::clone(&self.window),
    ));
    {
      let mut tabs = self.tabs.lock();
      if matches!(owner, TabOwner::Run { .. }) {
        tokio::spawn(log::follow(
          self.cdp.subscribe(Some(session.clone())),
          Arc::downgrade(&self.cdp),
          session.clone(),
          Arc::clone(&state),
          tabs.follower(),
        ));
      }
      // The tab is its owner's from the moment it is attached: a failure below leaves it to them.
      tabs.add(owner, (target.clone(), session.clone()), state, window_follower);
    }
    self.window.changed();
    self.cdp.call(Some(&session), "Page.enable", json!({})).await?;
    if matches!(owner, TabOwner::Run { .. }) {
      for domain in ["Runtime.enable", "Network.enable", "Log.enable"] {
        self.cdp.call(Some(&session), domain, json!({})).await?;
      }
    }
    Ok((target, session))
  }

  pub(super) async fn stop(self) {
    match self
      .cdp
      .call_within(None, "Browser.close", json!({}), CLOSE_LIMIT)
      .await
    {
      // The browser may close the connection before it answers.
      Ok(_) | Err(CdpError::Closed) => {}
      Err(error) => tracing::warn!(%error, "the browser did not close when asked"),
    }
    drop((self._publisher, self.cdp));
    match self.process.end(EXIT_GRACE).await {
      ProcessExit::Exited { .. } => tracing::info!("the project's browser stopped"),
      exit => tracing::warn!(?exit, "the project's browser was ended"),
    }
  }
}

/// Blocks what the agents' browser never does (`docs/design/browser.md`, "Permissions and policy"): downloads, and
/// the permissions a page may prompt for, denied on every site.
async fn block(cdp: &Cdp) -> Result<(), CdpError> {
  cdp
    .call(None, "Browser.setDownloadBehavior", json!({ "behavior": "deny" }))
    .await?;
  for permission in DENIED_PERMISSIONS {
    cdp
      .call(
        None,
        "Browser.setPermission",
        json!({ "permission": { "name": permission }, "setting": "denied" }),
      )
      .await?;
  }
  Ok(())
}

/// Navigates the tab of `session` to `url` and waits for that navigation's load.
pub(super) async fn load(cdp: &Cdp, session: &SessionId, url: &str) -> Result<(), BrowserError> {
  let mut events = cdp.subscribe(Some(session.clone()));
  cdp.call(Some(session), "Page.enable", json!({})).await?;
  cdp
    .call(
      Some(session),
      "Page.setLifecycleEventsEnabled",
      json!({ "enabled": true }),
    )
    .await?;
  let navigated = cdp.call(Some(session), "Page.navigate", json!({ "url": url })).await?;
  if let Some(reason) = navigated.get("errorText").and_then(Value::as_str) {
    return Err(BrowserError::NotOpened {
      url: url.to_owned(),
      reason: reason.to_owned(),
    });
  }
  // A navigation that only moves within its document starts no loader and has nothing more to wait for.
  let Some(loader) = navigated.get("loaderId").and_then(Value::as_str) else {
    return Ok(());
  };
  cdp::wait_for_event(
    &mut events,
    "Page.lifecycleEvent",
    |event| event["name"] == "load" && event["loaderId"] == loader,
    LOAD_LIMIT,
  )
  .await?;
  Ok(())
}

/// The debugging address the browser writes into `profile` once it listens, within [`START_LIMIT`]; refused at once
/// when it exits first.
async fn devtools_port(profile: &Path, process: &BrowserProcess) -> Result<DevToolsActivePort, BrowserError> {
  let deadline = Instant::now() + START_LIMIT;
  loop {
    let written = DevToolsActivePort::read(profile)
      .map_err(|error| BrowserError::Start(format!("its debugging port file cannot be read: {error}")))?;
    if let Some(port) = written {
      return Ok(port);
    }
    if let Some(exit) = process.exit() {
      return Err(BrowserError::Start(format!(
        "it exited ({exit:?}) before it listened on its debugging port; another browser may hold its profile folder"
      )));
    }
    if Instant::now() >= deadline {
      return Err(BrowserError::Start(format!(
        "it did not listen on its debugging port within {START_LIMIT:?}"
      )));
    }
    tokio::time::sleep(PORT_POLL).await;
  }
}
