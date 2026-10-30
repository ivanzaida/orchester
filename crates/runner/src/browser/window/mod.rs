//! What the *Browser* window sees of the project's browser (`docs/design/browser.md`, "The *Browser* window"): the
//! desktop that subscribed, the tab it shows, and that tab's latest screencast frame, which waits alone to be written:
//! a newer frame replaces it, so frames are dropped rather than queued when the desktop reads slower than the page
//! changes. Each tab's events are followed for it ([`follow`]), the person's inputs keep their order ([`order`]) and
//! hold the agent of their tab ([`hold`]), and the browser's state is sent whenever it changes.

mod follow;
mod hold;
mod input;
mod order;
mod restart;
mod select;
mod serve;

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::Duration,
};

use orchester_ipc::{
  Event,
  browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, ScreencastFrame},
};
use parking_lot::Mutex;
use tokio::{
  sync::{Notify, mpsc},
  time::MissedTickBehavior,
};

pub(super) use self::{
  follow::{Followed, WindowWatch, follow_tab, follow_targets, follow_window_targets},
  restart::Starter,
};
use self::{hold::Holds, order::InputLines};
pub use self::{
  hold::{HoldClock, PERSON_HOLD, PersonWait, WAIT_CAP},
  order::Arrived,
};
use super::{
  cdp::{Cdp, SessionId, TargetId},
  running::Running,
  tabs::Tabs,
};

/// How often the tabs' titles and addresses are read again while a desktop follows the browser.
const TITLE_POLL: Duration = Duration::from_secs(1);

/// The window's side of the project's browser; one for the runner's whole life, across the browser's starts.
#[derive(Default)]
pub(super) struct Window {
  desktop: Mutex<Option<mpsc::UnboundedSender<Event>>>,
  shown: Mutex<Option<Shown>>,
  frames: FrameSlot,
  /// Numbers the frames of every screencast, so a gap shows frames dropped.
  sequence: AtomicU64,
  /// Counts the browser's starts: a browser that ended on its own tells the window only while it is the latest.
  generation: AtomicU64,
  /// The browser restarts: its state is not sent until its tabs are back.
  restarting: AtomicBool,
  /// Wakes the publisher of the running browser's state.
  changed: Notify,
  inputs: InputLines,
  holds: Holds,
}

/// The tab the window shows: screencast now, or with the browser in a window of its own only named, since that window
/// draws its own pages.
#[derive(Clone)]
pub(super) struct Shown {
  pub(super) target: TargetId,
  pub(super) session: SessionId,
}

impl Window {
  /// From now on `desktop` gets the browser's events; it replaces any desktop before.
  pub(super) fn subscribe(&self, desktop: mpsc::UnboundedSender<Event>) {
    *self.desktop.lock() = Some(desktop);
  }

  fn subscribed(&self) -> bool {
    self.desktop.lock().is_some()
  }

  /// Sends `event` to the subscribed desktop, if one is.
  pub(super) fn send(&self, event: BrowserEvent) {
    let desktop = self.desktop.lock();
    if let Some(desktop) = desktop.as_ref()
      && desktop.send(Event::Browser(event)).is_err()
    {
      tracing::debug!("the desktop left before the browser's event");
    }
  }

  /// The browser's state changed: the publisher sends it.
  pub(super) fn changed(&self) {
    self.changed.notify_one();
  }

  pub(super) fn shown(&self) -> Option<Shown> {
    self.shown.lock().clone()
  }

  /// Shows `shown` instead of the tab shown before, which this returns; a frame of that tab still waiting is dropped.
  pub(super) fn show(&self, shown: Option<Shown>) -> Option<Shown> {
    let before = std::mem::replace(&mut *self.shown.lock(), shown);
    self.frames.clear();
    before
  }

  /// `target` is gone: the window shows nothing if it showed it.
  pub(super) fn forget(&self, target: &TargetId) {
    self.inputs.forget(target);
    self.holds.forget(target);
    let mut shown = self.shown.lock();
    if shown.as_ref().is_some_and(|shown| &shown.target == target) {
      *shown = None;
      self.frames.clear();
    }
  }

  fn shows(&self, target: &TargetId) -> bool {
    self.shown.lock().as_ref().is_some_and(|shown| &shown.target == target)
  }

  /// A frame of `target`'s screencast, `css` its page area in CSS pixels: kept for the desktop while `target` is the
  /// tab shown, replacing a frame still waiting.
  fn frame(&self, target: &TargetId, css: (u32, u32), jpeg: Vec<u8>) {
    if !self.shows(target) {
      return;
    }
    self.frames.put(ScreencastFrame {
      tab: tab_id(target),
      sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
      css_width: css.0,
      css_height: css.1,
      jpeg,
    });
  }

  /// The latest frame of the tab shown, once there is one.
  pub(super) async fn next_frame(&self) -> ScreencastFrame {
    self.frames.next().await
  }

  /// How many frames a newer one replaced before they were written.
  pub(super) fn dropped_frames(&self) -> u64 {
    self.frames.dropped.load(Ordering::Relaxed)
  }

  /// A browser starts: the number it ends with ([`Window::ended`]).
  pub(super) fn started(&self) -> u64 {
    self.inputs.clear();
    self.holds.clear();
    self.generation.fetch_add(1, Ordering::AcqRel) + 1
  }

  /// The browser of `generation` ended: while no other started since, the window shows nothing and learns that the
  /// browser has no tabs.
  pub(super) fn ended(&self, generation: u64) {
    if self.generation.load(Ordering::Acquire) != generation {
      return;
    }
    self.no_browser();
  }

  /// The browser restarts: until [`Self::restarted`] the window hears nothing of it, neither of the one stopping nor of
  /// the one starting, and keeps what it shows.
  pub(super) fn restarting(&self) {
    self.restarting.store(true, Ordering::Release);
    self.started();
  }

  /// The restart is over: the window learns the state of the browser `running`, or that none runs. Sent here, before
  /// the restart's reply on the same channel, so the window never has the reply with the old tabs.
  pub(super) async fn restarted(&self, running: Option<&Running>) {
    match running {
      Some(running) => {
        let state = state(&running.cdp, &running.tabs, self, running.mode).await;
        self.send(BrowserEvent::Changed(state));
      }
      None => self.no_browser(),
    }
    self.restarting.store(false, Ordering::Release);
  }

  fn is_restarting(&self) -> bool {
    self.restarting.load(Ordering::Acquire)
  }

  fn no_browser(&self) {
    self.holds.clear();
    self.show(None);
    self.send(BrowserEvent::Changed(BrowserState::default()));
  }
}

/// The one frame waiting to be written.
#[derive(Default)]
struct FrameSlot {
  latest: Mutex<Option<ScreencastFrame>>,
  ready: Notify,
  dropped: AtomicU64,
}

impl FrameSlot {
  fn put(&self, frame: ScreencastFrame) {
    if self.latest.lock().replace(frame).is_some() {
      self.dropped.fetch_add(1, Ordering::Relaxed);
    }
    self.ready.notify_one();
  }

  fn clear(&self) {
    self.latest.lock().take();
  }

  async fn next(&self) -> ScreencastFrame {
    loop {
      if let Some(frame) = self.latest.lock().take() {
        return frame;
      }
      // A frame put after the look above leaves a permit, so this returns at once.
      self.ready.notified().await;
    }
  }
}

/// Sends the running browser's state each time it changes, while a desktop is subscribed, until it is aborted with
/// the browser. The browser reports no title a page sets by script (`Target.targetInfoChanged` comes with navigations
/// only), so the tabs are also read again every [`TITLE_POLL`], and their state is sent when it differs.
pub(super) async fn publish(cdp: Arc<Cdp>, tabs: Arc<Mutex<Tabs>>, window: Arc<Window>, mode: BrowserMode) {
  let mut poll = tokio::time::interval(TITLE_POLL);
  poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
  let mut sent: Option<BrowserState> = None;
  loop {
    let told = tokio::select! {
      () = window.changed.notified() => true,
      _ = poll.tick() => false,
    };
    if !window.subscribed() || window.is_restarting() {
      continue;
    }
    let state = state(&cdp, &tabs, &window, mode).await;
    // The restart may have begun while the tabs were read.
    if window.is_restarting() {
      continue;
    }
    if told || sent.as_ref() != Some(&state) {
      window.send(BrowserEvent::Changed(state.clone()));
      sent = Some(state);
    }
  }
}

/// The browser's state now: its tabs with their titles and addresses as the browser lists them.
async fn state(cdp: &Cdp, tabs: &Mutex<Tabs>, window: &Window, mode: BrowserMode) -> BrowserState {
  // Unlisted, the tabs keep the titles and addresses last reported; the next change sends them again.
  let listed = cdp.targets().await.unwrap_or_else(|error| {
    tracing::warn!(%error, "the browser's tabs could not be listed for the window");
    Vec::new()
  });
  let tabs = tabs
    .lock()
    .all()
    .iter()
    .map(|tab| {
      let info = listed.iter().find(|info| info.id == tab.target);
      BrowserTab {
        id: tab_id(&tab.target),
        owner: tab.owner,
        title: info.map_or_else(|| tab.title.clone(), |info| info.title.clone()),
        url: info.map_or_else(|| tab.url.clone(), |info| info.url.clone()),
        loading: tab.loading,
      }
    })
    .collect();
  BrowserState {
    tabs,
    shown: window.shown().map(|shown| tab_id(&shown.target)),
    mode,
  }
}

fn tab_id(target: &TargetId) -> BrowserTabId {
  BrowserTabId::new(target.as_str())
}
