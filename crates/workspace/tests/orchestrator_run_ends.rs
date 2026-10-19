//! The Orchestrator chat follows the ends of its project's runs (`orchestrator::waits`, `docs/design/
//! orchestrator-toolset.md`, decision 5): a task's latest run that succeeded, failed, stopped for a decision, stopped
//! at a limit or was interrupted is kept once, linking to the task's Summary, in the conversation that last dealt with
//! its task, else the latest one; ends from before the project followed them, a pause the person asked for and an
//! interruption the runner continues by itself are not kept; with *Let the Orchestrator answer agents* on an end starts
//! one Orchestrator turn by itself, with the prompt its end calls for, but none for an end the chat's own decision on
//! the run led to, and none while the switch is off.
//! `cargo test -p orchester-workspace --test orchestrator_run_ends run_ends`.

mod support;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, OrchestratorSettings, RunEnd, TurnState, WaitKind, WaitTurn,
    change::{Change, ContinueChoice, ProposedChange},
  },
  resource::ModelId,
  run::{InterruptCause, Run, RunLimit, StepFailure},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::waits::{Followed, follow_in};
use support::{Fixture, at};

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
      },
    })
    .expect("check");
  fixture.db.create_connection(&connection).expect("connection");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
  fixture
    .db
    .publish_orchestrator(&settings, Timestamp::now())
    .expect("published");
}

/// A run of task `title` at work.
fn working(fixture: &mut Fixture, title: &str) -> Run {
  let task = fixture.task(title, at(40));
  fixture.started_run(task, at(50))
}

/// `run`'s one step fails at `now` for `failure`.
fn fail(fixture: &mut Fixture, run: &mut Run, failure: StepFailure) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.fail_step(execution, failure, Timestamp::now()).expect("failed");
  fixture.record(run, &events);
}

fn project(fixture: &Fixture) -> ProjectId {
  fixture.db.project().id()
}

/// Follows the ends with the project's runner attached.
fn follow(fixture: &Fixture, answers: bool) -> Followed {
  follow_in(fixture.repo.path(), project(fixture), (answers, true)).expect("followed")
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

/// The waits of the project's latest conversation.
fn waits(fixture: &Fixture) -> Vec<(AgentWait, Option<WaitTurn>)> {
  match fixture.db.latest_chat().expect("read") {
    Some(chat) => waits_in(fixture, chat),
    None => Vec::new(),
  }
}

/// A conversation, started at `at(start)`, whose answered turn took `action`.
fn acted(fixture: &mut Fixture, action: &ChatAction, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Look after the runs", at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after the runs").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  fixture
    .db
    .add_chat_action(chat.id(), action, at(start + 1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 2))
    .expect("ended");
  chat
}

#[test]
fn run_ends_are_kept_once_in_the_conversation_that_dealt_with_their_task() {
  let mut fixture = Fixture::new();
  let mut done = working(&mut fixture, "Add login");
  let mut broken = working(&mut fixture, "Add logout");
  let dealt = acted(
    &mut fixture,
    &ChatAction::new("update_task", Some("ORC-1".to_owned()), ActionOutcome::Done),
    10,
  );
  let latest = acted(
    &mut fixture,
    &ChatAction::new("create_task", Some("ORC-9".to_owned()), ActionOutcome::Done),
    20,
  );
  fixture.succeed(&mut done, Timestamp::now());
  fail(&mut fixture, &mut broken, StepFailure::ReportedFailed);
  assert!(follow(&fixture, false).kept);
  let ready = waits_in(&fixture, dealt.id());
  assert_eq!(ready.len(), 1, "{ready:?}");
  let (wait, turn) = &ready[0];
  assert_eq!(wait.kind, WaitKind::Ended(RunEnd::Succeeded));
  assert_eq!((wait.task.as_str(), wait.number), ("ORC-1", 1));
  assert_eq!(wait.link, format!("/project/{}/tasks/ORC-1/summary", project(&fixture)));
  assert!(!wait.automatic && turn.is_none(), "the switch is off: {wait:?}");
  let failed = waits_in(&fixture, latest.id());
  assert_eq!(failed.len(), 1, "{failed:?}");
  assert_eq!(failed[0].0.kind, WaitKind::Ended(RunEnd::Failed));
  assert_eq!(failed[0].0.step.as_deref(), Some("Implement"), "the step it failed at");
  assert_eq!(follow(&fixture, false), Followed::default(), "each end is kept once");
}

#[test]
fn run_ends_name_how_the_run_stopped_and_leave_out_what_is_no_end() {
  let mut fixture = Fixture::new();
  let mut old = working(&mut fixture, "Old work");
  fixture.succeed(&mut old, at(60));
  let mut paused = working(&mut fixture, "Paused by the person");
  let events = paused.pause(Timestamp::now()).expect("pause asked");
  fixture.record(&paused, &events);
  let mut lost = working(&mut fixture, "Runner lost");
  let events = lost
    .interrupt(InterruptCause::RunnerLost, Timestamp::now())
    .expect("interrupted");
  fixture.record(&lost, &events);
  assert_eq!(
    follow(&fixture, false),
    Followed::default(),
    "history, a person's pause and a run the runner continues itself are no ends"
  );
  let mut limited = working(&mut fixture, "Spent its budget");
  let events = limited
    .stop_at_limit(RunLimit::Tokens, Timestamp::now())
    .expect("stopped");
  fixture.record(&limited, &events);
  let mut crashed = working(&mut fixture, "Crashed");
  let events = crashed
    .interrupt(InterruptCause::Crashed, Timestamp::now())
    .expect("interrupted");
  fixture.record(&crashed, &events);
  let mut unverified = working(&mut fixture, "Could not check");
  fail(&mut fixture, &mut unverified, StepFailure::Unverified);
  assert!(follow(&fixture, false).kept);
  let mut kept: Vec<(String, WaitKind)> = waits(&fixture)
    .into_iter()
    .map(|(wait, _)| (wait.title, wait.kind))
    .collect();
  kept.sort_by(|a, b| a.0.cmp(&b.0));
  assert_eq!(
    kept,
    vec![
      ("Could not check".to_owned(), WaitKind::Ended(RunEnd::Stopped)),
      ("Crashed".to_owned(), WaitKind::Ended(RunEnd::Interrupted)),
      (
        "Spent its budget".to_owned(),
        WaitKind::Ended(RunEnd::Limit {
          limit: RunLimit::Tokens
        })
      ),
    ]
  );
}

#[test]
fn run_ends_start_one_turn_with_their_prompt_while_answering_agents_is_on() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  fixture.succeed(&mut run, Timestamp::now());
  let started = follow(&fixture, true).turn.expect("the end starts a turn by itself");
  assert_eq!(follow(&fixture, true).turn, None, "one turn per end");
  let (wait, turn) = &waits(&fixture)[0];
  assert!(wait.automatic && !wait.own, "{wait:?}");
  assert_eq!(turn.as_ref().map(|turn| turn.message), Some(started.message));
  let prompt = fixture.db.chat_turn(started.message).expect("read").expect("the turn");
  for part in [
    "Run #1 of ORC-1 (Add login) succeeded: its work is ready to merge",
    "Merge it only if the person asked you earlier in this conversation",
  ] {
    assert!(
      prompt.text.as_str().contains(part),
      "{part} in {}",
      prompt.text.as_str()
    );
  }
  fixture
    .db
    .end_chat_turn(started.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  assert_eq!(follow(&fixture, true), Followed::default(), "the end stays one");
}

#[test]
fn run_ends_the_chats_own_decision_led_to_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  let retried = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add login".to_owned(),
      change: Change::Continue {
        run: run.id(),
        number: 1,
        attempt: 1,
        step: Some("Implement".to_owned()),
        choice: ContinueChoice::AllowRetry,
      },
    })),
    ..ChatAction::new("continue_run", Some("ORC-1".to_owned()), ActionOutcome::Applied)
  };
  let chat = acted(&mut fixture, &retried, 10);
  fail(&mut fixture, &mut run, StepFailure::ReportedFailed);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let (wait, turn) = &waits_in(&fixture, chat.id())[0];
  assert!(wait.own && !wait.automatic && turn.is_none(), "{wait:?}");
}

#[test]
fn run_end_turns_name_what_the_person_chooses_for_a_run_that_stopped() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  fail(&mut fixture, &mut run, StepFailure::Unverified);
  let started = follow(&fixture, true).turn.expect("a turn");
  let prompt = fixture.db.chat_turn(started.message).expect("read").expect("the turn");
  for part in [
    "Run #1 of ORC-1 (Add login) stopped at the step Implement and waits for a decision",
    "continue_run",
    "the person applies",
  ] {
    assert!(
      prompt.text.as_str().contains(part),
      "{part} in {}",
      prompt.text.as_str()
    );
  }
}
