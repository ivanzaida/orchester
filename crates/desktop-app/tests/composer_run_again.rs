//! The Conversation composer's *Run again with this message* runs the task as the person read it: a task edited
//! after the page read it is refused by the runner, and the message stays queued with why. On the QA seed's
//! `live` scenario with the real runner and `orchester-fake-codex`, which the test builds itself
//! (`support::runner_programs`). Needs `git`.

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
  store::{RunStore, TaskStore},
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

/// Clicks the middle of `id`. Delivers no future: what the page read stays what it shows.
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

fn icon_is(window: &mut Window, id: &str, icon: Icon) -> bool {
  window
    .tree
    .get_element_by_id(id)
    .and_then(|element| element.text_content().map(str::to_owned))
    .is_some_and(|glyph| glyph == icon.glyph().to_string())
}

/// Types `words` into the composer, a key at a time; only lays the window out.
fn write(window: &mut Window, words: &str) {
  click(window, "composer-input");
  for letter in words.chars() {
    let (key, code) = match letter {
      ' ' => (" ".to_owned(), "Space".to_owned()),
      letter => (letter.to_string(), format!("Key{}", letter.to_ascii_uppercase())),
    };
    window.tree.key_down(key.clone(), code.clone(), false, false, false);
    window.tree.key_up(key, code, false, false, false);
    window.pass();
  }
}

#[test]
fn composer_run_again_of_a_task_changed_since_the_page_read_it_keeps_the_message() {
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
  let seeded_task = seeded.tasks.iter().find(|task| task.title == TITLE).expect("the task");
  let key: orchester_core::project::TaskKey = seeded_task.key.parse().expect("a task key");
  let page = |view| Route::Project(seeded.project, ProjectPage::Task(key.clone(), view));
  window.navigation().go(page(TaskView::of(TaskTab::Summary)));
  wait(&mut window, "the runner", |window| {
    window.tree.get_element_by_id("task-run-icon").is_some() && !icon_is(window, "task-run-icon", Icon::Lock)
  });
  click(&mut window, "task-run");
  wait(&mut window, "the agent at work", |window| {
    window.texts("task-pause-run").is_some()
  });
  click(&mut window, "task-cancel-run");
  wait(&mut window, "Run again", |window| {
    icon_is(window, "task-run-again-icon", Icon::RotateCcw)
  });
  window.navigation().go(page(TaskView::of(TaskTab::Conversation)));
  wait(&mut window, "the composer's Run again", |window| {
    window
      .texts("composer-send")
      .is_some_and(|texts| texts.join(" ").contains("Run again with this message"))
  });
  write(&mut window, "Keep going");

  // Another window edits the task. No future is delivered from here to the press, so the composer sends
  // the revision the page read before the edit.
  let mut db = ProjectDb::open(&repo).expect("the project");
  let mut task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == TITLE)
    .expect("the task");
  let mut details = task.details().clone();
  details.brief = "Delete the repository.".into();
  task.edit(details, true).expect("edit");
  db.save_task(&task).expect("save").saved().expect("saved as read");
  window
    .tree
    .key_down("Enter".into(), "Enter".into(), false, false, false);
  window.tree.key_up("Enter".into(), "Enter".into(), false, false, false);
  window.pass();

  wait(&mut window, "why the message stays queued", |window| {
    window
      .texts("composer-kept")
      .is_some_and(|texts| texts.join(" ").contains("The task was changed since you opened it"))
  });
  assert_eq!(db.runs_of_task(task.id()).expect("runs").len(), 1, "nothing started");
  let services = window.launch.services().expect("the session started").clone();
  services.runners.terminate();
}
