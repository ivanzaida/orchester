//! The person's hold on a tab as the window's side of the project's browser keeps it (`docs/design/browser.md`,
//! decision 16), on a clock the tests drive instead of sleeping for ten seconds: input and navigation start it, each
//! new input pushes back the moment a waiting call runs, the wait has a cap, a stop of the run or a dropped call ends
//! it, and the window hears it all. Only the tab that closes needs a real browser; the agents' calls end to end are in
//! [`super::hold_tools`].

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::{
  Event,
  browser::{
    AgentWaitEnd, BrowserCommand, BrowserEvent, BrowserTabId, Modifiers, MouseAction, MouseButton, Navigation,
    PersonInput,
  },
};
use orchester_runner::browser::{HoldClock, PERSON_HOLD, PersonWait, ProjectBrowser, WAIT_CAP, cdp::TargetId};
use tokio::{
  sync::{mpsc, watch},
  task::JoinHandle,
  time::timeout,
};

use super::site;
use crate::{browser_installed, support::PATIENCE};

/// The wall clock of the driven clock's start, in milliseconds.
const STARTS_AT: i64 = 1_800_000_000_000;
/// Long enough for a call that should not end yet to have ended, if it were going to: the test's clock stands still.
const STILL: Duration = Duration::from_millis(50);

/// A clock that moves only when the test says.
pub(super) struct DrivenClock {
  start: Instant,
  passed: watch::Sender<Duration>,
}

impl DrivenClock {
  pub(super) fn new() -> Arc<Self> {
    Arc::new(Self {
      start: Instant::now(),
      passed: watch::Sender::new(Duration::ZERO),
    })
  }

  pub(super) fn advance(&self, by: Duration) {
    self.passed.send_modify(|passed| *passed += by);
  }

  /// `after` from the start of this clock, as the window is told it.
  pub(super) fn at(&self, after: Duration) -> Timestamp {
    self.timestamp(self.start + after)
  }
}

impl HoldClock for DrivenClock {
  fn now(&self) -> Instant {
    self.start + *self.passed.borrow()
  }

  fn sleep_until(&self, moment: Instant) -> BoxFuture<'_, ()> {
    let mut passed = self.passed.subscribe();
    Box::pin(async move {
      // The sender is this clock's own, which outlives the sleep it lends itself to.
      drop(passed.wait_for(|passed| self.start + *passed >= moment).await);
    })
  }

  fn timestamp(&self, moment: Instant) -> Timestamp {
    let after = moment.saturating_duration_since(self.start).as_millis();
    Timestamp::from_unix_millis(STARTS_AT + i64::try_from(after).expect("a test's time"))
  }
}

pub(super) const fn moved() -> PersonInput {
  PersonInput::Mouse {
    action: MouseAction::Moved,
    x: 1,
    y: 1,
    button: MouseButton::None,
    clicks: 0,
    modifiers: Modifiers {
      alt: false,
      ctrl: false,
      meta: false,
      shift: false,
    },
  }
}

const fn pressed() -> PersonInput {
  PersonInput::Mouse {
    action: MouseAction::Pressed,
    x: 1,
    y: 1,
    button: MouseButton::Left,
    clicks: 1,
    modifiers: Modifiers {
      alt: false,
      ctrl: false,
      meta: false,
      shift: false,
    },
  }
}

/// A browser on the driven clock with the window subscribed, and one of a run's tabs, which no browser needs here.
struct Held {
  browser: Arc<ProjectBrowser>,
  clock: Arc<DrivenClock>,
  window: mpsc::UnboundedReceiver<Event>,
  run: RunId,
  tab: TargetId,
  _profile: tempfile::TempDir,
}

impl Held {
  async fn new() -> Self {
    let profile = tempfile::tempdir().expect("temporary folder");
    let clock = DrivenClock::new();
    let browser = Arc::new(ProjectBrowser::new(profile.path().to_path_buf()).with_hold_clock(clock.clone()));
    let (desktop, window) = mpsc::unbounded_channel();
    browser
      .serve(browser.arrived(BrowserCommand::Subscribe), &desktop)
      .await
      .expect("subscribed");
    let mut held = Self {
      browser,
      clock,
      window,
      run: RunId::generate(),
      tab: TargetId::from("T1"),
      _profile: profile,
    };
    assert!(matches!(held.event().await, BrowserEvent::Changed(_)));
    held
  }

  fn tab_id(&self) -> BrowserTabId {
    BrowserTabId::new(self.tab.as_str())
  }

  /// The person's `input` reaches the tab, as the window's request is read.
  fn input(&self, input: PersonInput) {
    let tab = self.tab_id();
    drop(self.browser.arrived(BrowserCommand::Input { tab, input }));
  }

  /// An agent's call of the run on the tab begins to wait for the person, on a task of its own.
  fn call(&self) -> JoinHandle<PersonWait> {
    let (browser, run, tab) = (Arc::clone(&self.browser), self.run, self.tab.clone());
    tokio::spawn(async move { browser.wait_for_person(run, &tab).await })
  }

  async fn event(&mut self) -> BrowserEvent {
    let event = timeout(PATIENCE, self.window.recv())
      .await
      .expect("the window's event in time")
      .expect("the browser keeps sending");
    match event {
      Event::Browser(event) => event,
      other => panic!("a browser event, not {other:?}"),
    }
  }

  /// The window hears that the run's agent waits on the tab until `after` from the clock's start.
  async fn told_waiting_until(&mut self, after: Duration) {
    let (run, tab, until) = (self.run, self.tab_id(), self.clock.at(after));
    assert_eq!(self.event().await, BrowserEvent::AgentWaiting { run, tab, until });
  }

  async fn told_ended(&mut self, end: AgentWaitEnd) {
    let (run, tab) = (self.run, self.tab_id());
    assert_eq!(self.event().await, BrowserEvent::AgentWaitEnded { run, tab, end });
  }

  fn told_nothing_more(&mut self) {
    assert!(self.window.try_recv().is_err(), "nothing more for the window");
  }
}

async fn still_waiting(call: &mut JoinHandle<PersonWait>) {
  assert!(
    timeout(STILL, &mut *call).await.is_err(),
    "the call waits while the clock stands still"
  );
}

async fn came_to(call: JoinHandle<PersonWait>) -> PersonWait {
  timeout(PATIENCE, call)
    .await
    .expect("the wait ends in time")
    .expect("the call's task")
}

#[tokio::test]
async fn browser_hold_is_kept_for_a_tab_the_person_used_less_than_ten_seconds_ago() {
  let mut held = Held::new().await;
  assert_eq!(came_to(held.call()).await, PersonWait::NotHeld, "no input, no wait");

  held.input(moved());
  held.clock.advance(PERSON_HOLD);
  assert_eq!(
    came_to(held.call()).await,
    PersonWait::NotHeld,
    "input ten seconds ago holds nothing"
  );
  held.input(moved());
  let other = TargetId::from("T2");
  let (browser, run) = (Arc::clone(&held.browser), held.run);
  let elsewhere = tokio::spawn(async move { browser.wait_for_person(run, &other).await });
  assert_eq!(came_to(elsewhere).await, PersonWait::NotHeld, "another tab is not held");
  held.told_nothing_more();
}

/// A copy or a cut is the person at work on the tab too, whether or not the page changes.
#[tokio::test]
async fn browser_hold_starts_on_a_copy_and_on_a_cut() {
  let mut held = Held::new().await;
  let tab = held.tab_id();
  drop(held.browser.arrived(BrowserCommand::CopySelection { tab: tab.clone() }));
  let call = held.call();
  held.told_waiting_until(PERSON_HOLD).await;
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(call).await, PersonWait::Over(PERSON_HOLD));
  held.told_ended(AgentWaitEnd::Ran).await;

  drop(held.browser.arrived(BrowserCommand::CutSelection { tab }));
  let call = held.call();
  held.told_waiting_until(PERSON_HOLD * 2).await;
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(call).await, PersonWait::Over(PERSON_HOLD));
  held.told_ended(AgentWaitEnd::Ran).await;
}

#[tokio::test]
async fn browser_hold_waits_until_ten_seconds_after_the_last_input_and_each_input_pushes_it_back() {
  let mut held = Held::new().await;
  held.input(pressed());
  let mut call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;

  held.clock.advance(Duration::from_secs(4));
  still_waiting(&mut call).await;
  held.input(pressed());
  held.told_waiting_until(Duration::from_secs(14)).await;

  held.clock.advance(Duration::from_secs(3));
  held.input(moved());
  held.told_waiting_until(Duration::from_secs(17)).await;

  // The window's navigation of the tab holds it as its input does.
  held.clock.advance(Duration::from_secs(3));
  let to = Navigation::Reload;
  let tab = held.tab_id();
  drop(held.browser.arrived(BrowserCommand::Navigate { tab, to }));
  held.told_waiting_until(Duration::from_secs(20)).await;

  held.clock.advance(Duration::from_millis(9_999));
  still_waiting(&mut call).await;
  held.clock.advance(Duration::from_millis(1));
  assert_eq!(came_to(call).await, PersonWait::Over(Duration::from_secs(20)));
  held.told_ended(AgentWaitEnd::Ran).await;
  held.told_nothing_more();
}

/// A moving pointer sends input many times a second; the window counts in seconds.
#[tokio::test]
async fn browser_hold_tells_the_window_only_when_the_moment_moved_a_second() {
  let mut held = Held::new().await;
  held.input(moved());
  let mut call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  for _ in 0..9 {
    held.clock.advance(Duration::from_millis(100));
    held.input(moved());
    still_waiting(&mut call).await;
  }
  held.told_nothing_more();
  held.clock.advance(Duration::from_millis(100));
  held.input(moved());
  held.told_waiting_until(Duration::from_secs(11)).await;
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(call).await, PersonWait::Over(Duration::from_secs(11)));
  held.told_ended(AgentWaitEnd::Ran).await;
}

#[tokio::test]
async fn browser_hold_gives_up_at_its_cap_while_the_person_goes_on() {
  let mut held = Held::new().await;
  held.input(moved());
  let mut call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  // The window is told the moment the wait ends, which the cap keeps from going further.
  let mut passed = Duration::ZERO;
  let step = Duration::from_secs(5);
  while passed + step < WAIT_CAP {
    held.clock.advance(step);
    passed += step;
    held.input(moved());
    still_waiting(&mut call).await;
    if passed + PERSON_HOLD <= WAIT_CAP {
      held.told_waiting_until(passed + PERSON_HOLD).await;
    }
  }
  held.clock.advance(WAIT_CAP - passed);
  assert_eq!(came_to(call).await, PersonWait::Capped(WAIT_CAP));
  held.told_ended(AgentWaitEnd::Capped).await;
  held.told_nothing_more();
}

/// A person who stops in time for the cap lets the call run: the cap is for one who goes on.
#[tokio::test]
async fn browser_hold_runs_at_the_cap_when_the_person_stopped_by_then() {
  let mut held = Held::new().await;
  held.input(moved());
  let call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  let mut passed = Duration::ZERO;
  while passed < WAIT_CAP - PERSON_HOLD {
    let step = Duration::from_secs(5);
    held.clock.advance(step);
    passed += step;
    held.input(moved());
    held.told_waiting_until(passed + PERSON_HOLD).await;
  }
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(call).await, PersonWait::Over(WAIT_CAP));
  held.told_ended(AgentWaitEnd::Ran).await;
}

#[tokio::test]
async fn browser_hold_ends_at_once_when_the_run_stops_and_waits_again_once_it_goes_on() {
  let mut held = Held::new().await;
  held.input(moved());
  let call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;

  held.browser.stop_person_waits(RunId::generate());
  held.told_nothing_more();
  held.browser.stop_person_waits(held.run);
  assert_eq!(came_to(call).await, PersonWait::Stopped, "the clock did not move");
  held.told_ended(AgentWaitEnd::Cancelled).await;

  assert_eq!(
    came_to(held.call()).await,
    PersonWait::Stopped,
    "a call that comes while the run stops does not wait either"
  );
  held.told_nothing_more();

  held.browser.person_waits_go_on(held.run);
  let mut again = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  still_waiting(&mut again).await;
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(again).await, PersonWait::Over(PERSON_HOLD));
  held.told_ended(AgentWaitEnd::Ran).await;
}

/// The relay of a worker that ended takes the call with it.
#[tokio::test]
async fn browser_hold_dropped_with_its_call_tells_the_window_it_was_cancelled() {
  let mut held = Held::new().await;
  held.input(moved());
  let call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  call.abort();
  held.told_ended(AgentWaitEnd::Cancelled).await;
  held.told_nothing_more();
}

/// Two calls on the tab are one wait for the window, which ends with the last of them.
#[tokio::test]
async fn browser_hold_is_one_wait_for_the_calls_on_a_tab() {
  let mut held = Held::new().await;
  held.input(moved());
  let first = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  let mut second = held.call();
  still_waiting(&mut second).await;
  held.told_nothing_more();
  first.abort();
  still_waiting(&mut second).await;
  held.told_nothing_more();
  held.clock.advance(PERSON_HOLD);
  assert_eq!(came_to(second).await, PersonWait::Over(PERSON_HOLD));
  held.told_ended(AgentWaitEnd::Ran).await;
}

/// A desktop that comes while an agent waits learns it with the browser's state.
#[tokio::test]
async fn browser_hold_under_way_is_told_to_a_desktop_that_subscribes() {
  let mut held = Held::new().await;
  held.input(moved());
  let _call = held.call();
  held.told_waiting_until(Duration::from_secs(10)).await;
  let (desktop, mut window) = mpsc::unbounded_channel();
  held
    .browser
    .serve(held.browser.arrived(BrowserCommand::Subscribe), &desktop)
    .await
    .expect("subscribed");
  let mut events = Vec::new();
  for _ in 0..2 {
    events.push(window.recv().await.expect("an event"));
  }
  let (run, tab, until) = (held.run, held.tab_id(), held.clock.at(Duration::from_secs(10)));
  assert!(matches!(events[0], Event::Browser(BrowserEvent::Changed(_))));
  assert_eq!(
    events[1],
    Event::Browser(BrowserEvent::AgentWaiting { run, tab, until })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_ends_when_the_person_closes_the_tab() {
  if !browser_installed() {
    return;
  }
  let mut held = Held::new().await;
  let site = site::serve().await;
  held.tab = held
    .browser
    .open_tab(held.run, &format!("{site}held"))
    .await
    .expect("opened");
  held.input(moved());
  let call = held.call();
  let tab = held.tab_id();
  while !matches!(held.event().await, BrowserEvent::AgentWaiting { .. }) {}
  let (desktop, _events) = mpsc::unbounded_channel();
  held
    .browser
    .serve(held.browser.arrived(BrowserCommand::CloseTab { tab }), &desktop)
    .await
    .expect("the window closes the tab");
  assert_eq!(came_to(call).await, PersonWait::Gone(Duration::ZERO));
  while !matches!(
    held.event().await,
    BrowserEvent::AgentWaitEnded {
      end: AgentWaitEnd::Cancelled,
      ..
    }
  ) {}
  held.browser.stop().await;
}
