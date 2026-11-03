//! A run's pull request CI on its task's page (ORC-335), through the seed's fake `gh`: *Typed errors per module*'s
//! run succeeded with pull request #137, one of its five checks failed; *Add retry to webhook sender* runs with #142,
//! one of its checks still running. The Summary's card says how CI stands, *Fix CI* sends the failing check to the
//! run's agent as one message and records it, the Conversation shows it among its rows, and a `gh` that is not signed
//! in shows the callout with its command and the switch locked.

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  pull_request::ci::fix::{FixEventKind, FixedBy},
  store::{ConversationStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use crate::harness::Seeded;

/// Opens `tab` of the task titled `title` and waits for its pull request's CI card or line, `ready`.
fn open(seeded: &mut Seeded, title: &str, tab: TaskTab, ready: &str) {
  let key = seeded.key(title);
  let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(tab));
  seeded.window.navigation().go(Route::Project(seeded.project, page));
  let ready = ready.to_owned();
  seeded.window.until(&format!("{ready} of {title}"), move |window| {
    window.texts(&ready).is_some()
  });
}

#[test]
fn ci_a_failed_check_shows_on_the_summary_and_fix_ci_sends_it() {
  let mut seeded = Seeded::answering("ok");
  open(&mut seeded, "Typed errors per module", TaskTab::Summary, "task-ci");
  assert_eq!(seeded.text("task-ci-title"), "Pull request #137");
  assert_eq!(seeded.text("task-ci-badge"), "Failed");
  assert_eq!(seeded.text("task-ci-count"), "CI · 1 of 5 checks failed");
  assert!(seeded.text("task-ci-check-0").starts_with("check 5 failed"));
  assert!(
    seeded
      .text("task-ci-check-0-tail")
      .contains("test webhook_retry_137 ... FAILED"),
    "the failing check's log tail"
  );
  assert!(seeded.text("task-ci-passed").starts_with("4 passed · check 1"));
  assert_eq!(seeded.text("task-status"), "CI failed on #137");
  assert_eq!(seeded.text("task-ci-auto-hint"), "Off · CI waits for you when it fails");
  seeded.window.press("task-ci-fix");
  let repo = seeded.repo.clone();
  let task = ProjectDb::open(&repo)
    .expect("the project")
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Typed errors per module")
    .expect("the task")
    .id();
  seeded.window.until("the Fix CI message", |_| {
    ProjectDb::open(&repo)
      .expect("the project")
      .messages(task)
      .expect("messages")
      .iter()
      .any(|message| message.body().as_str().starts_with("CI failed on pull request #137."))
  });
  let db = ProjectDb::open(&repo).expect("the project");
  let messages = db.messages(task).expect("messages");
  let text = messages.last().expect("the message").body().as_str().to_owned();
  assert!(text.contains("check 5 · failed · last lines of the log"), "{text}");
  assert!(
    text.contains("Details: https://github.com/ivanzaida/kontur/actions/runs/137/job/13704"),
    "{text}"
  );
  assert!(!text.contains("check 1"), "passed checks are left out: {text}");
  let run = db
    .runs_of_task(task)
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("readable");
  let fixes = db.ci_fixes(run.id()).expect("the run's Fix CI");
  assert!(matches!(
    fixes.events.as_slice(),
    [event] if matches!(event.kind, FixEventKind::Sent { by: FixedBy::Hand, .. })
  ));
  // The card waits: a Fix CI was sent for this failure.
  seeded.window.until("the task read again", |window| {
    window.texts("task-ci-badge").is_some_and(|badge| badge == ["Fixing"])
  });
  open(
    &mut seeded,
    "Typed errors per module",
    TaskTab::Conversation,
    "conversation-ci-fix-0",
  );
  assert!(
    seeded
      .text("conversation-ci-fix-0-text")
      .starts_with("CI failed on #137 · check 5 · Fix CI sent to "),
    "the Fix CI's line"
  );
}

#[test]
fn ci_running_shows_each_check_and_no_fix() {
  let mut seeded = Seeded::answering("ok");
  open(&mut seeded, "Add retry to webhook sender", TaskTab::Summary, "task-ci");
  assert_eq!(seeded.text("task-ci-badge"), "Running");
  assert_eq!(seeded.text("task-ci-count"), "CI · 5 of 6 checks done");
  assert!(seeded.text("task-ci-check-5").starts_with("check 6"));
  assert!(!seeded.shows("task-ci-fix"), "nothing failed: no Fix CI");
  assert_eq!(seeded.text("task-ci-auto-hint"), "Off · CI waits for you when it fails");
}

#[test]
fn ci_without_a_signed_in_gh_says_how_to_sign_in_and_locks_the_switch() {
  let mut seeded = Seeded::answering("not-signed-in");
  open(
    &mut seeded,
    "Typed errors per module",
    TaskTab::Summary,
    "task-ci-not-read",
  );
  assert_eq!(seeded.text("task-ci-badge"), "Unknown");
  assert_eq!(seeded.text("task-ci-count"), "CI · checks not read");
  assert!(
    seeded
      .text("task-ci-not-read")
      .starts_with("GitHub CLI isn't signed in")
  );
  assert_eq!(seeded.text("task-ci-command"), "gh auth login");
  assert!(seeded.shows("task-ci-check-again"));
  assert!(!seeded.shows("task-ci-fix"));
  assert_eq!(seeded.text("task-ci-auto-hint"), "Needs gh to read checks");
  assert_eq!(seeded.text("task-status"), "Pull request #137 · CI not read");
}
