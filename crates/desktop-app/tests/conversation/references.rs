//! Places an agent's message in a task's conversation names as plain text are links, as in the Orchestrator's chat
//! (`ui::chat::references`): a key of one of the project's tasks and an app route path of a place in a project, as
//! text or as all a code span holds; an unknown key, a route outside a project and a path inside longer code stay
//! as written. Every in-app link opens by a
//! browser's rules (`support::link_press`).

use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  connection::stream::AgentEvent,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{Window, key_of, open, seeded, support::link_press::opens_by_browser_rules};

/// Whether an element's text is `text`: a link's words.
fn is_link(window: &mut Window, text: &str) -> bool {
  window
    .tree
    .find_element(|element| element.text_content() == Some(text))
    .is_some()
}

#[test]
fn conversation_message_links_task_keys_and_routes_and_opens_them_by_browser_rules() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let other = seeded
    .tasks
    .iter()
    .map(|task| task.key.clone())
    .find(|other| *other != key.to_string())
    .expect("another task");
  let project = seeded.project;
  let changes = format!("/project/{project}/tasks/{other}/changes");
  let code = format!("/project/{project}/tasks/{other}/conversation");
  let longer = format!("/project/{project}/tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key() == &key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .expect("a run");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .pop()
    .expect("an invocation");
  let said = format!("Like {other}: see {changes} and `{code}`. Not KON-999, /settings or `open {longer}`.");
  db.append_activity(&invocation, &[(Timestamp::now(), AgentEvent::Message { text: said })])
    .expect("the message");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the linked message", |window| is_link(window, &other));
  assert!(is_link(&mut window, &changes), "a route path is a link");
  assert!(is_link(&mut window, &code), "a code span of a route path is a link");
  for text in ["KON-999", "/settings", longer.as_str()] {
    assert!(!is_link(&mut window, text), "{text} stays text");
  }

  let other_key = other.parse().expect("a task key");
  let summary = Route::Project(project, ProjectPage::Task(other_key, TaskView::Summary));
  opens_by_browser_rules(&mut window, &other, &summary);
  window.navigation().back();
  window.until("the chat again", |window| is_link(window, &changes));
  let place = Route::parse(&changes).expect("the Changes route");
  opens_by_browser_rules(&mut window, &changes, &place);
}
