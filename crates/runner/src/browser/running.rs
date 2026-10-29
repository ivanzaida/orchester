//! A started browser: its process, its one CDP connection and its tabs; how it starts, opens a tab, loads a page and
//! stops.

use std::{fs, path::Path, sync::Arc, time::Duration};

use orchester_exec::browser::{self as program, DevToolsActivePort};
use orchester_ipc::browser::{BrowserMode, TabOwner};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{task::JoinHandle, time::Instant};

use super::{
  BrowserError,
  cdp::{self, Cdp, CdpError, SessionId, TargetId},
  process::BrowserProcess,
  tabs::Tabs,
  window::{self, Window},
};
use crate::supervisor::ProcessExit;

/// How long the browser may take from its start to listening on its debugging port.
const START_LIMIT: Duration = Duration::from_secs(30);
/// How often the start looks for the debugging port's file.
const PORT_POLL: Duration = Duration::from_millis(50);
/// How long a page may take to load.
const LOAD_LIMIT: Duration = Duration::from_secs(30);
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
  /// Finds the browser, starts it on `profile`, headless or in a window of its own as `mode` says, and connects to
  /// it. A browser that does not get as far as the connection is ended.
  pub(super) async fn start(profile: &Path, mode: BrowserMode, window: &Arc<Window>) -> Result<Self, BrowserError> {
    let found = tokio::task::spawn_blocking(program::find)
      .await
      .map_err(|error| BrowserError::Start(format!("looking for the browser failed: {error}")))??;
    fs::create_dir_all(profile).map_err(|error| {
      BrowserError::Start(format!(
        "the profile folder {} cannot be made: {error}",
        profile.display()
      ))
    })?;
    DevToolsActivePort::remove(profile)
      .map_err(|error| BrowserError::Start(format!("an earlier browser's debugging port file stays: {error}")))?;
    let command = match mode {
      BrowserMode::Headless => program::headless(&found, profile)?,
      BrowserMode::Headed => program::headed(&found, profile)?,
    };
    let process = BrowserProcess::start(&command)?;
    tracing::info!(browser = found.name(), ?mode, "the project's browser started");
    let port = devtools_port(profile, &process).await?;
    let cdp = Arc::new(Cdp::connect(&port.websocket_url()).await?);
    let tabs = Arc::new(Mutex::new(Tabs::default()));
    let targets = cdp.subscribe(None);
    cdp
      .call(None, "Target.setDiscoverTargets", json!({ "discover": true }))
      .await?;
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

  /// Opens a blank tab of `owner`'s and attaches to it; its events are followed for the window from the start. A tab
  /// that could not be attached is closed again.
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
    let events = self.cdp.subscribe(Some(session.clone()));
    let follower = tokio::spawn(window::follow_tab(
      Arc::clone(&self.cdp),
      window::Followed {
        target: target.clone(),
        session: session.clone(),
        owner,
      },
      events,
      Arc::clone(&self.tabs),
      Arc::clone(&self.window),
    ));
    // The tab is its owner's from the moment it is attached: a failure below leaves it to them.
    self.tabs.lock().add(owner, target.clone(), session.clone(), follower);
    self.window.changed();
    self.cdp.call(Some(&session), "Page.enable", json!({})).await?;
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
