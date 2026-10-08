//! Questions with suggested answers (ORC-83), end to end. Claude Code: the relay lists Orchester's
//! `ask_question` beside the step's tools, a call of it waits in the agent's turn as a recorded question
//! while the agent's time stops counting, a wrong answer is refused, the answer comes back as the call's
//! result and the run goes on; a second answer is refused. Codex: one `requestUserInput` with two questions
//! waits in its turn, answers for only one are refused, and both answers reach it in its response format.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test questions questions`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  question::{AskedQuestions, GivenAnswer, QuestionState, QuestionTurn},
  run::{Answerer, RunStatus},
  store::ActivityStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, project, reported, scheduler_with, wait_for};

async fn waiting_question(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> AskedQuestions {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(asked) = db
        .questions(run)
        .expect("questions")
        .into_iter()
        .find(AskedQuestions::waits)
      {
        return asked;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("a question arrives")
}

#[tokio::test(flavor = "multi_thread")]
async fn questions_from_claude_code_wait_in_its_turn_and_the_answer_reaches_it() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    )),
    ..Setup::new("Ask with options, several allowed.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let asked = waiting_question(root, &mut events, id).await;
  assert_eq!(asked.request.turn(), QuestionTurn::InTurn);
  assert_eq!(asked.request.agent(), "Builder");
  let question = &asked.request.questions()[0];
  assert_eq!(question.header(), Some("Docs layout"));
  assert!(question.multi_select());
  let db = ProjectDb::open(root).expect("open");
  let invocation = db
    .invocations(id)
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.id == asked.request.invocation())
    .expect("the asking invocation");
  assert!(
    invocation.timing.waiting_since().is_some(),
    "the agent's time stops counting"
  );

  let unknown = vec![GivenAnswer {
    choices: vec!["Nowhere".to_owned()],
    text: None,
  }];
  let refused = scheduler
    .answer_question(id, asked.request.id().to_owned(), unknown, Answerer::Person)
    .await;
  assert!(matches!(refused, Err(Rejection::InvalidAnswer { .. })), "{refused:?}");
  let answer = vec![GivenAnswer {
    choices: vec!["Under Sign in".to_owned(), "In the sidebar".to_owned()],
    text: Some("and in the footer".to_owned()),
  }];
  scheduler
    .answer_question(id, asked.request.id().to_owned(), answer.clone(), Answerer::Person)
    .await
    .expect("answered");
  let again = scheduler
    .answer_question(id, asked.request.id().to_owned(), answer, Answerer::Person)
    .await;
  assert_eq!(again, Err(Rejection::QuestionAnswered));

  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let listed = reported(&db, id, "orchester-tools: ");
  assert!(
    listed.first().is_some_and(|tools| tools.ends_with(",ask_question")),
    "{listed:?}"
  );
  assert_eq!(
    reported(&db, id, "orchester-answer: "),
    ["ok The person answered: | - Under Sign in | - In the sidebar | - In their own words: and in the footer"]
  );
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
}

#[tokio::test(flavor = "multi_thread")]
async fn questions_from_codex_wait_in_its_turn_and_each_answer_reaches_it() {
  let project = project(&Setup::new("Ask Codex with options."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let asked = waiting_question(root, &mut events, id).await;
  assert_eq!(asked.request.turn(), QuestionTurn::InTurn);
  let [layout, tone] = asked.request.questions() else {
    panic!("two questions in one request");
  };
  assert_eq!(layout.header(), Some("Docs layout"));
  assert_eq!(layout.options()[1].description(), None);
  assert_eq!(tone.options()[1].description(), Some("Warmer"));
  assert!(!layout.multi_select() && !tone.multi_select());

  let one = vec![GivenAnswer {
    choices: vec!["Top of the page".to_owned()],
    text: None,
  }];
  let refused = scheduler
    .answer_question(id, asked.request.id().to_owned(), one, Answerer::Person)
    .await;
  assert!(matches!(refused, Err(Rejection::InvalidAnswer { .. })), "{refused:?}");
  let both = vec![
    GivenAnswer {
      choices: Vec::new(),
      text: Some("In the middle".to_owned()),
    },
    GivenAnswer {
      choices: vec!["Friendly".to_owned()],
      text: None,
    },
  ];
  scheduler
    .answer_question(id, asked.request.id().to_owned(), both, Answerer::Person)
    .await
    .expect("answered");

  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "codex-answer: "),
    [r#"{"layout":{"answers":["In the middle"]},"tone":{"answers":["Friendly"]}}"#]
  );
}
