//! A native agent's question with suggested answers ended its step (ORC-83): its card stands in for the
//! agent's question message, so the chat does not show the question twice, and the composer does not answer
//! it: a message there waits for the answer. Once answered, the card keeps the answer and the person's
//! message that carried it to the agent is not shown again. Seeded, without a runner.
//! `cargo test -p orchester-desktop --test form_keyboard question_card_step_end`.

use std::path::Path;

use orchester_core::{
  conversation::{AgentMessage, Message, MessageBody, Recipient},
  id::{IdempotencyKey, MessageId},
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionTurn, question_as_text},
  run::{Answerer, Invocation, Run, WaitReason},
  store::{ConversationStore, RunStore},
};
use orchester_store::ProjectDb;

use crate::{
  conversation_composer::placeholder, github::shows, harness::Desktop, native_run::native_run, task_run::has,
  unreadable_run::at,
};

/// The step of KON-1's run waits for the answer to Builder's question with options; the request.
fn asked(repo: &Path) -> (Run, Invocation, QuestionRequest) {
  let (mut run, invocation) = native_run(repo);
  let mut db = ProjectDb::open(repo).expect("open the project");
  let question = Question::new(
    "Which database should the cache use?",
    Some("Storage"),
    vec![
      QuestionOption::new("SQLite", Some("One file next to the app")),
      QuestionOption::new("Postgres", None),
    ],
    false,
  )
  .expect("a question");
  let request = QuestionRequest::new(
    format!("{}:toolu_ask", invocation.id),
    (run.id(), invocation.execution, invocation.id),
    "Builder".to_owned(),
    vec![question.clone()],
    (at(29), QuestionTurn::StepEnd),
  )
  .expect("a request");
  db.ask_questions(&request).expect("asked");
  let body = MessageBody::new(&question_as_text(&question)).expect("a question");
  let message = Message::from_agent(
    run.task(),
    run.id(),
    (invocation.execution, invocation.id),
    AgentMessage::Question,
    body,
    at(30),
  );
  db.add_agent_message(&message).expect("the agent's question");
  let events = run
    .wait_step(invocation.execution, WaitReason::Message, at(30))
    .expect("the step waits");
  db.append_events(&run, &events).expect("append");
  (run, invocation, request)
}

fn open_conversation(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
}

/// Whether any element of the chat is the plain question card of an agent's question message.
fn plain_question_card(desktop: &mut Desktop) -> bool {
  desktop
    .tree
    .find_element(|element| element.id().is_some_and(|id| id.starts_with("conversation-asks-")))
    .is_some()
}

#[test]
fn question_card_step_end_stands_in_for_the_question_and_the_composer_waits_for_it() {
  let mut id = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (_, _, request) = asked(repo);
    id = Some(request.id().to_owned());
  });
  let card = format!("conversation-question-card-{}", id.expect("seeded"));
  open_conversation(&mut desktop);
  desktop.wait_for(&card);
  for text in [
    "Builder asks you",
    "Storage",
    "SQLite",
    "One file next to the app",
    "Postgres",
  ] {
    assert!(shows(&mut desktop, &card, text), "{text}");
  }
  assert!(
    !plain_question_card(&mut desktop),
    "the question shows once, as its card"
  );
  // The composer does not answer it: a message there reaches Builder after the answer.
  desktop.wait_for("conversation-input");
  assert_eq!(placeholder(&mut desktop), "Message Builder…");
  assert!(shows(
    &mut desktop,
    "conversation-explanation",
    "Builder is waiting for your answer above; a message here reaches it after you answer."
  ));
  assert!(shows(&mut desktop, "conversation-recipient", "asking you"));
}

#[test]
fn question_card_step_end_answered_keeps_the_answer_and_hides_its_message() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, _, request) = asked(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    let answers = request
      .answer(&[GivenAnswer {
        choices: vec!["Postgres".to_owned()],
        text: None,
      }])
      .expect("an answer");
    let body = MessageBody::new("The person answered:\n- Postgres").expect("a message");
    let message = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body, at(40));
    db.answer_questions(
      (run.id(), request.id()),
      &answers,
      (Answerer::Person, at(40)),
      Some(message.id()),
    )
    .expect("answered");
    db.send_message(IdempotencyKey::generate(), &message)
      .expect("the answer's message");
    seeded = Some((request.id().to_owned(), message.id()));
  });
  let (id, message): (String, MessageId) = seeded.expect("seeded");
  let card = format!("conversation-question-card-{id}");
  open_conversation(&mut desktop);
  desktop.wait_for(&card);
  assert!(shows(&mut desktop, &card, "Builder asked you"));
  assert!(shows(&mut desktop, &card, "Answered by you"));
  assert!(shows(&mut desktop, "conversation-question-answered-0", "Postgres"));
  assert!(!shows(&mut desktop, "conversation-question-answered-0", "SQLite"));
  assert!(
    !has(&mut desktop, &format!("conversation-message-{message}")),
    "no separate You message"
  );
}
