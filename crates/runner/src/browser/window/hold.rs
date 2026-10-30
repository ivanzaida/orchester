//! The person's hold on a tab (`docs/design/browser.md`, "What the agent gets", decision 16): the person's input and
//! navigation in the window start it, and an agent's call that acts on that tab waits until [`PERSON_HOLD`] after the
//! last of them, at most [`WAIT_CAP`]. A waiting call ends at once when its run stops, its tab closes or the browser
//! restarts, and the window follows each wait by [`BrowserEvent::AgentWaiting`] and [`BrowserEvent::AgentWaitEnded`].
//!
//! The time comes from a [`HoldClock`], so a test drives it instead of sleeping. A wait that is dropped, as the relay
//! of a worker that ended is, tells the window that it was cancelled.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
  time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::browser::{AgentWaitEnd, BrowserEvent};
use parking_lot::Mutex;
use tokio::sync::watch;

use super::{Window, tab_id};
use crate::browser::cdp::TargetId;

/// How long after the person's last input an agent's call on the tab waits.
pub const PERSON_HOLD: Duration = Duration::from_secs(10);
/// How long a call waits at most while the person keeps using the tab (`docs/design/browser.md`, decision 16).
pub const WAIT_CAP: Duration = Duration::from_secs(60);
/// How far the moment a wait ends must move before the window is told: the input of a moving pointer comes many times a
/// second, and the window counts in whole seconds.
const REPORT_STEP: Duration = Duration::from_secs(1);

/// The time the hold goes by: the computer's, or one a test drives.
pub trait HoldClock: Send + Sync {
  /// The time now.
  fn now(&self) -> Instant;

  /// Ready at `moment`, or as soon as it passed.
  fn sleep_until(&self, moment: Instant) -> BoxFuture<'_, ()>;

  /// `moment` as the window reads it.
  fn timestamp(&self, moment: Instant) -> Timestamp;
}

/// The computer's time: monotonic for the waits, and the wall clock only for what the window is told.
struct SystemClock;

impl HoldClock for SystemClock {
  fn now(&self) -> Instant {
    Instant::now()
  }

  fn sleep_until(&self, moment: Instant) -> BoxFuture<'_, ()> {
    Box::pin(tokio::time::sleep_until(moment.into()))
  }

  fn timestamp(&self, moment: Instant) -> Timestamp {
    let left = moment.saturating_duration_since(Instant::now());
    Timestamp::from_unix_millis(
      Timestamp::now()
        .unix_millis()
        .saturating_add_unsigned(left.as_millis() as u64),
    )
  }
}

/// How a call's wait for the person came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonWait {
  /// The person was not using the tab: the call runs, and nothing waited.
  NotHeld,
  /// The person left the tab alone after this long: the call runs.
  Over(Duration),
  /// The person was still using the tab after this long, the cap: the call does not act.
  Capped(Duration),
  /// The run stopped, now or before the call came: the call does not act.
  Stopped,
  /// The tab closed or the browser restarted after this long: there is nothing left to wait for on it.
  Gone(Duration),
}

/// The agents' waits on the tabs and the person's input they wait on.
pub(super) struct Holds {
  clock: Arc<dyn HoldClock>,
  state: Mutex<State>,
  /// Moves at each change a wait must look at.
  changes: watch::Sender<u64>,
}

#[derive(Default)]
struct State {
  /// When the person's input last reached each open tab.
  last_input: HashMap<TargetId, Instant>,
  /// The tabs agents wait on, with the calls that do; a tab is its run's alone, so they are of one run.
  waits: HashMap<TargetId, TabWait>,
  /// Runs a stop reached: a call of theirs does not wait until they work again.
  stopped: HashSet<RunId>,
}

struct TabWait {
  run: RunId,
  /// When the first call began to wait: the cap counts from here, so calls that wait together end together.
  since: Instant,
  /// The moment the window was last told.
  reported: Option<Instant>,
  calls: usize,
  ending: Option<Ending>,
}

/// Why the calls waiting on a tab must end before their time.
#[derive(Clone, Copy)]
enum Ending {
  Stopped,
  Gone,
}

/// What a call waiting on a tab finds when it looks.
enum Look {
  /// Waits until this moment, or the next change; the window is told so first when `report`.
  Until {
    moment: Instant,
    report: bool,
  },
  Over,
  Capped,
  Ended(Ending),
}

impl Default for Holds {
  fn default() -> Self {
    Self::new(Arc::new(SystemClock))
  }
}

impl Holds {
  pub(super) fn new(clock: Arc<dyn HoldClock>) -> Self {
    Self {
      clock,
      state: Mutex::new(State::default()),
      changes: watch::Sender::new(0),
    }
  }

  /// The person's input reached `tab` now.
  pub(super) fn used(&self, tab: TargetId) {
    let now = self.clock.now();
    self.state.lock().last_input.insert(tab, now);
    self.changed();
  }

  /// `tab` closed: what was kept of it goes, and the calls waiting on it end.
  pub(super) fn forget(&self, tab: &TargetId) {
    {
      let mut state = self.state.lock();
      state.last_input.remove(tab);
      if let Some(wait) = state.waits.get_mut(tab) {
        wait.ending = Some(Ending::Gone);
      }
    }
    self.changed();
  }

  /// The browser ended or starts again, with tabs of new ids.
  pub(super) fn clear(&self) {
    {
      let mut state = self.state.lock();
      state.last_input.clear();
      for wait in state.waits.values_mut() {
        wait.ending = Some(Ending::Gone);
      }
    }
    self.changed();
  }

  /// `run` stops: its calls waiting now end at once, and until [`Self::go_on`] those to come do not wait.
  pub(super) fn stop(&self, run: RunId) {
    {
      let mut state = self.state.lock();
      state.stopped.insert(run);
      for wait in state.waits.values_mut().filter(|wait| wait.run == run) {
        wait.ending = Some(Ending::Stopped);
      }
    }
    self.changed();
  }

  /// `run` works again, or ended.
  pub(super) fn go_on(&self, run: RunId) {
    self.state.lock().stopped.remove(&run);
  }

  fn changed(&self) {
    self
      .changes
      .send_modify(|revision| *revision = revision.wrapping_add(1));
  }

  /// A call of `run` joins the wait on `tab`, unless it finds what it comes to right away.
  fn join(&self, run: RunId, tab: &TargetId) -> Option<PersonWait> {
    let now = self.clock.now();
    let mut state = self.state.lock();
    let held = state.last_input.get(tab).is_some_and(|last| now < *last + PERSON_HOLD);
    if !held {
      return Some(PersonWait::NotHeld);
    }
    if state.stopped.contains(&run) {
      return Some(PersonWait::Stopped);
    }
    let wait = state.waits.entry(tab.clone()).or_insert(TabWait {
      run,
      since: now,
      reported: None,
      calls: 0,
      ending: None,
    });
    wait.calls += 1;
    None
  }

  /// What a call waiting on `tab` finds now. The tab is in the waits while a call waits on it; one that is not has
  /// nothing left to wait for.
  fn look(&self, tab: &TargetId) -> Look {
    let now = self.clock.now();
    let mut state = self.state.lock();
    let last = state.last_input.get(tab).copied();
    let Some(wait) = state.waits.get_mut(tab) else {
      return Look::Ended(Ending::Gone);
    };
    if let Some(ending) = wait.ending {
      return Look::Ended(ending);
    }
    let Some(last) = last else {
      return Look::Ended(Ending::Gone);
    };
    let (released, cap) = (last + PERSON_HOLD, wait.since + WAIT_CAP);
    if now >= released {
      return Look::Over;
    }
    if now >= cap {
      return Look::Capped;
    }
    let moment = released.min(cap);
    let report = wait
      .reported
      .is_none_or(|reported| moment.saturating_duration_since(reported) >= REPORT_STEP);
    if report {
      wait.reported = Some(moment);
    }
    Look::Until { moment, report }
  }

  /// A call stops waiting on `tab`, as `end` says: what the window is told when it was the last.
  fn leave(&self, tab: &TargetId, end: AgentWaitEnd) -> Option<BrowserEvent> {
    let mut state = self.state.lock();
    let wait = state.waits.get_mut(tab)?;
    wait.calls = wait.calls.saturating_sub(1);
    if wait.calls > 0 {
      return None;
    }
    let run = wait.run;
    state.waits.remove(tab);
    Some(BrowserEvent::AgentWaitEnded {
      run,
      tab: tab_id(tab),
      end,
    })
  }

  /// What the window is told of the waits now, for a desktop that just subscribed.
  fn waiting(&self) -> Vec<BrowserEvent> {
    let state = self.state.lock();
    state
      .waits
      .iter()
      .filter_map(|(tab, wait)| {
        Some(BrowserEvent::AgentWaiting {
          run: wait.run,
          tab: tab_id(tab),
          until: self.clock.timestamp(wait.reported?),
        })
      })
      .collect()
  }
}

/// A call among those waiting on a tab; dropped, it leaves the wait, and the window hears how it ended.
struct Waiting<'a> {
  window: &'a Window,
  tab: &'a TargetId,
  /// How it ended; a wait dropped before it did was cancelled.
  end: AgentWaitEnd,
}

impl Drop for Waiting<'_> {
  fn drop(&mut self) {
    if let Some(ended) = self.window.holds.leave(self.tab, self.end) {
      self.window.send(ended);
    }
  }
}

impl Window {
  /// The window's side of a browser whose hold goes by `clock`.
  pub(in crate::browser) fn with_clock(clock: Arc<dyn HoldClock>) -> Self {
    Self {
      holds: Holds::new(clock),
      ..Self::default()
    }
  }

  /// `run` stops: see [`Holds::stop`].
  pub(in crate::browser) fn stop_waits(&self, run: RunId) {
    self.holds.stop(run);
  }

  /// `run` works again, or ended: see [`Holds::go_on`].
  pub(in crate::browser) fn waits_go_on(&self, run: RunId) {
    self.holds.go_on(run);
  }
  /// Waits while the person uses `tab`, for a call of `run` that acts on it: until [`PERSON_HOLD`] after the person's
  /// last input, new input pushing that back, at most [`WAIT_CAP`], or until the run stops or the tab goes.
  pub(in crate::browser) async fn wait_for_person(&self, run: RunId, tab: &TargetId) -> PersonWait {
    let holds = &self.holds;
    let mut changes = holds.changes.subscribe();
    if let Some(came) = holds.join(run, tab) {
      return came;
    }
    let mut waiting = Waiting {
      window: self,
      tab,
      end: AgentWaitEnd::Cancelled,
    };
    let began = holds.clock.now();
    loop {
      let waited = holds.clock.now().saturating_duration_since(began);
      match holds.look(tab) {
        Look::Over => {
          waiting.end = AgentWaitEnd::Ran;
          return PersonWait::Over(waited);
        }
        Look::Capped => {
          waiting.end = AgentWaitEnd::Capped;
          return PersonWait::Capped(waited);
        }
        Look::Ended(Ending::Stopped) => return PersonWait::Stopped,
        Look::Ended(Ending::Gone) => return PersonWait::Gone(waited),
        Look::Until { moment, report } => {
          if report {
            self.send(BrowserEvent::AgentWaiting {
              run,
              tab: tab_id(tab),
              until: holds.clock.timestamp(moment),
            });
          }
          tokio::select! {
            () = holds.clock.sleep_until(moment) => {}
            // The sender is the window's own, and outlives this call on it.
            _ = changes.changed() => {}
          }
        }
      }
    }
  }

  /// Tells a desktop that just subscribed of the waits under way.
  pub(super) fn report_waits(&self) {
    for waiting in self.holds.waiting() {
      self.send(waiting);
    }
  }
}
