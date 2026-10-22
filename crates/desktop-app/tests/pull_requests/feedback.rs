//! Agent feedback on a pull request (`Pull request feedback` `seN1p`) on the `pull-request-feedback` scenario: #142's
//! threads where each stands (working, held, answered, ready to verify, in the conversation, outdated, resolved),
//! a held request withdrawn, a fix marked resolved and reopened, a question on the whole pull request and a fix on
//! lines sent to the run's Builder, #137's succeeded run offering a follow-up, and #134 without a task.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  connection::GitHubHost, id::FeedbackId, pull_request::feedback::FeedbackThread, repository::GitHubRepo,
  store::ConversationStore,
};
use orchester_store::ProjectDb;

use crate::harness::{Seeded, nonzero};

const WORKING: &str = "Check for cancellation before each attempt";
const HELD: &str = "Log the attempt number";
const ANSWERED: &str = "Why is the retry budget capped";
const READY: &str = "Seed the jitter from a fixed RNG";
const IN_CONVERSATION: &str = "Does this change the order";
const OUTDATED: &str = "Does this loop end";
const RESOLVED: &str = "Is retry_budget read once";

impl Seeded {
  fn feedback() -> Self {
    let seeded = Self::new("pull-request-feedback");
    seeded.mode("ok");
    seeded
  }

  /// Pull request `number`'s Agent feedback, `thread` open beside its list, once its threads are read.
  fn feedback_of(&mut self, number: u64, thread: Option<FeedbackId>) {
    self.window.navigation().go(Route::Project(
      self.project,
      ProjectPage::PullRequestFeedback(nonzero(number), thread),
    ));
    self.window.until("the feedback to be read", |window| {
      ["feedback-list", "feedback-no-task", "feedback-empty"]
        .iter()
        .any(|id| window.tree.get_element_by_id(id).is_some())
    });
  }

  fn threads(&self, number: u64) -> Vec<FeedbackThread> {
    let repo = GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("the seeded repository");
    ProjectDb::open(&self.repo)
      .expect("the project's database opens")
      .feedback_threads(&repo, Some(number))
      .expect("the threads are read")
  }

  /// The thread of #142 whose first request starts with `body`.
  fn thread(&self, body: &str) -> FeedbackThread {
    self
      .threads(142)
      .into_iter()
      .find(|thread| thread.posts()[0].body().as_str().starts_with(body))
      .unwrap_or_else(|| panic!("a thread asking {body}"))
  }

  fn row_state(&mut self, body: &str) -> String {
    let id = self.thread(body).id();
    self.text(&format!("feedback-row-{id}-state"))
  }
}

#[test]
fn feedback_tab_shows_where_each_thread_stands() {
  let mut seeded = Seeded::feedback();
  seeded.feedback_of(142, None);
  assert_eq!(seeded.text("pull-request-tab-feedback"), "Agent feedback 6");
  assert_eq!(
    seeded.text("pull-request-tabs-reason"),
    "Agent checks and Overview are planned."
  );
  for (body, state) in [
    (WORKING, "Working"),
    (HELD, "Queued"),
    (ANSWERED, "Answered"),
    (READY, "Ready to verify"),
    (IN_CONVERSATION, "Delivered"),
  ] {
    let shown = seeded.row_state(body);
    assert!(shown.contains(state), "{body}: {shown}");
  }
  // Its code is looked for at the head once the pull request's diff is read.
  let outdated = format!("feedback-row-{}-state", seeded.thread(OUTDATED).id());
  seeded
    .window
    .until("the thread written at an earlier head to turn outdated", |window| {
      window
        .texts(&outdated)
        .is_some_and(|texts| texts.iter().any(|text| text.contains("Outdated")))
    });
  let footer = seeded.text("feedback-footer");
  assert!(footer.starts_with("6 open threads"), "{footer}");
  let resolved = seeded.thread(RESOLVED).id();
  assert!(!seeded.shows(&format!("feedback-row-{resolved}")));
  seeded.window.press("feedback-filter-resolved");
  assert!(seeded.row_state(RESOLVED).contains("Resolved"));
}

#[test]
fn feedback_open_thread_shows_the_agents_answer_and_its_sources() {
  let mut seeded = Seeded::feedback();
  let answered = seeded.thread(ANSWERED).id();
  seeded.feedback_of(142, Some(answered));
  let panel = seeded.text("feedback-panel");
  assert!(panel.contains("src/webhooks/settings.rs:12"), "{panel}");
  assert!(
    seeded.shows(&format!("feedback-{answered}-fix")),
    "an answered question offers a fix"
  );
  let ready = seeded.thread(READY).id();
  seeded.feedback_of(142, Some(ready));
  let panel = seeded.text("feedback-panel");
  assert!(panel.contains("Seed retry jitter in tests"), "{panel}");
}

#[test]
fn feedback_held_request_is_withdrawn() {
  let mut seeded = Seeded::feedback();
  let held = seeded.thread(HELD).id();
  seeded.feedback_of(142, Some(held));
  seeded.window.press(&format!("feedback-{held}-withdraw"));
  seeded.window.until("the request to be withdrawn", |window| {
    window
      .texts("feedback-list")
      .is_some_and(|texts| !texts.iter().any(|text| text.contains(HELD)))
  });
  let thread = seeded.thread(HELD);
  assert!(thread.is_withdrawn(), "{thread:?}");
  assert_eq!(thread.posts()[0].message(), None, "it was never sent");
}

#[test]
fn feedback_fix_ready_to_verify_is_resolved_and_reopened() {
  let mut seeded = Seeded::feedback();
  let ready = seeded.thread(READY).id();
  seeded.feedback_of(142, Some(ready));
  seeded.window.press(&format!("feedback-{ready}-resolve"));
  seeded.window.until("the thread to be resolved", |window| {
    window
      .tree
      .get_element_by_id(&format!("feedback-{ready}-reopen"))
      .is_some()
  });
  assert!(seeded.thread(READY).resolved_at().is_some());
  seeded.window.press(&format!("feedback-{ready}-reopen"));
  seeded.window.until("the thread to be reopened", |window| {
    window
      .tree
      .get_element_by_id(&format!("feedback-{ready}-resolve"))
      .is_some()
  });
  assert_eq!(seeded.thread(READY).resolved_at(), None);
}

#[test]
fn feedback_question_on_the_whole_pull_request_goes_to_the_builder() {
  let mut seeded = Seeded::feedback();
  seeded.feedback_of(142, None);
  let before = seeded.threads(142).len();
  seeded.window.press("feedback-ask");
  assert!(seeded.shows("feedback-composer"));
  assert_eq!(seeded.text("feedback-composer-send"), "Ask");
  seeded
    .window
    .set("feedback-composer-text", "Which endpoints are retried first?");
  seeded.window.press("feedback-composer-send");
  seeded.window.until("the question to be kept", |window| {
    window.tree.get_element_by_id("feedback-composer").is_none()
  });
  let threads = seeded.threads(142);
  assert_eq!(threads.len(), before + 1);
  let asked = threads.last().expect("the new thread");
  assert_eq!(asked.posts()[0].body().as_str(), "Which endpoints are retried first?");
  let message = asked.posts()[0].message().expect("a question is sent at once");
  let conversation = ProjectDb::open(&seeded.repo)
    .expect("the project's database opens")
    .messages(asked.task())
    .expect("the conversation");
  let sent = conversation
    .iter()
    .find(|candidate| candidate.id() == message)
    .expect("the message is in the task's conversation");
  assert!(
    sent.body().as_str().starts_with("Question about pull request #142"),
    "{}",
    sent.body().as_str()
  );
}

#[test]
fn feedback_fix_on_lines_starts_from_files_changed() {
  let mut seeded = Seeded::feedback();
  seeded.window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::PullRequest(nonzero(142), None),
  ));
  seeded.window.until("the files", |window| {
    window
      .tree
      .get_element_by_id("changes-file:src/webhooks/sender.rs")
      .is_some()
  });
  seeded.window.press("changes-file:src/webhooks/sender.rs");
  seeded.window.until("the file's diff", |window| {
    window.tree.get_element_by_id("changes-diff-line-new-12").is_some()
  });
  seeded.window.press("changes-diff-line-new-12");
  seeded.window.until("the composer under the line", |window| {
    window.tree.get_element_by_id("feedback-composer").is_some()
  });
  let anchor = seeded.text("feedback-composer-anchor");
  assert!(anchor.contains("src/webhooks/sender.rs"), "{anchor}");
  seeded.window.press("feedback-composer-fix");
  assert_eq!(seeded.text("feedback-composer-send"), "Request fix");
  seeded
    .window
    .set("feedback-composer-text", "Name this retry after the endpoint.");
  seeded.window.press("feedback-composer-send");
  seeded.window.until("the fix to be kept", |window| {
    window.tree.get_element_by_id("feedback-composer").is_none()
  });
  let fix = seeded.thread("Name this retry");
  let lines = fix.anchor().lines().expect("on lines");
  assert_eq!((lines.path(), lines.span().start()), ("src/webhooks/sender.rs", 12));
  assert!(
    fix.posts()[0].is_held(),
    "a fix waits for the one the Builder is working on"
  );
}

#[test]
fn feedback_on_a_succeeded_run_starts_a_follow_up() {
  let mut seeded = Seeded::feedback();
  seeded.feedback_of(137, None);
  seeded.window.press("feedback-ask");
  assert_eq!(seeded.text("feedback-composer-send"), "Start follow-up run and send");
}

#[test]
fn feedback_needs_a_linked_task() {
  let mut seeded = Seeded::feedback();
  seeded.feedback_of(134, None);
  let shown = seeded.text("feedback-no-task");
  assert!(shown.contains("Agent feedback needs a linked task"), "{shown}");
  assert!(shown.contains("#134"), "{shown}");
}
