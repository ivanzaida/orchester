//! The latest todo list of a step execution, read from its transcript, and of an Orchester chat turn, kept in place
//! of the one before; no list is `None`, never an empty one.

mod support;

use orchester_core::{
  connection::stream::{AgentEvent, TodoItem, TodoList, TodoState},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::Chat,
  store::ActivityStore,
};

use self::support::{at, model, started_run};

fn item(text: &str, state: TodoState) -> TodoItem {
  TodoItem {
    text: text.to_owned(),
    state,
  }
}

fn update(items: Vec<TodoItem>) -> AgentEvent {
  AgentEvent::TodoList { items }
}

#[test]
fn todo_lists_of_a_step_are_its_transcripts_latest_update() {
  let (_repo, mut db, run, execution) = started_run();
  assert_eq!(db.step_todo(run.id(), execution).expect("read"), None, "no update yet");
  let first = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let started = vec![item("Read", TodoState::InProgress), item("Write", TodoState::Pending)];
  let later = vec![item("Read", TodoState::Done), item("Write", TodoState::InProgress)];
  db.append_activity(&first, &[(at(11), update(started)), (at(12), AgentEvent::TurnStarted)])
    .expect("append");
  // The step's next invocation goes on with the list; its update replaces the one before.
  let second = db
    .start_invocation(run.id(), execution, &model(), at(20))
    .expect("start");
  db.append_activity(&second, &[(at(21), update(later.clone()))])
    .expect("append");
  assert_eq!(db.step_todo(run.id(), execution).expect("read"), TodoList::new(later));
  db.append_activity(&second, &[(at(22), update(Vec::new()))])
    .expect("append");
  assert_eq!(
    db.step_todo(run.id(), execution).expect("read"),
    None,
    "an emptied list is none"
  );
}

#[test]
fn todo_lists_of_a_chat_turn_replace_the_one_before() {
  let (_repo, mut db, _run, _execution) = started_run();
  let body = MessageBody::new("Plan the webhook work").expect("text");
  let chat = Chat::start(ChatId::generate(), &body, at(10));
  let (first, second) = (ChatMessageId::generate(), ChatMessageId::generate());
  db.keep_chat_message(&chat, (first, &body), at(10)).expect("kept");
  db.keep_chat_message(&chat, (second, &body), at(20)).expect("kept");
  assert!(db.chat_todos(chat.id()).expect("read").is_empty());
  let planned = vec![item("Survey the runs", TodoState::InProgress)];
  let done = vec![
    item("Survey the runs", TodoState::Done),
    item("Report", TodoState::Pending),
  ];
  db.keep_chat_todo(first, &planned, at(11)).expect("kept");
  db.keep_chat_todo(second, &planned, at(21)).expect("kept");
  db.keep_chat_todo(first, &done, at(12)).expect("replaced");
  db.keep_chat_todo(second, &[], at(22)).expect("emptied");
  let kept = db.chat_todos(chat.id()).expect("read");
  assert_eq!(kept, [(first, TodoList::new(done).expect("a list"))]);
}
