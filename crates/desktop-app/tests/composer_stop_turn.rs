//! *Stop turn* in a task's Conversation (ORC-92): while the agent works, the working line offers *Stop turn*; Esc in
//! the composer stops the turn too, and the send menu offers *Stop turn and send*. A stopped turn shows in the
//! conversation, the run goes on: the agent continues its session with the person's message. On the QA seed's
//! `live` scenario with the real runner and `orchester-fake-codex`, which the test builds itself
//! (`support::runner_programs`); its "Keep working" task keeps working until it is stopped. Needs `git`.
//! Run with `cargo test -p orchester-app --test composer_stop_turn composer_stop_turn`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::{
  thread,
  time::{Duration, Instant},
};

use lurq::app::events::MouseButton;
use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{
  connection::stream::InvocationOutcome,
  project::TaskKey,
  run::RunStatus,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use support::{Data, Window, runner_programs::programs};

const TITLE: &str = "Typed errors per module";

/// As the window's `until`, for work a real runner and agent do: up to a minute.
fn wait(window: &mut Window, what: &str, mut done: impl FnMut(&mut Window) -> bool) {
  let deadline = Instant::now() + Duration::from_secs(60);
  loop {
    window.tree.tick_futures();
    window.pass();
    if done(window) {
      return;
    }
    assert!(Instant::now() < deadline, "timed out waiting for {what}");
    thread::sleep(Duration::from_millis(20));
  }
}

fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

fn key(window: &mut Window, (key, code): (&str, &str)) {
  window
    .tree
    .key_down(key.to_owned(), code.to_owned(), false, false, false);
  window.tree.key_up(key.to_owned(), code.to_owned(), false, false, false);
  window.pass();
}

/// Types `words` into the composer, a key at a time.
fn write(window: &mut Window, words: &str) {
  click(window, "composer-input");
  for letter in words.chars() {
    let (letter, code) = match letter {
      ' ' => (" ".to_owned(), "Space".to_owned()),
      letter => (letter.to_string(), format!("Key{}", letter.to_ascii_uppercase())),
    };
    key(window, (&letter, &code));
  }
}

fn text(window: &mut Window, id: &str) -> String {
  window.texts(id).unwrap_or_default().join(" ")
}

/// A window on the `live` seed, the task's run started and its agent at work, on the task's Conversation.
fn working() -> (Window, std::path::PathBuf, TaskKey) {
  let programs = programs();
  let (runner, codex) = (programs.runner.clone(), programs.fake_codex.clone());
  let data = Data::new();
  let repo = data.dir.path().join("repo-live");
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: kontur::scenarios::named("live").expect("a known scenario"),
    fake_codex: Some(codex),
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let launch = data.launch_with(runner);
  let mut window = Window::of(data, launch);
  let task = seeded.tasks.iter().find(|task| task.title == TITLE).expect("the task");
  let key: TaskKey = task.key.parse().expect("a task key");
  let page = |view| Route::Project(seeded.project, ProjectPage::Task(key.clone(), view));
  window.navigation().go(page(TaskView::of(TaskTab::Summary)));
  // Run shows a lock until the runner is connected.
  wait(&mut window, "the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|element| element.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  click(&mut window, "task-run");
  wait(&mut window, "the agent at work", |window| {
    window.texts("task-pause-run").is_some()
  });
  window.navigation().go(page(TaskView::of(TaskTab::Conversation)));
  wait(&mut window, "Stop turn on the working line", |window| {
    window.tree.get_element_by_id("conversation-stop-turn").is_some()
  });
  (window, repo, key)
}

/// The outcomes of the task's latest run's invocations, and its status.
fn latest(repo: &std::path::Path, key: &TaskKey) -> (Vec<Option<InvocationOutcome>>, RunStatus) {
  let db = ProjectDb::open(repo).expect("open");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("readable");
  let outcomes = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .map(|invocation| invocation.outcome)
    .collect();
  (outcomes, run.status().clone())
}

/// Stops the agent's turn with `stop`; the conversation shows it, the run goes on, and the next message continues it.
fn stops_and_goes_on(stop: impl FnOnce(&mut Window)) {
  let (mut window, repo, task) = working();
  stop(&mut window);
  wait(&mut window, "the stopped turn in the conversation", |window| {
    text(window, "conversation").contains("Turn stopped by you")
  });
  let (outcomes, status) = latest(&repo, &task);
  assert_eq!(outcomes.first(), Some(&Some(InvocationOutcome::TurnStopped)));
  assert_eq!(status, RunStatus::Running, "the run is not paused");
  write(&mut window, "Then finish");
  key(&mut window, ("Enter", "Enter"));
  wait(&mut window, "the run goes on to its end", |_| {
    latest(&repo, &task).1 == RunStatus::Succeeded
  });
}

#[test]
fn composer_stop_turn_from_the_working_line_and_the_next_message_goes_on() {
  stops_and_goes_on(|window| click(window, "conversation-stop-turn"));
}

#[test]
fn composer_stop_turn_esc_in_the_composer_stops_the_turn() {
  stops_and_goes_on(|window| {
    click(window, "composer-input");
    key(window, ("Escape", "Escape"));
  });
}

#[test]
fn composer_stop_turn_and_send_from_the_send_menu() {
  let (mut window, repo, task) = working();
  // The turn stops and the agent reads the message as its next instruction.
  write(&mut window, "Rename it first");
  click(&mut window, "composer-send-options");
  wait(&mut window, "the send menu", |window| {
    window.tree.get_element_by_id("composer-send-stop").is_some()
  });
  click(&mut window, "composer-send-stop");
  wait(&mut window, "the run going on with the message", |_| {
    latest(&repo, &task).1 == RunStatus::Succeeded
  });
  let (outcomes, _) = latest(&repo, &task);
  assert_eq!(
    outcomes.first(),
    Some(&Some(InvocationOutcome::TurnStopped)),
    "{outcomes:?}"
  );
  assert!(
    text(&mut window, "conversation").contains("Turn stopped by you"),
    "the stopped turn shows"
  );
}
