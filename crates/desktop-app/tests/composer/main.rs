//! The composer under a task's Conversation tab, headless at 1440 × 900 on the QA seed's `task-states`: what
//! it offers in each state of the run ([`states`]: placeholder, recipient, the send button's words, the line
//! over the box and the one under it, the lock once no agent reads messages), and what sending does
//! ([`sending`]: Enter and Ctrl+Enter, a draft kept through a failed send with *Try again*, a message stored
//! once, a message its agent did not take sent after the turn or put back into the composer). The window's
//! runner program does not exist, so what a stored message asks of the runner is refused and the message
//! stays queued; an agent at work (*Send now*, *Send after this turn*) needs a live run (QA's `live`
//! scenario). Needs `git`.

mod attachments;
mod keeping;
#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its tasks"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
mod sending;
mod states;
#[path = "../support/mod.rs"]
mod support;
mod theme;

use kontur::{SeedRequest, Seeded, scenarios};
use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{conversation::Message, id::TaskId, project::TaskKey, store::ConversationStore, time::Timestamp};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::detail::TaskDetail;
use support::{Data, Window};

/// A window on `task-states`, seeded in the window's own data folder.
fn seeded() -> (Window, Seeded) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-task-states"),
    scenario: scenarios::named("task-states").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  (Window::of(data, launch), seeded)
}

/// The key of the seeded task titled `title`.
fn key_of(seeded: &Seeded, title: &str) -> TaskKey {
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.title == title)
    .unwrap_or_else(|| panic!("no seeded task {title}"));
  task.key.parse().expect("a task key")
}

/// The id of the task `key`.
fn task_id(seeded: &Seeded, key: &TaskKey) -> TaskId {
  use orchester_core::store::TaskStore;
  let db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  db.tasks()
    .expect("the tasks are read")
    .into_iter()
    .find(|task| task.key() == key)
    .map(|task| task.id())
    .expect("the task is stored")
}

/// The task `key` with its latest run, as stored.
fn detail(seeded: &Seeded, key: &TaskKey) -> TaskDetail {
  TaskDetail::load_in(&seeded.repo, task_id(seeded, key)).expect("the task is read")
}

/// The task's messages, as stored.
fn messages(seeded: &Seeded, key: &TaskKey) -> Vec<Message> {
  let db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  db.messages(task_id(seeded, key)).expect("the messages are read")
}

/// Opens the Conversation tab of the task titled `title` and waits for its composer.
fn open(window: &mut Window, seeded: &Seeded, title: &str) -> TaskKey {
  let key = key_of(seeded, title);
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key.clone(), TaskView::Conversation),
  ));
  window.until(&format!("the composer of {key}"), |window| {
    window.tree.get_element_by_id("composer").is_some()
  });
  key
}

/// The texts inside `id`, joined; empty when it is not shown.
fn text(window: &mut Window, id: &str) -> String {
  window.texts(id).unwrap_or_default().join(" ")
}

/// Clicks the middle of the element `id`.
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
  window.pass();
}

/// A key pressed in the focused field: `ctrl` and `shift` held as asked.
fn key(window: &mut Window, (key, code): (&str, &str), (shift, ctrl): (bool, bool)) {
  window
    .tree
    .key_down(key.to_owned(), code.to_owned(), shift, ctrl, false);
  window.tree.key_up(key.to_owned(), code.to_owned(), shift, ctrl, false);
  window.pass();
}

/// Clicks into the message field and types `words`, a key at a time.
fn write(window: &mut Window, words: &str) {
  click(window, "composer-input");
  type_on(window, words);
}

/// Types `words` where the focus is, a key at a time.
fn type_on(window: &mut Window, words: &str) {
  for letter in words.chars() {
    let (key, code) = match letter {
      ' ' => (" ".to_owned(), "Space".to_owned()),
      letter => (letter.to_string(), format!("Key{}", letter.to_ascii_uppercase())),
    };
    key_press(window, &key, &code);
  }
}

fn key_press(window: &mut Window, key: &str, code: &str) {
  self::key(window, (key, code), (false, false));
}

/// The draft as the field holds it.
fn draft(window: &mut Window) -> String {
  text(window, "composer-input")
}
