//! The *Browser* window's requests, served from the project's browser: the screencast of the tab shown, the person's
//! input and navigation, the person's own tabs, dialogs, `<select>` lists, and the restart in a browser window of its
//! own and back (`docs/design/browser.md`, "The *Browser* window", decision 7).

use std::sync::Arc;

use orchester_ipc::{
  Event, Rejection,
  browser::{
    BrowserCommand, BrowserEvent, BrowserMode, BrowserReply, BrowserState, BrowserTabId, Navigation, PersonInput,
    SelectId, TabOwner, Viewport,
  },
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
  Shown, input,
  order::{Arrived, Turn},
  select,
};
use crate::browser::{
  BrowserError, ProjectBrowser,
  cdp::{Cdp, CdpError, SessionId, TargetId},
  running::Running,
  tabs::ClosedBy,
};

/// The JPEG quality of the screencast: sharp enough for text, about 100 KB a frame of a full page.
const SCREENCAST_QUALITY: u32 = 80;

/// One of the browser's tabs as the window reaches it, whoever's it is.
struct WindowTab {
  cdp: Arc<Cdp>,
  target: TargetId,
  session: SessionId,
  mode: BrowserMode,
}

impl ProjectBrowser {
  /// The window's `command` as it arrives, to be carried out ([`Self::serve`]): the person's input takes its place in
  /// its tab's line now, whenever it is carried out, and it and the person's navigation make the agent of their tab
  /// wait from now.
  pub fn arrived(&self, command: BrowserCommand) -> Arrived {
    let turn = match &command {
      BrowserCommand::Input { tab, .. } => {
        let target = TargetId::from(tab.as_str());
        self.window.holds.used(target.clone());
        self.window.inputs.join(target)
      }
      BrowserCommand::Navigate { tab, .. } => {
        self.window.holds.used(TargetId::from(tab.as_str()));
        Turn::default()
      }
      _ => Turn::default(),
    };
    Arrived { command, turn }
  }

  /// Carries out the window's command; `desktop` gets the browser's events once it subscribed.
  pub async fn serve(
    &self,
    arrived: Arrived,
    desktop: &mpsc::UnboundedSender<Event>,
  ) -> Result<BrowserReply, Rejection> {
    let Arrived { command, turn } = arrived;
    let done = match command {
      BrowserCommand::Subscribe => {
        self.subscribe(desktop.clone()).await;
        Ok(())
      }
      BrowserCommand::Show { tab, viewport } => self.show(&tab, viewport).await,
      BrowserCommand::StopShowing => self.stop_showing().await,
      BrowserCommand::Input { tab, input } => self.input(&tab, &input, turn).await,
      BrowserCommand::Navigate { tab, to } => self.navigate(&tab, &to).await,
      BrowserCommand::OpenTab { url } => {
        return match self.open_person_tab(url.as_deref()).await {
          Ok(tab) => Ok(BrowserReply::TabOpened { tab }),
          Err(error) => Err(rejection(&error)),
        };
      }
      BrowserCommand::CloseTab { tab } => self.close_window_tab(&tab).await,
      BrowserCommand::AnswerDialog { tab, accept, text } => self.answer_dialog(&tab, accept, text).await,
      BrowserCommand::ChooseOption { tab, select, index } => self.choose_option(&tab, select, index).await,
      BrowserCommand::OpenInWindow => self.restart(BrowserMode::Headed).await,
      BrowserCommand::BackToOrchester => self.restart(BrowserMode::Headless).await,
    };
    done.map(|()| BrowserReply::Done).map_err(|error| rejection(&error))
  }

  /// `desktop` follows the browser from now on, starting with its state now and the agents waiting for the person.
  async fn subscribe(&self, desktop: mpsc::UnboundedSender<Event>) {
    self.window.subscribe(desktop);
    if self.running.lock().await.is_some() {
      self.window.changed();
    } else {
      self.window.send(BrowserEvent::Changed(BrowserState::default()));
    }
    self.window.report_waits();
  }

  /// Screencasts `tab` at `viewport` instead of the tab shown before: the page takes the window's size and scale,
  /// and its frames are kept for the window from the first. With the browser in a window of its own, which draws its
  /// own pages, `tab` is only named as the one shown.
  async fn show(&self, tab: &BrowserTabId, viewport: Viewport) -> Result<(), BrowserError> {
    let shown = self.window_tab(tab).await?;
    let before = self.window.show(Some(Shown {
      target: shown.target.clone(),
      session: shown.session.clone(),
    }));
    if shown.mode == BrowserMode::Headed {
      self.window.changed();
      return Ok(());
    }
    if let Some(before) = before {
      stop_screencast(&shown.cdp, &before.session).await;
    }
    let started = start_screencast(&shown.cdp, &shown.session, viewport).await;
    if started.is_err() {
      self.window.show(None);
    }
    self.window.changed();
    started
  }

  async fn stop_showing(&self) -> Result<(), BrowserError> {
    let before = self.window.show(None);
    let running = self.running.lock().await;
    if let (Some(before), Some(running)) = (before, running.as_ref()) {
      stop_screencast(&running.cdp, &before.session).await;
      self.window.changed();
    }
    Ok(())
  }

  /// The person's `input` on `tab`, at its `turn`; a press on a `<select>` opens its list in the window instead.
  async fn input(&self, tab: &BrowserTabId, input: &PersonInput, mut turn: Turn) -> Result<(), BrowserError> {
    turn.come().await;
    let reached = self.window_tab(tab).await?;
    if let Some(list) = input::dispatch(&reached.cdp, &reached.session, input, turn).await? {
      self.window.send(BrowserEvent::SelectOpened { tab: tab.clone(), list });
    }
    Ok(())
  }

  async fn navigate(&self, tab: &BrowserTabId, to: &Navigation) -> Result<(), BrowserError> {
    let reached = self.window_tab(tab).await?;
    let (cdp, session) = (&reached.cdp, Some(&reached.session));
    match to {
      Navigation::Address { url } => go_to(cdp, &reached.session, url).await,
      Navigation::Reload => Ok(cdp.call(session, "Page.reload", json!({})).await.map(drop)?),
      Navigation::Back => through_history(cdp, &reached.session, -1).await,
      Navigation::Forward => through_history(cdp, &reached.session, 1).await,
    }
  }

  /// Opens a tab of the person's, which no run reaches, on `url` or blank; the browser starts first if it does not
  /// run.
  async fn open_person_tab(&self, url: Option<&str>) -> Result<BrowserTabId, BrowserError> {
    let (target, session, cdp) = {
      let running = self.connection().await?;
      let (target, session) = running.open(TabOwner::Person).await?;
      (target, session, Arc::clone(&running.cdp))
    };
    if let Some(url) = url {
      go_to(&cdp, &session, url).await?;
    }
    Ok(BrowserTabId::new(target.as_str()))
  }

  /// Closes `tab`, the person's or an agent's, which is no longer kept from now on: an agent's next call on it answers
  /// that the person closed it.
  async fn close_window_tab(&self, tab: &BrowserTabId) -> Result<(), BrowserError> {
    let target = TargetId::from(tab.as_str());
    let cdp = {
      let running = self.running.lock().await;
      let closed = running
        .as_ref()
        .filter(|running| running.tabs.lock().closed_by(&target, ClosedBy::Person));
      match closed {
        Some(running) => Arc::clone(&running.cdp),
        None => return Err(BrowserError::NoSuchTab(target)),
      }
    };
    self.window.forget(&target);
    self.window.changed();
    Ok(cdp.close_target(&target).await?)
  }

  async fn answer_dialog(&self, tab: &BrowserTabId, accept: bool, text: Option<String>) -> Result<(), BrowserError> {
    let reached = self.window_tab(tab).await?;
    let mut params = json!({ "accept": accept });
    if let Some(text) = text {
      params["promptText"] = json!(text);
    }
    reached
      .cdp
      .call(Some(&reached.session), "Page.handleJavaScriptDialog", params)
      .await?;
    Ok(())
  }

  async fn choose_option(&self, tab: &BrowserTabId, list: SelectId, index: u32) -> Result<(), BrowserError> {
    let reached = self.window_tab(tab).await?;
    select::choose(&reached.cdp, &reached.session, list, index).await
  }

  /// Starts the browser again on the same profile, headless or in a window of its own as `mode` says, and reopens
  /// its tabs by their addresses for the same owners. The tabs get new ids, the tab shown stays the one shown, and the
  /// window learns all of that at once when the tabs are back, before this answers: until then it keeps what it shows.
  /// Nothing is screencast until it asks.
  async fn restart(&self, mode: BrowserMode) -> Result<(), BrowserError> {
    let mut running = self.running.lock().await;
    self.window.restarting();
    let restarted = self.start_again(&mut running, mode).await;
    self.window.restarted(running.as_ref()).await;
    restarted
  }

  async fn start_again(&self, running: &mut Option<Running>, mode: BrowserMode) -> Result<(), BrowserError> {
    let reopen = match running.take() {
      Some(current) => {
        let reopen = reopened(&current, self.window.shown()).await;
        current.stop().await;
        reopen?
      }
      None => Vec::new(),
    };
    self.window.show(None);
    let profile = self.profile.clone()?;
    let found = self.program().await?;
    let started = running.insert(Running::start(&found, &profile, mode, &self.window).await?);
    let mut failed = None;
    for tab in reopen {
      let opened = match started.open(tab.owner).await {
        Ok((target, session)) => {
          if tab.shown {
            let session = session.clone();
            self.window.show(Some(Shown { target, session }));
          }
          if tab.url == "about:blank" {
            Ok(())
          } else {
            go_to(&started.cdp, &session, &tab.url).await
          }
        }
        Err(error) => Err(error),
      };
      if let Err(error) = opened {
        tracing::warn!(%error, "a tab was not reopened after the browser's restart");
        failed.get_or_insert(error);
      }
    }
    failed.map_or(Ok(()), Err)
  }

  /// The connection and session of `tab`, any of the browser's tabs; none while the browser does not run.
  async fn window_tab(&self, tab: &BrowserTabId) -> Result<WindowTab, BrowserError> {
    let target = TargetId::from(tab.as_str());
    let running = self.running.lock().await;
    let session = running.as_ref().and_then(|running| {
      Some((
        Arc::clone(&running.cdp),
        running.tabs.lock().get(&target)?.session.clone(),
        running.mode,
      ))
    });
    match session {
      Some((cdp, session, mode)) => Ok(WindowTab {
        cdp,
        target,
        session,
        mode,
      }),
      None => Err(BrowserError::NoSuchTab(target)),
    }
  }
}

/// A tab of a browser that restarts, to open again.
struct Reopened {
  owner: TabOwner,
  url: String,
  /// The window shows it.
  shown: bool,
}

/// Each tab of `running` with its owner and address, and whether it is the tab `shown`.
async fn reopened(running: &Running, shown: Option<Shown>) -> Result<Vec<Reopened>, BrowserError> {
  let listed = running.cdp.targets().await?;
  Ok(
    running
      .tabs
      .lock()
      .all()
      .iter()
      .map(|tab| Reopened {
        owner: tab.owner,
        url: listed
          .iter()
          .find(|info| info.id == tab.target)
          .map_or_else(|| "about:blank".to_owned(), |info| info.url.clone()),
        shown: shown.as_ref().is_some_and(|shown| shown.target == tab.target),
      })
      .collect(),
  )
}

async fn start_screencast(cdp: &Cdp, session: &SessionId, viewport: Viewport) -> Result<(), BrowserError> {
  let session = Some(session);
  cdp
    .call(
      session,
      "Emulation.setDeviceMetricsOverride",
      json!({
        "width": viewport.width(),
        "height": viewport.height(),
        "deviceScaleFactor": viewport.scale(),
        "mobile": false,
      }),
    )
    .await?;
  let device = |side: u32| u64::from(side) * u64::from(viewport.scale_percent()) / 100;
  cdp
    .call(
      session,
      "Page.startScreencast",
      json!({
        "format": "jpeg",
        "quality": SCREENCAST_QUALITY,
        "maxWidth": device(viewport.width()),
        "maxHeight": device(viewport.height()),
      }),
    )
    .await?;
  Ok(())
}

/// Stops the screencast of the tab of `session`; a tab closed meanwhile has none to stop.
async fn stop_screencast(cdp: &Cdp, session: &SessionId) {
  if let Err(error) = cdp.call(Some(session), "Page.stopScreencast", json!({})).await {
    tracing::debug!(%error, "the screencast of the tab shown before did not stop");
  }
}

/// Sends the tab of `session` to `url` without waiting for its load: the window shows it loading. An address the
/// network does not reach opens the browser's own error page in the tab, which the person sees there.
async fn go_to(cdp: &Cdp, session: &SessionId, url: &str) -> Result<(), BrowserError> {
  let navigated = cdp.call(Some(session), "Page.navigate", json!({ "url": url })).await?;
  if let Some(reason) = navigated.get("errorText").and_then(Value::as_str) {
    tracing::debug!(url, reason, "the person's address did not load");
  }
  Ok(())
}

/// Goes `step` pages back (-1) or forward (1) in the tab's history.
async fn through_history(cdp: &Cdp, session: &SessionId, step: i64) -> Result<(), BrowserError> {
  const METHOD: &str = "Page.getNavigationHistory";
  let history = cdp.call(Some(session), METHOD, json!({})).await?;
  let missing = |field| CdpError::Unexpected {
    method: METHOD.to_owned(),
    field,
  };
  let current = history
    .get("currentIndex")
    .and_then(Value::as_i64)
    .ok_or_else(|| missing("currentIndex"))?;
  let entry = usize::try_from(current + step)
    .ok()
    .and_then(|index| history.get("entries")?.as_array()?.get(index))
    .ok_or(BrowserError::NoHistory(if step < 0 { "back" } else { "forward" }))?;
  let id = entry
    .get("id")
    .and_then(Value::as_i64)
    .ok_or_else(|| missing("entries.id"))?;
  cdp
    .call(Some(session), "Page.navigateToHistoryEntry", json!({ "entryId": id }))
    .await?;
  Ok(())
}

/// What the window is told when its request could not be carried out.
fn rejection(error: &BrowserError) -> Rejection {
  match error {
    BrowserError::Unavailable(_) | BrowserError::Start(_) => Rejection::BrowserUnavailable {
      detail: error.to_string(),
    },
    BrowserError::NoSuchTab(_)
    | BrowserError::NotRunsTab(_)
    | BrowserError::ClosedByPerson(_)
    | BrowserError::ClosedInBrowser(_)
    | BrowserError::NoTab => Rejection::NoSuchTab,
    BrowserError::Cdp(_)
    | BrowserError::NotOpened { .. }
    | BrowserError::NoHistory(_)
    | BrowserError::Element(_)
    | BrowserError::StaleRef(_) => Rejection::Failed {
      detail: error.to_string(),
    },
  }
}
