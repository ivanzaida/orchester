//! The sidebar's Running list and the Orchestrator's conversation read again only for what they show: the runs under
//! way when a run changes state, never for a transcript's lines; the conversation shown when it changed, not when the
//! Orchestrator writes in another one. The reads they start never overlap (`settled_read`).

use orchester_app::routes::{OrchestratorChat, ProjectPage};
use orchester_core::id::ChatId;
use orchester_store::ProjectDb;

use super::Counted;

#[test]
fn reads_the_running_list_on_a_run_changing_state_not_on_its_lines() {
  let mut running = Counted::open("tasks");
  running.show(ProjectPage::Overview(Default::default()), "running");
  running.report(false);
  let line = running.settle();
  assert_eq!(line.get("running"), None, "a transcript line reads no runs: {line:?}");
  running.report(true);
  let state = running.settle();
  assert_eq!(
    state.get("running"),
    Some(&1),
    "a run's state reads the runs: {state:?}"
  );
}

/// The conversation of the seeded project that its follower of waits kept.
fn kept_chat(counted: &Counted) -> ChatId {
  ProjectDb::open(&counted.root())
    .expect("the project's database")
    .chats()
    .expect("the chats")
    .into_iter()
    .next()
    .expect("the scenario keeps a conversation")
    .chat
    .id()
}

#[test]
fn reads_an_orchestrator_conversation_on_its_own_changes_not_on_another_ones() {
  let mut screen = Counted::open("conversation");
  let services = screen.window.launch.services().expect("the session started").clone();
  let chats = services.runners.chats_of(&screen.root());
  screen
    .window
    .until("the waits kept in the Orchestrator chat", |_| *chats.borrow() > 0);
  let chat = kept_chat(&screen);
  screen.show(
    ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
    "orchestrator-conversation",
  );
  services
    .runners
    .count_chat_change_for_test(&screen.root(), ChatId::generate());
  let other = screen.settle();
  assert_eq!(
    other.get("orchestrator conversation"),
    None,
    "another conversation's change does not read this one: {other:?}"
  );
  assert_eq!(
    other.get("orchestrator history"),
    Some(&1),
    "History lists them all: {other:?}"
  );
  services.runners.count_chat_change_for_test(&screen.root(), chat);
  let own = screen.settle();
  assert_eq!(
    own.get("orchestrator conversation"),
    Some(&1),
    "its own change reads it: {own:?}"
  );
}
