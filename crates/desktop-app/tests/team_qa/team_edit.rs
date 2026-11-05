//! Edit task on a team project's task (ORC-734, `Team authority · redesign` 05 `LmxUY`), on the QA app's `team-admin`,
//! `team-edit-conflict` and `team-member` states with the stand-in team server: the form reads the task from the
//! server and an Admin's save goes there through the dispatcher on the task's revision; a change someone else made
//! first stops it with what the task is now, to set the person's again or keep theirs; a member, who never reaches the
//! form, is told only Admins change the team's tasks when they save anyway.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_edit`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ProjectPage, Route},
  test_support::{Window, project_id},
};

use super::{prepared, team::State};

/// KON-12 on the server: number 12, "Add retry to webhook sender", priority High, revision 1.
const NUMBER: u32 = 12;
const CALLOUT: &str = "task-form-team-conflict-callout";

/// Edit task on KON-12 in `state`, once its fields are filled from the server's read.
fn editing(state: State) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  let key = "KON-12".parse().expect("a task key");
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::EditTask(key)));
  window.until("Edit task's form", |window| {
    window.tree.get_element_by_id("task-form-title-input").is_some()
  });
  (window, server)
}

fn set_priority(window: &mut Window, priority: &str) {
  window.press("task-form-priority");
  let option = format!("task-form-priority-{priority}");
  window.until("the priority menu", |window| {
    window.tree.get_element_by_id(&option).is_some()
  });
  window.press(&option);
}

fn priority(server: &TeamServer) -> (u64, String) {
  let (revision, priority, _) = server.task_now(NUMBER).expect("the server has KON-12");
  (revision, priority)
}

#[test]
fn team_qa_edit_saves_an_admins_change_on_the_server() {
  let (mut window, server) = editing(State::Admin);
  window.set("task-form-title-input", "Retry webhook sends with backoff");
  window.press("task-form-save");
  window.until("the server to take it", |_| {
    server
      .task_now(NUMBER)
      .is_some_and(|(_, _, title)| title == "Retry webhook sends with backoff")
  });
  assert_eq!(server.task_now(NUMBER).map(|(revision, ..)| revision), Some(2));
  window.until("the form to be saved", |window| {
    window.text("task-form-status-label") == "Draft saved locally"
  });
  assert!(window.texts(CALLOUT).is_none());
}

#[test]
fn team_qa_edit_a_change_made_first_shows_what_it_is_now_and_sets_the_persons_again() {
  let (mut window, server) = editing(State::EditConflict);
  set_priority(&mut window, "Low");
  window.press("task-form-save");
  window.until("someone else's change", |window| window.texts(CALLOUT).is_some());
  let said = window.text(CALLOUT);
  assert!(said.starts_with("olha changed KON-12 first"), "{said}");
  assert!(said.contains("Priority is Urgent now (olha, "), "{said}");
  assert!(
    said.contains("you set Low. The team server keeps one version"),
    "{said}"
  );
  assert_eq!(window.text("task-form-team-conflict-reapply"), "Set Low again");
  assert_eq!(window.text("task-form-team-conflict-keep"), "Keep Urgent");
  assert_eq!(
    priority(&server),
    (2, "urgent".to_owned()),
    "nothing of the person's was saved"
  );

  window.press("task-form-team-conflict-reapply");
  window.until("the person's priority", |_| priority(&server) == (3, "low".to_owned()));
  window.until("the callout to go", |window| window.texts(CALLOUT).is_none());
}

#[test]
fn team_qa_edit_keep_takes_the_task_as_it_is_now() {
  let (mut window, server) = editing(State::EditConflict);
  set_priority(&mut window, "Low");
  window.press("task-form-save");
  window.until("someone else's change", |window| window.texts(CALLOUT).is_some());
  window.press("task-form-team-conflict-keep");
  window.until("the callout to go", |window| window.texts(CALLOUT).is_none());
  window.until("nothing left to save", |window| {
    window.text("task-form-status-label") == "Draft saved locally"
  });
  assert_eq!(priority(&server), (2, "urgent".to_owned()));
}

#[test]
fn team_qa_edit_a_members_save_says_only_admins_change_the_teams_tasks() {
  let (mut window, server) = editing(State::Member);
  window.set("task-form-title-input", "Retry webhook sends with backoff");
  window.press("task-form-save");
  window.until("the refusal", |window| window.texts("task-form-team-refused").is_some());
  let said = window.text("task-form-team-refused");
  assert!(said.contains("Couldn't save KON-12"), "{said}");
  assert!(said.contains("Only Admins of the workspace change its tasks"), "{said}");
  assert_eq!(server.task_now(NUMBER).map(|(revision, ..)| revision), Some(1));
}
