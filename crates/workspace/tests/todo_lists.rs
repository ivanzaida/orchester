//! A run's steps and the Orchestrator chat's turns as screens and automation read them, each with the todo list its
//! agent's latest update left (ORC-314); a step or a turn whose agent keeps none has no list.

mod support;

use orchester_core::{
  connection::stream::{AgentEvent, TodoItem, TodoList, TodoState},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::Chat,
  resource::ModelId,
  store::{ActivityStore, RunStore},
};
use orchester_workspace::{orchestrator::chat::ChatView, tasks::detail::run_view};
use support::{Fixture, at};

fn item(text: &str, state: TodoState) -> TodoItem {
  TodoItem {
    text: text.to_owned(),
    state,
  }
}

#[test]
fn todo_lists_of_a_run_step_are_its_latest_executions_latest_update() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(40));
  let run = fixture.started_run(task, at(50));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let view = run_view(&fixture.db, fixture.db.run(run.id()).expect("read").expect("kept")).expect("view");
  assert_eq!(view.steps[0].todo, None, "its agent keeps no list yet");
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(55))
    .expect("invocation");
  let started = vec![item("Read the form", TodoState::InProgress)];
  let later = vec![
    item("Read the form", TodoState::Done),
    item("Add the check", TodoState::Pending),
  ];
  fixture
    .db
    .append_activity(
      &invocation,
      &[
        (at(56), AgentEvent::TodoList { items: started }),
        (at(57), AgentEvent::TodoList { items: later.clone() }),
      ],
    )
    .expect("transcript");
  let view = run_view(&fixture.db, fixture.db.run(run.id()).expect("read").expect("kept")).expect("view");
  assert_eq!(view.steps[0].todo, TodoList::new(later));
}

#[test]
fn todo_lists_of_orchestrator_turns_are_read_with_their_conversation() {
  let mut fixture = Fixture::new();
  let body = MessageBody::new("Plan the login work").expect("text");
  let chat = Chat::start(ChatId::generate(), &body, at(10));
  let (planned, plain) = (ChatMessageId::generate(), ChatMessageId::generate());
  fixture
    .db
    .keep_chat_message(&chat, (planned, &body), at(10))
    .expect("kept");
  fixture
    .db
    .keep_chat_message(&chat, (plain, &body), at(20))
    .expect("kept");
  let list = vec![
    item("Survey the runs", TodoState::InProgress),
    item("Report", TodoState::Pending),
  ];
  fixture.db.keep_chat_todo(planned, &list, at(11)).expect("kept");
  let view = ChatView::load_in(fixture.repo.path(), chat.id())
    .expect("read")
    .expect("kept");
  assert_eq!(view.todo_of(planned), TodoList::new(list).as_ref());
  assert_eq!(view.todo_of(plain), None, "a turn whose agent keeps none has no list");
}
