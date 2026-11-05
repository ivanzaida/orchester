//! Starting the browser again on the same profile with its tabs (`docs/design/browser.md`, decision 7): headed or
//! headless at the person's request, and headless once a browser in a window of its own ended or closed that window
//! without Orchester stopping it.

use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use orchester_exec::browser::{BrowserProgram, BrowserUnavailable};
use orchester_ipc::browser::{BrowserMode, ComputerBrowser, TabOwner};
use parking_lot::Mutex;
use tokio::sync::oneshot;

use super::{Shown, Window, serve::go_to};
use crate::browser::{
  BrowserError, ProjectBrowser, RunningSlot, cdp::TargetId, chosen_or_found, running::Running, tabs::Remembered,
};

/// Which profile folder the browser starts on.
#[derive(Clone)]
pub(in crate::browser) enum Profile {
  /// A folder of its own, a test's.
  Own(PathBuf),
  /// The project's own folder among the computer's profiles, named by the project's id, or why the project cannot be
  /// read.
  OfProject(Result<String, BrowserUnavailable>),
}

/// What starts the project's browser; a copy waits for each browser in a window of its own to end
/// ([`back_when_closed`]).
#[derive(Clone)]
pub(in crate::browser) struct Starter {
  profile: Profile,
  /// The computer's Settings › Browser as the desktop passed them last; `None` until it did. Read at each start.
  pub(in crate::browser) computer: Arc<Mutex<Option<ComputerBrowser>>>,
  pub(in crate::browser) out_of_sight: bool,
  pub(in crate::browser) window: Arc<Window>,
  /// Why the browser did not start again, until one starts: the answer to a run that still names its tab.
  unavailable: Arc<Mutex<Option<BrowserUnavailable>>>,
}

impl Starter {
  pub(in crate::browser) fn new(profile: Profile, computer: Option<ComputerBrowser>, window: Arc<Window>) -> Self {
    Self {
      profile,
      computer: Arc::new(Mutex::new(computer)),
      out_of_sight: false,
      window,
      unavailable: Arc::new(Mutex::new(None)),
    }
  }

  /// Why the browser that should run did not start again; `None` when it was simply not started yet.
  pub(in crate::browser) fn unavailable(&self) -> Option<BrowserUnavailable> {
    self.unavailable.lock().clone()
  }

  /// A new browser, headless or in a window of its own as `mode` says.
  pub(in crate::browser) async fn start(&self, mode: BrowserMode) -> Result<Running, BrowserError> {
    let profile = self.profile()?;
    let found = self.program().await?;
    let started = Running::start(&found, &profile, (mode, self.out_of_sight), &self.window).await?;
    self.unavailable.lock().take();
    Ok(started)
  }

  /// The computer's Settings › Browser as the desktop passed them last; unavailable until it did.
  fn computer(&self) -> Result<ComputerBrowser, BrowserUnavailable> {
    self
      .computer
      .lock()
      .clone()
      .ok_or_else(|| BrowserUnavailable::new("the app has not passed this computer's Settings › Browser yet"))
  }

  /// The profile folder of the next start ([`ProjectBrowser::profile`]).
  pub(in crate::browser) fn profile(&self) -> Result<PathBuf, BrowserUnavailable> {
    match &self.profile {
      Profile::Own(profile) => Ok(profile.clone()),
      Profile::OfProject(project) => {
        let profiles = self.computer()?.profiles.map_err(BrowserUnavailable::new)?;
        Ok(profiles.join(project.clone()?))
      }
    }
  }

  /// The program the browser starts with: the one chosen in Settings › Browser, else the one installed
  /// ([`chosen_or_found`]).
  async fn program(&self) -> Result<BrowserProgram, BrowserError> {
    let chosen = self.computer()?.program;
    Ok(
      tokio::task::spawn_blocking(move || chosen_or_found(chosen))
        .await
        .map_err(|error| BrowserError::Start(format!("looking for the browser failed: {error}")))??,
    )
  }

  /// Starts the browser again on the same profile, headless or in a window of its own as `mode` says, and reopens
  /// its tabs by their addresses for the same owners. The tabs get new ids, the tab shown stays the one shown, and the
  /// window learns all of that at once when the tabs are back, before this answers: until then it keeps what it shows.
  /// Nothing is screencast until it asks.
  pub(in crate::browser) async fn restart(
    &self,
    running: &mut Option<Running>,
    mode: BrowserMode,
  ) -> Result<(), BrowserError> {
    self.window.restarting();
    let restarted = self.start_again(running, mode).await;
    self.window.restarted(running.as_ref()).await;
    if let (None, Err(error)) = (running.as_ref(), &restarted) {
      *self.unavailable.lock() = Some(unavailable_because(error));
    }
    restarted
  }

  async fn start_again(&self, running: &mut Option<Running>, mode: BrowserMode) -> Result<(), BrowserError> {
    let (reopen, remembered) = match running.take() {
      Some(current) => {
        let reopen = reopened(&current, self.window.shown()).await;
        let remembered = current.tabs.lock().remembered();
        current.stop().await;
        (reopen, remembered)
      }
      None => (Vec::new(), Remembered::default()),
    };
    self.window.show(None);
    let started = running.insert(self.start(mode).await?);
    let mut moved = Vec::new();
    let mut failed = None;
    for tab in reopen {
      let opened = match started.open(tab.owner).await {
        Ok((target, session)) => {
          moved.push((tab.target, target.clone()));
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
    started.tabs.lock().remember(remembered, moved);
    failed.map_or(Ok(()), Err)
  }
}

impl ProjectBrowser {
  /// [`Starter::restart`] at the person's request; a browser now in a window of its own comes back by itself when that
  /// window closes.
  pub(super) async fn restart(&self, mode: BrowserMode) -> Result<(), BrowserError> {
    let mut running = self.running.lock().await;
    let restarted = self.starter.restart(&mut running, mode).await;
    if let Some(started) = running.as_mut()
      && let Some(closed) = started.window_closes.take()
    {
      tokio::spawn(back_when_closed(
        closed,
        started.generation,
        Arc::downgrade(&self.running),
        self.starter.clone(),
      ));
    }
    restarted
  }
}

/// Waits for the browser of `generation`, in a window of its own, to end or close that window, then starts it again
/// headless as *Back to Orchester* does; nobody is waiting for an answer, so a failure is only logged, and the browser
/// is unavailable with its reason from then on. A browser no longer the one running was stopped or replaced by
/// Orchester, and only ends for the window.
async fn back_when_closed(
  closed: oneshot::Receiver<()>,
  generation: u64,
  running: Weak<RunningSlot>,
  starter: Starter,
) {
  if closed.await.is_err() {
    return;
  }
  let Some(running) = running.upgrade() else {
    return;
  };
  let mut running = running.lock().await;
  if running.as_ref().is_none_or(|current| current.generation != generation) {
    starter.window.ended(generation);
    return;
  }
  tracing::info!("the browser's own window closed; starting it again in the background");
  if let Err(error) = starter.restart(&mut running, BrowserMode::Headless).await {
    tracing::warn!(%error, "the browser did not come back from its own window");
  }
}

fn unavailable_because(error: &BrowserError) -> BrowserUnavailable {
  match error {
    BrowserError::Unavailable(unavailable) => unavailable.clone(),
    other => BrowserUnavailable::new(other.to_string()),
  }
}

/// A tab of a browser that restarts, to open again.
struct Reopened {
  target: TargetId,
  owner: TabOwner,
  url: String,
  /// The window shows it.
  shown: bool,
}

/// Each tab of `running` with its owner and address, and whether it is the tab `shown`. A browser that ended or closed
/// its window lists nothing any more: its tabs go by the addresses it last reported.
async fn reopened(running: &Running, shown: Option<Shown>) -> Vec<Reopened> {
  let listed = running.cdp.targets().await.unwrap_or_else(|error| {
    tracing::debug!(%error, "the browser's tabs could not be listed before its restart");
    Vec::new()
  });
  running
    .tabs
    .lock()
    .all()
    .iter()
    .map(|tab| Reopened {
      target: tab.target.clone(),
      owner: tab.owner,
      url: listed
        .iter()
        .find(|info| info.id == tab.target)
        .map_or_else(|| tab.url.clone(), |info| info.url.clone()),
      shown: shown.as_ref().is_some_and(|shown| shown.target == tab.target),
    })
    .collect()
}
