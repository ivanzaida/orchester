//! An agent of the project waits for a message, and the Orchestrator follows it (ORC-42): the wait is kept in the
//! conversation and, with *Let the Orchestrator answer agents* on, starts one Orchestrator turn by itself, which the
//! runner answers to its end, however the conversation and the runs change meanwhile. The end of a run the chat
//! started shows in its conversation as a card that opens the task's Summary.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskView};
use orchester_core::{
  id::ChatMessageId,
  orchestrator::{EntryKind, RunEnd, TurnState, WaitKind},
  run::{StepStatus, WaitReason},
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::workspace::OrchestratorConsent;

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card},
};

/// The waits of the project's latest conversation, each with the message its turn was started under.
fn waits(db: &ProjectDb) -> Vec<(WaitKind, Option<ChatMessageId>)> {
  let Some(chat) = db.latest_chat().expect("read") else {
    return Vec::new();
  };
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait.kind, turn.map(|turn| turn.message))),
      _ => None,
    })
    .collect()
}

/// Whether the latest run of task `key` has a step waiting for a message.
fn waits_for_message(screen: &Orchestrator, key: &str) -> bool {
  let db = screen.db();
  let Some(task) = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
  else {
    return false;
  };
  db.runs_of_task(task.id())
    .expect("runs")
    .pop()
    .and_then(Result::ok)
    .and_then(|run| run.current_attempt().cloned())
    .is_some_and(|attempt| {
      attempt
        .executions()
        .iter()
        .any(|execution| execution.status() == StepStatus::Waiting(WaitReason::Message))
    })
}

#[test]
fn orchestrator_wait_turn_is_answered_to_its_end() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.let_orchestrator(OrchestratorConsent::AnswerAgents, true);
  let delivery = screen.delivery();
  let key = screen.draft_with(
    delivery,
    "Ask about sign-in",
    ("Ask first.", &["The question was asked."]),
  );
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the proposed start", |screen| {
    outcomes(screen).first().is_some_and(|outcome| outcome.is_proposed())
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(&screen);
  screen.until("its buttons", |screen| screen.exists(&format!("{card}-start")));
  screen.click(&format!("{card}-start"));
  screen.until("the step waiting for a message", |screen| {
    waits_for_message(screen, &key)
  });
  screen.until("the wait's own turn", |screen| {
    waits(&screen.db())
      .iter()
      .any(|(kind, turn)| *kind == WaitKind::Message && turn.is_some())
  });
  let turn = waits(&screen.db())
    .into_iter()
    .find_map(|(kind, turn)| turn.filter(|_| kind == WaitKind::Message))
    .expect("the wait's turn");
  // Keeping the turn told the conversations they changed, and so did its answer: the turn still ends answered.
  screen.until("the wait's turn answered", |screen| {
    let state = screen.db().chat_turn(turn).expect("read").map(|kept| kept.turn);
    !matches!(state, None | Some(TurnState::Thinking))
  });
  let kept = screen.db().chat_turn(turn).expect("read").expect("the turn");
  assert_eq!(kept.turn, TurnState::Answered, "{:?}", kept.turn);
  let messages = waits(&screen.db())
    .into_iter()
    .filter(|(kind, _)| *kind == WaitKind::Message)
    .count();
  assert_eq!(messages, 1, "the wait is kept once and starts one turn");
}

/// The ends of runs kept in the project's latest conversation, each by its entry.
fn ends(db: &ProjectDb) -> Vec<(i64, RunEnd)> {
  let Some(chat) = db.latest_chat().expect("read") else {
    return Vec::new();
  };
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } => wait.kind.run_end().map(|end| (entry.seq, end)),
      _ => None,
    })
    .collect()
}

#[test]
fn orchestrator_run_end_lands_in_the_conversation_that_started_the_run() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.auto_mode(true);
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Read the readme");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the run's end", |screen| !ends(&screen.db()).is_empty());
  let (seq, end) = ends(&screen.db())[0];
  assert_eq!(end, RunEnd::Succeeded);
  let card = format!("orchestrator-wait-wait-{seq}");
  screen.until("its line", |screen| screen.exists(&card));
  let shown = screen.text(&card);
  for part in [
    "Ready to merge".to_owned(),
    format!("{key} · #1"),
    "Review ↗".to_owned(),
  ] {
    assert!(shown.contains(&part), "{part} in {shown}");
  }
  // What happens with it is said once the line is opened.
  screen.click(&format!("{card}-line"));
  screen.until("what happens with it", |screen| {
    screen
      .text(&format!("{card}-reason"))
      .contains("It waits for you on the task's Summary")
  });
  screen.click(&format!("{card}-open"));
  assert!(
    matches!(screen.route(), Route::Project(_, ProjectPage::Task(task, TaskView::Summary)) if task.to_string() == key),
    "{:?}",
    screen.route()
  );
}
