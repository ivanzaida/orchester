//! A run's pull request CI on its task's page (ORC-335), through the seed's fake `gh`: *Typed errors per module*'s
//! run succeeded with pull request #137, one of its five checks failed; *Add retry to webhook sender* runs with #142,
//! one of its checks still running. The Summary's card says how CI stands, *Fix CI* sends the failing check to the
//! run's agent as one message and records it, the Conversation shows it among its rows, only a person sets *Fix CI
//! automatically*, and a `gh` that is not signed in shows the callout with its command and the switch locked.

use lurq::app::events::with_synthetic_input;
use orchester_app::{
  root::Start,
  routes::{ProjectPage, Route, TaskTab, TaskView},
};
use orchester_core::{
  pull_request::ci::fix::{FixEvent, FixEventKind, FixedBy},
  store::{ConversationStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::outside::Outside;

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
  // The run ended at its review: Fix CI goes to the agent of the step that wrote the code.
  assert_eq!(
    seeded.text("task-ci-fix-hint"),
    "Sends the failing check's name and last 40 log lines to Builder"
  );
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
  let sent = messages.last().expect("the message");
  let text = sent.body().as_str().to_owned();
  assert!(
    text.contains(
      "[check 5](https://github.com/ivanzaida/kontur/actions/runs/137/job/13704) · failed · last lines of the log"
    ),
    "{text}"
  );
  let shown = format!("conversation-message-{}-text", sent.id());
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
  sent_in_the_conversation(&mut seeded, &shown);
}

/// The Conversation's Fix CI line, and its message, whose words are element `shown`, drawn as Markdown: its log tail a
/// code block, no fence shown as text (`11b` `X8xYZ`).
fn sent_in_the_conversation(seeded: &mut Seeded, shown: &str) {
  open(
    seeded,
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
  let tail = format!("{shown}-code-1");
  seeded.window.until("the Fix CI message's log tail", |window| {
    window.tree.get_element_by_id(&tail).is_some()
  });
  let words = seeded.window.texts(shown).unwrap_or_default().concat();
  assert!(!words.contains("```"), "no fence shows: {words}");
  assert!(words.contains("test webhook_retry_137 ... FAILED"), "{words}");
}

/// The *Fix CI automatically* switch of the latest run of the task titled `title`, as stored.
fn auto_on(seeded: &Seeded, title: &str) -> bool {
  let db = ProjectDb::open(&seeded.repo).expect("the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the task")
    .id();
  let run = db
    .runs_of_task(task)
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("readable");
  db.ci_fixes(run.id()).expect("the run's Fix CI").auto.on
}

#[test]
fn ci_a_fix_that_waits_for_a_pipeline_that_is_not_ready_here_is_not_called_fixing() {
  let mut seeded = Seeded::answering("ok");
  let title = "Typed errors per module";
  open(&mut seeded, title, TaskTab::Summary, "task-ci");
  seeded.window.press("task-ci-fix");
  seeded.window.until("the card waits: Fix CI was sent", |window| {
    window.texts("task-ci-badge").is_some_and(|badge| badge == ["Fixing"])
  });
  assert_eq!(seeded.text("task-status"), "Fixing CI on #137");

  // The task's pipeline is not ready here: Run again with the message started nothing, and the run records it.
  let mut db = ProjectDb::open(&seeded.repo).expect("the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the task")
    .id();
  let run = db
    .runs_of_task(task)
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("readable");
  let sent = db.ci_fixes(run.id()).expect("the run's Fix CI").events.remove(0);
  let FixEventKind::Sent { message, .. } = sent.kind else {
    panic!("the Fix CI sent");
  };
  let waiting = FixEvent {
    at: Timestamp::now(),
    kind: FixEventKind::NotReady { message },
    ..sent
  };
  db.record_ci_fix(run.id(), &waiting).expect("recorded");
  drop(db);
  let Start::Ready(ready) = seeded.window.launch.start() else {
    panic!("a started session");
  };
  ready.services.changes.record(seeded.project, Outside::Tasks);

  open(&mut seeded, title, TaskTab::Conversation, "conversation-ci-fix-0");
  open(&mut seeded, title, TaskTab::Summary, "task-ci");
  seeded.window.until("the header says it is kept", |window| {
    window
      .texts("task-status")
      .is_some_and(|status| status.concat().contains("is kept"))
  });
  assert_eq!(seeded.text("task-status"), "Fix CI on #137 is kept · not ready here");
}

#[test]
fn ci_fix_automatically_is_set_by_a_person_only() {
  let mut seeded = Seeded::answering("ok");
  let title = "Typed errors per module";
  open(&mut seeded, title, TaskTab::Summary, "task-ci-auto-switch");
  let off = "Off · CI waits for you when it fails";
  // Window automation flipping the switch would restart its count and so lift the run's limit: it is refused.
  with_synthetic_input(|| seeded.window.press("task-ci-auto-switch"));
  for _ in 0..40 {
    seeded.window.tree.tick_futures();
    seeded.window.pass();
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
  assert!(
    !auto_on(&seeded, title),
    "automation did not turn Fix CI automatically on"
  );
  assert_eq!(seeded.text("task-ci-auto-hint"), off);
  seeded.window.press("task-ci-auto-switch");
  seeded.window.until("the person's switch saved", |window| {
    window.texts("task-ci-auto-hint").is_some_and(|hint| hint != [off])
  });
  assert!(auto_on(&seeded, title), "the person turned Fix CI automatically on");
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
