//! The person's hold through the browser tools, end to end: the fake CLI's script `act_on_a_held_tab`
//! (`support/fake_browser_tools.rs`), the real browser and a clock the test drives. Every call that acts waits and then
//! says so, the reading tools never do, the cap and a stop of the run answer without acting, a call is judged by the
//! site it acts on once its wait is over, and the window hears it all. The clock stands still unless the test moves it,
//! so a call that wrongly waits ends the test at its patience.

use std::{fs, path::Path, sync::Arc, time::Duration};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::browser::BrowserAccess,
  run::{Answerer, ApprovalAnswer, PauseReason, PendingToolApproval, RunStatus},
};
use orchester_ipc::{
  Event,
  browser::{AgentWaitEnd, BrowserCommand, BrowserEvent, BrowserTabId, Navigation},
};
use orchester_runner::{
  browser::{PERSON_HOLD, ProjectBrowser, WAIT_CAP},
  execution::Scheduler,
};
use tokio::sync::mpsc;

use super::{
  hold::{DrivenClock, moved},
  pages::ELSEWHERE,
  site,
  tools::{answers, browsing, browsing_setup, call_of, calls},
};
use crate::{
  browser_installed,
  support::{PATIENCE, Setup, TestProject, pending_approval, project, scheduler, wait_for},
};

/// The first line of an answer after a wait of ten seconds.
const WAITED: &str = "The person was using this tab in the Browser window, so this call waited 10 s before it ran.";
/// The first line of the answer of a call denied after a wait of ten seconds.
const WAITED_NOT_RUN: &str =
  "The person was using this tab in the Browser window, so this call waited 10 s and did not run.";

/// A run on the fake's `script`, its form open in the tab the test uses as the person.
struct HeldRun {
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  desktop: mpsc::UnboundedSender<Event>,
  window: mpsc::UnboundedReceiver<Event>,
  clock: Arc<DrivenClock>,
  id: RunId,
  tab: BrowserTabId,
  /// The address of the pages, on a local host.
  site: String,
  _profile: tempfile::TempDir,
}

impl HeldRun {
  async fn start(project: &TestProject, script: &str) -> Self {
    let root = project.repo.path();
    let profile = tempfile::tempdir().expect("temporary folder");
    let clock = DrivenClock::new();
    let (scheduler, events) = scheduler(root);
    let browser = ProjectBrowser::new(profile.path().to_path_buf()).with_hold_clock(clock.clone());
    let scheduler = scheduler.with_browser(browser);
    let (desktop, window) = mpsc::unbounded_channel();
    let subscribe = scheduler.browser().arrived(BrowserCommand::Subscribe);
    scheduler
      .browser()
      .serve(subscribe, &desktop)
      .await
      .expect("subscribed");
    let site = site::serve().await;
    fs::write(say(root, "fake-browser-site"), &site).expect("site");
    fs::write(say(root, "fake-browser-hold"), script).expect("script");
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    let mut held = Self {
      scheduler,
      events,
      desktop,
      window,
      clock,
      id,
      tab: BrowserTabId::new(""),
      site,
      _profile: profile,
    };
    let first = held
      .until(|event| matches!(event, BrowserEvent::AgentActed { .. }))
      .await;
    let BrowserEvent::AgentActed { tab, .. } = first else {
      unreachable!("waited for an action");
    };
    held.tab = tab;
    held
  }

  /// The next browser event `wanted` accepts.
  async fn until(&mut self, wanted: impl Fn(&BrowserEvent) -> bool) -> BrowserEvent {
    tokio::time::timeout(PATIENCE, async {
      loop {
        if let Event::Browser(event) = self.window.recv().await.expect("the browser keeps sending")
          && wanted(&event)
        {
          return event;
        }
      }
    })
    .await
    .expect("the window's event in time")
  }

  /// The person's input reaches the tab, now on the driven clock.
  async fn use_tab(&self) {
    let input = BrowserCommand::Input {
      tab: self.tab.clone(),
      input: moved(),
    };
    let browser = self.scheduler.browser();
    browser
      .serve(browser.arrived(input), &self.desktop)
      .await
      .expect("the input reaches the tab");
  }

  /// The fake's next step goes ahead, as `word` says.
  fn go(root: &Path, step: u32, word: &str) {
    fs::write(say(root, &format!("fake-browser-go-{step}")), word).expect("the word");
  }

  /// The window hears the agent waits on the tab; from then on the clock moves.
  async fn waits(&mut self) -> BrowserEvent {
    self
      .until(|event| matches!(event, BrowserEvent::AgentWaiting { .. }))
      .await
  }

  async fn ended(&mut self, end: AgentWaitEnd) {
    let event = self
      .until(|event| matches!(event, BrowserEvent::AgentWaitEnded { .. }))
      .await;
    let (run, tab) = (self.id, self.tab.clone());
    assert_eq!(event, BrowserEvent::AgentWaitEnded { run, tab, end });
  }

  /// The person uses the tab, the fake's `step` waits on it, the person leaves it alone, and it runs.
  async fn step_waits_then_runs(&mut self, root: &Path, step: u32) {
    self.use_tab().await;
    Self::go(root, step, "go");
    self.waits().await;
    self.clock.advance(PERSON_HOLD);
    self.ended(AgentWaitEnd::Ran).await;
  }

  /// The person goes on using the tab until the wait on it reaches its cap.
  async fn use_tab_to_the_cap(&mut self) {
    let step = Duration::from_secs(5);
    let mut passed = Duration::ZERO;
    while passed + step < WAIT_CAP {
      self.clock.advance(step);
      passed += step;
      self.use_tab().await;
    }
    self.clock.advance(WAIT_CAP - passed);
    self.ended(AgentWaitEnd::Capped).await;
  }

  /// The person sends the tab to `page` of `site` in the window, and its page is there.
  async fn go_to(&mut self, site: &str, page: &str) {
    let navigate = BrowserCommand::Navigate {
      tab: self.tab.clone(),
      to: Navigation::Address {
        url: format!("{site}{page}"),
      },
    };
    let browser = self.scheduler.browser();
    browser
      .serve(browser.arrived(navigate), &self.desktop)
      .await
      .expect("the tab goes there");
    let tab = self.tab.clone();
    self
      .until(|event| {
        matches!(event, BrowserEvent::Changed(state) if state.tabs.iter().any(|shown| {
          shown.id == tab && shown.url.ends_with(page) && !shown.loading
        }))
      })
      .await;
  }

  /// The approval the run waits for, which must be about `action` on `target`.
  async fn asked(&mut self, root: &Path, (action, target): (&str, &str)) -> PendingToolApproval {
    let asked = pending_approval(root, &mut self.events, self.id).await;
    assert_eq!(
      (asked.action.as_str(), asked.target.as_deref()),
      (action, Some(target)),
      "{:?}",
      calls(root, self.id)
    );
    asked
  }

  async fn answer(&self, asked: PendingToolApproval, answer: ApprovalAnswer) {
    self
      .scheduler
      .decide_tool_approval(self.id, asked.id, answer, Answerer::Person)
      .await
      .expect("answered");
  }

  async fn answer_ask(&mut self, root: &Path, about: (&str, &str), answer: ApprovalAnswer) {
    let asked = self.asked(root, about).await;
    self.answer(asked, answer).await;
  }
}

fn say(root: &Path, name: &str) -> std::path::PathBuf {
  root.join(".orchester").join(name)
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_every_acting_call_waits_and_says_so_and_reading_tools_do_not() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let mut held = HeldRun::start(&project, "every-call").await;

  // The tab is held and the clock stands still through the reads: the next step's wait comes only once they answered.
  held.use_tab().await;
  HeldRun::go(root, 0, "go");
  for step in 1..=6 {
    held.step_waits_then_runs(root, step).await;
  }

  // `browser_eval` waits first, and asks once the wait is over. Input before it ran would hold it again.
  held.use_tab().await;
  HeldRun::go(root, 7, "go");
  held.waits().await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;
  held
    .answer_ask(root, ("browser_eval", "127.0.0.1"), ApprovalAnswer::Once)
    .await;
  held
    .until(|event| matches!(event, BrowserEvent::AgentActed { action, .. } if action.tool == "browser_eval"))
    .await;

  // The tab is held again: `browser_open` of a new tab touches no tab the person uses, and its action comes while the
  // clock stands still. `browser_close` of the held tab waits.
  held.use_tab().await;
  HeldRun::go(root, 8, "go");
  held
    .until(|event| matches!(event, BrowserEvent::AgentActed { action, .. } if action.summary.ends_with("other")))
    .await;
  HeldRun::go(root, 9, held.tab.as_str());
  held.waits().await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;

  let id = held.id;
  let run = wait_for(root, &mut held.events, id, |run| run.status().is_terminal()).await;
  let called = calls(root, id);
  assert_eq!(run.status(), &RunStatus::Succeeded, "{called:?}");
  held.scheduler.browser().stop().await;
  every_call_said_so(root, id, &called);
}

fn every_call_said_so(root: &Path, id: RunId, called: &[String]) {
  for tool in [
    "browser_open",
    "browser_navigate",
    "browser_type",
    "browser_click",
    "browser_key",
    "browser_scroll",
    "browser_eval",
    "browser_close",
  ] {
    let index = usize::from(tool == "browser_open");
    assert_eq!(
      call_of(called, tool, index),
      format!("{tool} ok: {WAITED}"),
      "{called:?}"
    );
  }
  for tool in [
    "browser_tabs",
    "browser_read_page",
    "browser_page_text",
    "browser_screenshot",
    "browser_console",
    "browser_network",
    "browser_wait",
  ] {
    assert!(
      !call_of(called, tool, 0).contains("waited"),
      "{tool} did not wait: {called:?}"
    );
  }
  assert!(
    !call_of(called, "browser_open", 2).contains("waited"),
    "a new tab did not wait: {called:?}"
  );
  assert!(
    answers(root, id, "browser_click")
      .iter()
      .any(|line| line.starts_with("Clicked ")),
    "the answer goes on with what the call did"
  );
}

/// The site is judged on the page the call acts on: the person's navigation starts the hold, so a call decided before
/// its wait could act on a site nobody judged. A call on a local page asks about where the person went, and does not
/// act when denied; an approval stands while the person only goes on using the page it named, but not once they go
/// elsewhere; and a wait that reaches its cap asks nobody.
#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_judges_the_site_the_call_acts_on_once_its_wait_is_over() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let mut held = HeldRun::start(&project, "site-moves").await;
  let elsewhere = held.site.replace("127.0.0.1", ELSEWHERE);

  held.use_tab().await;
  HeldRun::go(root, 0, "go");
  held.waits().await;
  held.go_to(&elsewhere, "form?away").await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;
  held
    .answer_ask(root, ("browser_key", ELSEWHERE), ApprovalAnswer::Deny)
    .await;

  // The person comes back to the page while the approval is pending: the call waits again, and is not asked about
  // again.
  HeldRun::go(root, 1, "go");
  let asked = held.asked(root, ("browser_eval", ELSEWHERE)).await;
  held.use_tab().await;
  held.answer(asked, ApprovalAnswer::Once).await;
  held.waits().await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;

  // Now they go to another site while it is pending: what was approved was not that site.
  HeldRun::go(root, 2, "go");
  let asked = held.asked(root, ("browser_eval", ELSEWHERE)).await;
  let home = held.site.clone();
  held.go_to(&home, "home").await;
  held.answer(asked, ApprovalAnswer::Once).await;
  held.waits().await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;
  held
    .answer_ask(root, ("browser_eval", "127.0.0.1"), ApprovalAnswer::Deny)
    .await;

  // `browser_eval` always asks, yet not when its wait reaches the cap: the run ends with nobody asked.
  held.use_tab().await;
  HeldRun::go(root, 3, "go");
  held.waits().await;
  held.use_tab_to_the_cap().await;

  let id = held.id;
  let run = wait_for(root, &mut held.events, id, |run| run.status().is_terminal()).await;
  let called = calls(root, id);
  assert_eq!(run.status(), &RunStatus::Succeeded, "{called:?}");
  held.scheduler.browser().stop().await;
  judged_where_it_acted(root, id, &called);
}

fn judged_where_it_acted(root: &Path, id: RunId, called: &[String]) {
  assert_eq!(
    call_of(called, "browser_key", 0),
    format!("browser_key error: {WAITED_NOT_RUN}"),
    "denied, it did not act and still says it waited"
  );
  assert_eq!(
    call_of(called, "browser_eval", 0),
    format!("browser_eval ok: {WAITED}"),
    "{called:?}"
  );
  assert!(
    answers(root, id, "browser_eval")
      .iter()
      .any(|line| line.contains("BODY")),
    "the denied key press did not move the focus: {:?}",
    answers(root, id, "browser_eval")
  );
  assert_eq!(
    call_of(called, "browser_eval", 1),
    format!("browser_eval error: {WAITED_NOT_RUN}"),
    "{called:?}"
  );
  assert_eq!(
    call_of(called, "browser_eval", 2),
    "browser_eval error: The person is still using this tab in the Browser window: this call waited 60 s and did not \
     act. Try again later, or work in another tab."
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_answers_without_acting_at_its_cap_and_a_later_call_runs() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let mut held = HeldRun::start(&project, "one-click").await;
  held.use_tab().await;
  HeldRun::go(root, 0, "go");
  held.waits().await;
  held.use_tab_to_the_cap().await;

  // The person stops: the fake's next click runs without waiting.
  held.clock.advance(PERSON_HOLD);
  HeldRun::go(root, 1, "go");
  let id = held.id;
  let run = wait_for(root, &mut held.events, id, |run| run.status().is_terminal()).await;
  let called = calls(root, id);
  assert_eq!(run.status(), &RunStatus::Succeeded, "{called:?}");
  held.scheduler.browser().stop().await;

  assert_eq!(
    call_of(&called, "browser_click", 0),
    "browser_click error: The person is still using this tab in the Browser window: this call waited 60 s and did \
     not act. Try again later, or work in another tab."
  );
  let texts = answers(root, id, "browser_page_text");
  let saved_at = texts
    .iter()
    .position(|line| line.starts_with("Saved"))
    .expect("saved once");
  assert!(
    texts[..saved_at].iter().any(|line| line == "Not saved"),
    "the capped click did not act: {texts:?}"
  );
  assert!(
    call_of(&called, "browser_click", 1).contains("ok: Clicked "),
    "{called:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_ends_without_acting_when_the_run_is_cancelled() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let mut held = HeldRun::start(&project, "one-click").await;
  held.use_tab().await;
  HeldRun::go(root, 0, "go");
  held.waits().await;

  let (scheduler, id) = (held.scheduler.clone(), held.id);
  let cancelling = tokio::spawn(async move { scheduler.cancel_run(id).await });
  held.ended(AgentWaitEnd::Cancelled).await;
  cancelling
    .await
    .expect("the cancel's task")
    .expect("the run is cancelled");
  let run = wait_for(root, &mut held.events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  let called = calls(root, id);
  assert!(
    !called.iter().any(|call| call.contains("ok: Clicked")),
    "the click did not act: {called:?}"
  );
  held.scheduler.browser().stop().await;
}

/// A pause lets a running tool finish, so the wait must end by itself: the click answers that it did not act, the next
/// one too while the tab is still held, and the run pauses at its next step without the clock moving. Resumed, that
/// step's calls wait again.
#[tokio::test(flavor = "multi_thread")]
async fn browser_hold_ends_without_acting_when_the_run_pauses_and_waits_again_once_resumed() {
  if !browser_installed() {
    return;
  }
  let project = project(&Setup {
    review: true,
    ..browsing_setup(AgentProvider::CodexCli, BrowserAccess::On)
  });
  let root = project.repo.path();
  let mut held = HeldRun::start(&project, "one-click").await;
  held.use_tab().await;
  HeldRun::go(root, 0, "go");
  held.waits().await;

  let id = held.id;
  held.scheduler.pause_run(id).await.expect("the pause is requested");
  held.ended(AgentWaitEnd::Cancelled).await;
  HeldRun::go(root, 1, "go");
  let paused = wait_for(root, &mut held.events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  let stopped = "This call did not act: the person was using this tab in the Browser window, and the run is stopping.";
  let called = calls(root, id);
  assert_eq!(
    call_of(&called, "browser_click", 0),
    format!("browser_click error: {stopped}")
  );
  assert_eq!(
    call_of(&called, "browser_click", 1),
    format!("browser_click error: {stopped}")
  );
  assert!(
    !answers(root, id, "browser_page_text")
      .iter()
      .any(|line| line.starts_with("Saved")),
    "neither click acted"
  );

  // Resumed, the agent starts on the tab again, which is still held: the clock has not moved.
  held.scheduler.resume_run(id).await.expect("the run resumes");
  held.waits().await;
  held.clock.advance(PERSON_HOLD);
  held.ended(AgentWaitEnd::Ran).await;
  let run = wait_for(root, &mut held.events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", calls(root, id));
  held.scheduler.browser().stop().await;
}
