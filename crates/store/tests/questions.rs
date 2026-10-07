//! Agents' questions with suggested answers (ORC-83): a request is kept with its run, step execution and
//! invocation, waits, is answered once (by whom, when, and the message a step-end answer went in), or ends
//! interrupted; and the migration that adds them keeps a database's runs.
//! `cargo test -p orchester-store --test questions questions`.
mod support;

use orchester_core::{
  id::MessageId,
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionState, QuestionTurn},
  run::{Answerer, Invocation},
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use support::{at, copied_at_version, model, started_run};

fn question(several: bool) -> Question {
  Question::new(
    "Where should the sign-in button go?",
    Some("Docs layout"),
    vec![
      QuestionOption::new("Top", Some("Before the first step")),
      QuestionOption::new("Bottom", None),
    ],
    several,
  )
  .expect("a question")
}

fn request(invocation: &Invocation, key: &str, turn: QuestionTurn) -> QuestionRequest {
  QuestionRequest::new(
    format!("{}:{key}", invocation.id),
    (invocation.run, invocation.execution, invocation.id),
    "Builder".to_owned(),
    vec![question(false), question(true)],
    (at(20), turn),
  )
  .expect("a request")
}

fn answers(request: &QuestionRequest) -> Vec<orchester_core::question::Answer> {
  request
    .answer(&[
      GivenAnswer {
        choices: vec!["Top".to_owned()],
        text: None,
      },
      GivenAnswer {
        choices: vec!["Bottom".to_owned(), "Top".to_owned()],
        text: Some("and in the footer".to_owned()),
      },
    ])
    .expect("answers")
}

#[test]
fn questions_wait_are_answered_once_and_keep_who_answered() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let asked = request(&invocation, "7", QuestionTurn::InTurn);
  db.ask_questions(&asked).expect("asked");
  assert!(db.ask_questions(&asked).is_err(), "an id is asked once in a run");
  let stored = db.questions(run.id()).expect("questions");
  assert_eq!(stored.len(), 1);
  assert_eq!(stored[0].request, asked);
  assert!(stored[0].waits());
  assert_eq!(db.waiting_questions().expect("waiting").len(), 1);

  let given = answers(&asked);
  let message = MessageId::generate();
  assert!(
    db.answer_questions(
      (run.id(), asked.id()),
      &given,
      (Answerer::Automation, at(30)),
      Some(message)
    )
    .expect("answered")
  );
  assert!(
    !db
      .answer_questions((run.id(), asked.id()), &given, (Answerer::Person, at(31)), None)
      .expect("not again"),
    "a request is answered once"
  );
  let answered = db.question_request(run.id(), asked.id()).expect("read").expect("kept");
  assert_eq!(
    answered.state,
    QuestionState::Answered {
      answers: given,
      by: Answerer::Automation,
      at: at(30),
    }
  );
  assert_eq!(answered.message, Some(message));
  assert!(db.waiting_questions().expect("waiting").is_empty());
}

#[test]
fn questions_in_a_turn_end_with_their_agent_and_step_end_ones_wait_on() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let in_turn = request(&invocation, "1", QuestionTurn::InTurn);
  let step_end = request(&invocation, "2", QuestionTurn::StepEnd);
  db.ask_questions(&in_turn).expect("asked");
  db.ask_questions(&step_end).expect("asked");
  assert!(db.interrupt_questions(run.id(), invocation.id).expect("interrupted"));
  assert!(!db.interrupt_questions(run.id(), invocation.id).expect("none left"));
  let stored = db.questions(run.id()).expect("questions");
  assert!(matches!(stored[0].state, QuestionState::Interrupted { .. }));
  assert!(stored[1].waits(), "a question that ended its step waits for the person");
  assert!(db.end_waiting_question(run.id(), step_end.id()).expect("ended"));
  assert!(!db.end_waiting_question(run.id(), step_end.id()).expect("not again"));

  let later = request(&invocation, "3", QuestionTurn::InTurn);
  db.ask_questions(&later).expect("asked");
  db.answer_questions(
    (run.id(), later.id()),
    &answers(&later),
    (Answerer::Person, at(40)),
    None,
  )
  .expect("answered");
  db.undelivered_questions(run.id(), later.id()).expect("undelivered");
  let undelivered = db.question_request(run.id(), later.id()).expect("read").expect("kept");
  assert!(matches!(undelivered.state, QuestionState::Interrupted { .. }));
  assert_eq!(undelivered.message, None);
}

fn schema_version(path: &std::path::Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

fn has_questions(connection: &rusqlite::Connection) -> bool {
  connection
    .prepare("SELECT 1 FROM sqlite_master WHERE name = 'questions'")
    .expect("schema")
    .exists([])
    .expect("schema")
}

/// The newest schema without questions, wherever later migrations moved the latest.
fn version_before_questions(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !has_questions(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before questions")
}

#[test]
fn questions_are_added_to_a_database_of_the_schema_before_and_its_runs_are_kept() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, version_before_questions(latest));
  let old_database = old.path().join(".orchester/orchester.db");
  assert!(
    !has_questions(&rusqlite::Connection::open(&old_database).expect("open")),
    "the schema before has no questions"
  );
  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old_database), latest);
  assert!(upgraded.questions(run.id()).expect("none yet").is_empty());
  let asked = request(&invocation, "1", QuestionTurn::StepEnd);
  upgraded.ask_questions(&asked).expect("a question of the kept run");
  assert_eq!(upgraded.questions(run.id()).expect("questions")[0].request, asked);
}
