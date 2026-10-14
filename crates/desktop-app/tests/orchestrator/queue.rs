//! Holding new starts from the chat and from Tasks: the Orchestrator's agent holds the project's queue through the
//! same tool as any client of the app, Tasks says new starts are held, and *Release* there lets them go on.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskList};
use orchester_core::orchestrator::{ActionOutcome, EntryKind};

use super::harness::{Orchestrator, Setup};

/// Whether new starts are held in the whole project.
fn held(screen: &Orchestrator) -> bool {
  screen.db().queue_holds().expect("holds").project
}

#[test]
fn orchestrator_holds_new_starts_and_tasks_releases_them() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call hold_queue {"project":"Kontur"}"#);
  screen.until("the held queue", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("New starts held")
  });
  assert!(held(&screen));
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let done = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .any(|entry| matches!(entry.kind, EntryKind::Action(action) if action.tool == "hold_queue" && action.outcome == ActionOutcome::Done));
  assert!(done, "the action is kept as done");
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));

  let project = screen.project;
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("Tasks saying so", |screen| {
    screen.text("tasks-queue-held").contains("New starts held")
  });
  screen.click("tasks-queue-release");
  screen.until("released", |screen| !held(screen));
  screen.until("Hold new starts again", |screen| screen.exists("tasks-queue-hold"));
  screen.click("tasks-queue-hold");
  screen.until("held from Tasks", |screen| held(screen));
}
