//! What holds the turns Orchester starts by itself (`orchestrator::held`, OA-03): while the project's daily budget is
//! used, or the account the Orchestrator runs on is at its plan limit, a wait is kept and starts no turn; its turn
//! starts at the reset, when the follower's clock wakes it; and a turn the runner refused for the budget is taken back
//! instead of failing.
//! `cargo test -p orchester-workspace --test orchestrator_held held`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  limits::ProjectLimits,
  name::Name,
  orchestrator::{Chat, EntryKind, Held, OrchestratorSettings, PlanHold, WaitTurn},
  resource::ModelId,
  run::{StepFailure, TokenUsage},
  store::{ConnectionStore, SettingsStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  held::held_in,
  turns::next_due_in,
  waits::{Followed, Switches, check_merges_in, follow_at},
};
use support::{Fixture, at};

const HOUR: i64 = 60 * 60 * 1000;

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on; the account it runs on.
fn ready(fixture: &mut Fixture) -> AgentAccountId {
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
  connection
    .account()
    .id(AgentProvider::ClaudeCodeCli)
    .expect("the connection's account")
}

/// A project whose Orchestrator is ready and whose one conversation leads everything, with a task's run that failed:
/// an event that starts a turn by itself. The conversation and the Orchestrator's account.
fn with_an_event() -> (Fixture, Chat, AgentAccountId) {
  let mut fixture = Fixture::new();
  let account = ready(&mut fixture);
  let chat = fixture.everything(at(10));
  let task = fixture.task("Add login", at(40));
  let mut run = fixture.started_run(task, at(50));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .fail_step(execution, StepFailure::ReportedFailed, Timestamp::now())
    .expect("failed");
  fixture.record(&run, &events);
  (fixture, chat, account)
}

/// Follows the waits at `at(now)`, the project's runner attached and the chat driving.
fn follow_then(fixture: &Fixture, now: i64) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let place = (root, fixture.db.project().id(), &programs);
  follow_at(place, &checks, (Switches { drives: true }, true), at(now)).expect("followed")
}

/// The turns of the waits `chat` keeps.
fn turns_in(fixture: &Fixture, chat: &Chat) -> Vec<Option<WaitTurn>> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { turn, .. } => Some(turn),
      _ => None,
    })
    .collect()
}

/// The project's daily budget is one token, and a turn of the Orchestrator spent more than that at `at(when)`.
fn use_up_the_budget(fixture: &mut Fixture, account: AgentAccountId, when: i64) {
  let tight = ProjectLimits::new(None, TokenBudget::new(1).expect("above zero"), None).expect("limits");
  fixture.db.save_project_limits(&tight).expect("limits");
  let spent = TokenUsage {
    input: 12,
    cached_input: 0,
    output: 27,
  };
  fixture
    .db
    .record_chat_usage(
      (ChatId::generate(), ChatMessageId::generate()),
      (account, spent),
      at(when),
    )
    .expect("usage");
}

#[test]
fn held_by_a_used_daily_budget_a_wait_is_kept_and_starts_its_turn_only_once_the_day_turned() {
  let (mut fixture, chat, account) = with_an_event();
  // Half past the hour: no time zone on a whole hour puts it at a local midnight.
  let now = 100 * HOUR + HOUR / 2;
  use_up_the_budget(&mut fixture, account, now - 1);
  let root = fixture.repo.path().to_path_buf();
  let Some(Held::Budget { until }) = held_in(&root, at(now)).expect("read") else {
    panic!("the budget holds");
  };
  let followed = follow_then(&fixture, now);
  assert!(followed.kept, "the event is kept");
  assert_eq!(followed.turn, None, "and starts no turn while the budget is used");
  assert_eq!(turns_in(&fixture, &chat), [None]);
  assert_eq!(
    next_due_in(&root, at(now)).expect("read"),
    Some(until),
    "the follower wakes when the day turns"
  );
  let next_day = until.unix_millis();
  assert_eq!(held_in(&root, at(next_day)).expect("read"), None, "a new day");
  let started = follow_then(&fixture, next_day)
    .turn
    .expect("the turn starts at the reset");
  assert_eq!(started.chat, chat.id());
}

#[test]
fn held_by_the_plan_limit_of_the_orchestrators_account_a_wait_starts_its_turn_at_the_reset() {
  let (mut fixture, chat, account) = with_an_event();
  let (now, resets) = (100 * HOUR, 103 * HOUR);
  let limit = PlanHold {
    since: at(now - 1),
    resets: Some(at(resets)),
  };
  fixture.db.hold_orchestrator(account, limit).expect("kept");
  let root = fixture.repo.path().to_path_buf();
  assert_eq!(
    held_in(&root, at(now)).expect("read"),
    Some(Held::PlanLimit { until: at(resets) })
  );
  assert_eq!(follow_then(&fixture, now).turn, None, "nothing starts at the limit");
  assert_eq!(next_due_in(&root, at(now)).expect("read"), Some(at(resets)));
  assert_eq!(follow_then(&fixture, resets - 1).turn, None, "not yet");
  let started = follow_then(&fixture, resets)
    .turn
    .expect("the turn starts at the reset");
  assert_eq!(started.chat, chat.id());
  assert_eq!(next_due_in(&root, at(resets)).expect("read"), None, "it thinks");
}

#[test]
fn held_by_nothing_of_another_account_or_once_the_account_answered_again() {
  let (mut fixture, _chat, account) = with_an_event();
  let now = 100 * HOUR;
  let limit = PlanHold {
    since: at(now - 1),
    resets: None,
  };
  fixture
    .db
    .hold_orchestrator(AgentAccountId::generate(), limit)
    .expect("kept");
  let root = fixture.repo.path().to_path_buf();
  assert_eq!(held_in(&root, at(now)).expect("read"), None, "another account's limit");
  fixture.db.hold_orchestrator(account, limit).expect("kept");
  assert!(held_in(&root, at(now)).expect("read").is_some());
  fixture.db.release_orchestrator(account).expect("released");
  assert_eq!(held_in(&root, at(now)).expect("read"), None);
  assert!(follow_then(&fixture, now).turn.is_some(), "its turn starts");
}

#[test]
fn held_turn_the_budget_refused_is_taken_back_and_starts_again_later() {
  let (mut fixture, chat, _account) = with_an_event();
  let now = 100 * HOUR;
  let started = follow_then(&fixture, now).turn.expect("the turn");
  // The runner refused it: today's budget was used meanwhile. The app takes the turn back.
  assert!(fixture.db.release_wait_turn(started.message).expect("taken back"));
  assert_eq!(
    turns_in(&fixture, &chat),
    [None],
    "the wait has no turn, and no failed one"
  );
  assert!(
    !fixture.db.release_wait_turn(started.message).expect("read"),
    "once taken back"
  );
  let again = follow_then(&fixture, now).turn.expect("it starts again");
  assert_ne!(again.message, started.message);
}
