//! A run that made no progress for the project's *A run without progress for* minutes raises one *no progress* event
//! (ORC-767, `orchestrator::stalls`): kept by the waits' follower at the time it is given, once per stall, re-armed by
//! progress, starting a turn only under *Let the Orchestrator answer agents*; never for a run that waits for a person,
//! is queued, paused or held, and never with the setting off.
//! `cargo test -p orchester-workspace --test orchestrator_stalls stalls`.

mod support;

use std::{collections::HashSet, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::{AgentMessage, Message, MessageBody},
  name::Name,
  orchestrator::{AgentWait, EntryKind, OrchestratorSettings, TaskEvent, TaskEventKind, WaitKind, WatchSettings},
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConnectionStore, ConversationStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{Borrow, Holder},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  stalls::next_stall_in,
  waits::{Followed, Switches, check_merges_in, follow_at},
};
use support::Fixture;

const MINUTE: i64 = 60_000;

/// `minutes` after the runs start.
fn minute(minutes: i64) -> Timestamp {
  Timestamp::from_unix_millis(1_000_000 + minutes * MINUTE)
}

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    orchester_core::id::ConnectionId::generate(),
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

/// A run of a new task titled `title`, started at minute 0, its one step at work.
fn running(fixture: &mut Fixture, title: &str) -> Run {
  let task = fixture.task(title, minute(-1));
  fixture.started_run(task, minute(0))
}

/// `run`'s agent is launched again at `at`: progress.
fn progress(fixture: &mut Fixture, run: &Run, at: Timestamp) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at)
    .expect("invocation");
}

/// Follows the waits at `now` with the project's runner attached, *Let the Orchestrator answer agents* on when
/// `answers`.
fn follow(fixture: &Fixture, answers: bool, now: Timestamp) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let switches = Switches {
    answers,
    ..Switches::default()
  };
  follow_at(
    (root, fixture.db.project().id(), &programs),
    &checks,
    (switches, true),
    now,
  )
  .expect("followed")
}

/// The no-progress events every conversation keeps, oldest first.
fn stalls(fixture: &Fixture) -> Vec<AgentWait> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  let mut kept = Vec::new();
  for listing in db.chats().expect("chats") {
    for entry in db.chat_entries(listing.chat.id()).expect("entries") {
      if let EntryKind::Wait { wait, .. } = entry.kind
        && matches!(wait.kind, WaitKind::Event(_))
      {
        kept.push(wait);
      }
    }
  }
  kept
}

fn stalled_since(since: Timestamp, at: Timestamp) -> WaitKind {
  WaitKind::Event(TaskEvent {
    kind: TaskEventKind::NoProgress { since, minutes: 20 },
    at,
  })
}

#[test]
fn stalls_raise_one_event_after_the_set_minutes_and_progress_rearms_it() {
  let mut fixture = Fixture::new();
  let run = running(&mut fixture, "Add login");
  assert_eq!(follow(&fixture, false, minute(19)), Followed::default(), "not yet");
  assert!(follow(&fixture, false, minute(20)).kept, "20 minutes without progress");
  let kept = stalls(&fixture);
  assert_eq!(kept.len(), 1, "{kept:?}");
  assert_eq!(kept[0].kind, stalled_since(minute(0), minute(20)));
  assert_eq!((kept[0].run, kept[0].task.as_str()), (run.id(), "ORC-1"));
  assert!(!kept[0].automatic, "answer agents is off: no turn");
  assert_eq!(
    follow(&fixture, false, minute(45)),
    Followed::default(),
    "one event per stall"
  );
  progress(&mut fixture, &run, minute(50));
  assert_eq!(
    follow(&fixture, false, minute(69)),
    Followed::default(),
    "progress re-armed it"
  );
  assert!(follow(&fixture, false, minute(70)).kept, "a new stall");
  let kept = stalls(&fixture);
  assert_eq!(kept.len(), 2, "{kept:?}");
  assert_eq!(kept[1].kind, stalled_since(minute(50), minute(70)));
}

#[test]
fn stalls_start_a_turn_only_under_answer_agents() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  running(&mut fixture, "Add login");
  let turn = follow(&fixture, true, minute(20))
    .turn
    .expect("the stall starts a turn by itself");
  let kept = stalls(&fixture);
  assert!(kept[0].automatic, "{kept:?}");
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  let entries = db.chat_entries(turn.chat).expect("entries");
  assert!(
    entries.iter().any(
      |entry| matches!(&entry.kind, EntryKind::Wait { turn: Some(started), .. }
      if started.message == turn.message)
    ),
    "the stall's own turn: {entries:?}"
  );
  assert_eq!(
    follow(&fixture, true, minute(30)),
    Followed::default(),
    "one turn per stall"
  );
}

#[test]
fn stalls_are_never_raised_for_waiting_queued_paused_or_held_runs() {
  let mut fixture = Fixture::new();
  let mut asking = running(&mut fixture, "Add login");
  let execution = asking.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(
      asking.id(),
      execution,
      &ModelId::new("gpt-5.5").expect("model"),
      minute(1),
    )
    .expect("invocation")
    .id;
  let question = Message::from_agent(
    asking.task(),
    asking.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("text"),
    minute(1),
  );
  fixture.db.add_agent_message(&question).expect("asked");
  let events = asking
    .wait_step(execution, WaitReason::Message, minute(2))
    .expect("waits");
  fixture.record(&asking, &events);
  let queued = fixture.task("Plan zoom", minute(-1));
  fixture.pending_run(queued, minute(0));
  let mut pausing = running(&mut fixture, "Add avatars");
  let events = pausing.request_pause(minute(3)).expect("asked to pause");
  fixture.record(&pausing, &events);
  let mut paused = running(&mut fixture, "Add logout");
  let events = paused.pause(minute(3)).expect("paused");
  fixture.record(&paused, &events);
  let holding = running(&mut fixture, "Add webhooks");
  let held = running(&mut fixture, "Add exports");
  let one = NonZeroU32::MIN;
  let took = fixture
    .db
    .borrow_workspace(Holder::Run { run: holding.id() }, None, one, minute(4));
  assert!(matches!(took, Ok(Borrow::Take(_))), "{took:?}");
  let waits = fixture
    .db
    .borrow_workspace(Holder::Run { run: held.id() }, None, one, minute(4));
  assert!(matches!(waits, Ok(Borrow::Wait(_))), "{waits:?}");
  follow(&fixture, false, minute(500));
  let kept = stalls(&fixture);
  let tasks: Vec<&str> = kept.iter().map(|wait| wait.task.as_str()).collect();
  assert_eq!(
    tasks,
    ["ORC-5"],
    "only the run an agent works on, on its workspace: {kept:?}"
  );
}

#[test]
fn stalls_are_not_raised_with_the_setting_off() {
  let mut fixture = Fixture::new();
  running(&mut fixture, "Add login");
  let off = WatchSettings::new("", None).expect("off");
  fixture.db.save_orchestrator_watch(&off).expect("saved");
  assert_eq!(follow(&fixture, true, minute(5_000)), Followed::default());
  assert!(stalls(&fixture).is_empty());
  assert_eq!(
    next_stall_in(fixture.repo.path(), minute(5_000)),
    Ok(None),
    "nothing to follow again for"
  );
}

#[test]
fn stalls_tell_when_following_again_may_raise_one() {
  let mut fixture = Fixture::new();
  assert_eq!(
    next_stall_in(fixture.repo.path(), minute(0)),
    Ok(None),
    "no agent works"
  );
  let run = running(&mut fixture, "Add login");
  let five = WatchSettings::new("", Some(5)).expect("five minutes");
  fixture.db.save_orchestrator_watch(&five).expect("saved");
  assert_eq!(next_stall_in(fixture.repo.path(), minute(2)), Ok(Some(minute(5))));
  assert!(follow(&fixture, false, minute(5)).kept);
  assert_eq!(
    next_stall_in(fixture.repo.path(), minute(6)),
    Ok(Some(minute(11))),
    "stalled already: the soonest it could stall again"
  );
  progress(&mut fixture, &run, minute(8));
  assert_eq!(next_stall_in(fixture.repo.path(), minute(9)), Ok(Some(minute(13))));
}
