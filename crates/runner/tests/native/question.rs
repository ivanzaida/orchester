//! A native agent's question with suggested answers (ORC-83): `ask_question` with options ends the step,
//! which waits; the question is kept for its card. A message written meanwhile does not resume the step;
//! the answer, given as the card gives it, reaches the agent as the call's result, and the run goes on.

use std::time::Duration;

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::IdempotencyKey,
  question::{GivenAnswer, QuestionState, QuestionTurn},
  run::{Answerer, RunStatus, StepStatus, WaitReason},
  store::{ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  calls, finish, native_project, serve,
  server::Script,
  support::{PATIENCE, scheduler_with, wait_for},
  tool_results,
};

#[tokio::test(flavor = "multi_thread")]
async fn native_question_with_options_waits_for_its_card_and_the_answer_reaches_the_agent() {
  let ask = json!({"question": "Where should the sign-in button go?", "header": "Docs layout", "multi_select": false,
    "options": [{"label": "Top of the page", "description": null}, {"label": "Under Sign in", "description": "Next to it"}]});
  let script = Script::new()
    .reply_sse(calls(&[("toolu_ask", "ask_question", ask)]))
    .expect(|request| {
      let results = tool_results(request);
      assert!(
        results
          .iter()
          .any(|result| result.contains("Under Sign in") && result.contains("Also keep it short.")),
        "{results:?}"
      );
    })
    .reply_sse(finish("toolu_finish"));
  let server = serve(script).await;
  let project = native_project("Ask the person.", None, &server);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| !run.awaiting_answer().is_empty()).await;
  let execution = run.awaiting_answer()[0];
  let db = ProjectDb::open(root).expect("open");
  let asked = db.questions(id).expect("questions").remove(0);
  assert_eq!(asked.request.turn(), QuestionTurn::StepEnd);
  assert_eq!(asked.request.execution(), execution);
  assert_eq!(
    asked.request.questions()[0].options()[1].description(),
    Some("Next to it")
  );

  // A message written while the question waits does not answer it: the step keeps waiting.
  let mut writer = ProjectDb::open(root).expect("open");
  let body = MessageBody::new("Also keep it short.").expect("a message");
  let note = Message::from_person(project.task, Some(id), Recipient::Orchestrator, body, Timestamp::now());
  writer.send_message(IdempotencyKey::generate(), &note).expect("stored");
  scheduler.deliver_messages(id).await.expect("delivered later");
  tokio::time::sleep(Duration::from_millis(300)).await;
  let still = db.run(id).expect("read").expect("run");
  assert_eq!(
    still.execution(execution).map(|step| step.status()),
    Some(StepStatus::Waiting(WaitReason::Message))
  );

  let several = vec![GivenAnswer {
    choices: vec!["Top of the page".to_owned(), "Under Sign in".to_owned()],
    text: None,
  }];
  let refused = scheduler
    .answer_question(id, asked.request.id().to_owned(), several, Answerer::Person)
    .await;
  assert!(matches!(refused, Err(Rejection::InvalidAnswer { .. })), "{refused:?}");
  let answer = vec![GivenAnswer {
    choices: vec!["Under Sign in".to_owned()],
    text: None,
  }];
  tokio::time::timeout(
    PATIENCE,
    scheduler.answer_question(id, asked.request.id().to_owned(), answer, Answerer::Person),
  )
  .await
  .expect("in time")
  .expect("answered");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = db
    .question_request(id, asked.request.id())
    .expect("read")
    .expect("kept");
  assert!(matches!(
    answered.state,
    QuestionState::Answered {
      by: Answerer::Person,
      ..
    }
  ));
  assert!(answered.message.is_some(), "the answer went to the agent in a message");
}
