//! The Orchestrator's end-of-day report (ORC-47, `orchestrator::report`), built from what the project's database
//! holds: a busy day tells what was merged, how the latest runs stand, what waits for the person, what the
//! Orchestrator did (itself or applied by the person) and the day's tokens against the daily budget, each with its
//! link; an empty day gets no report; a day split by app restarts is told whole and comes on the next start, labelled
//! with its day; one report per day; `get_daily_report` reads the same report; with *Let the Orchestrator answer
//! agents* on, the report starts one Orchestrator turn by itself that only proposes; work git proves merged without
//! Orchester is not ready ([`outside`]).
//! `cargo test -p orchester-workspace --test orchestrator_report report`.

mod outside;
#[path = "../support/mod.rs"]
mod support;

use std::collections::HashSet;

use chrono::{Days, NaiveDate};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    stream::{AgentEvent, InvocationOutcome},
  },
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  integration::{MergeKind, RunMerge},
  name::Name,
  orchestrator::{
    ActionOutcome, Chat, ChatAction, DailyReport, EntryKind, OrchestratorSettings, ReportSchedule, ReportTime, RunEnd,
    TurnState,
    change::{Change, ProposedChange},
    report::{ActionWay, MergedBy, PersonWaitKind, SharedBy},
  },
  resource::ModelId,
  revision::Revision,
  run::{BranchName, InterruptCause, Run, StepFailure, TokenUsage},
  store::{ActivityStore, ConnectionStore, MergeStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  report::{CATCH_UP_DAYS, Delivery, build_in, deliver_in, due_days, report_in},
  waits::{Switches, check_merges_in, follow_in},
};
use support::Fixture;

/// The day the tests tell, a Tuesday.
fn day() -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 10, 20).expect("a date")
}

/// `hour`:`minute` local time on `day`.
fn on(day: NaiveDate, hour: u16, minute: u16) -> Timestamp {
  ReportTime::new(hour * 60 + minute).expect("a time of day").on(day)
}

/// `hour`:00 local time on the day the tests tell.
fn at(hour: u16) -> Timestamp {
  on(day(), hour, 0)
}

fn next_day() -> NaiveDate {
  day().checked_add_days(Days::new(1)).expect("a date")
}

/// A run of a new task `title`, started at `started`.
fn started(fixture: &mut Fixture, title: &str, started: Timestamp) -> Run {
  let task = fixture.task(title, started);
  fixture.started_run(task, started)
}

/// `run`'s one step fails at `now`.
fn fail(fixture: &mut Fixture, run: &mut Run, now: Timestamp) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .fail_step(execution, StepFailure::ReportedFailed, now)
    .expect("failed");
  fixture.record(run, &events);
}

/// Orchester merges `run` at `now`.
fn merge(fixture: &mut Fixture, run: &Run, now: Timestamp) {
  fixture
    .db
    .save_run_merge(&RunMerge {
      run: run.id(),
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&"a".repeat(40)).expect("revision"),
      kind: MergeKind::FastForward,
      merged_at: now,
    })
    .expect("merged");
}

/// `run`'s agent spent `output` tokens on `account` in a turn from `from` to `to`.
fn spend(fixture: &mut Fixture, run: &Run, account: AgentAccountId, (from, to): (Timestamp, Timestamp), output: u64) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let mut invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), from)
    .expect("invocation");
  let usage = TokenUsage {
    input: 0,
    cached_input: 0,
    output,
  };
  fixture
    .db
    .append_activity(&invocation, &[(to, AgentEvent::TurnCompleted { usage })])
    .expect("turn");
  fixture
    .db
    .save_invocation_account(invocation.id, account)
    .expect("account");
  invocation.ended_at = Some(to);
  invocation.outcome = Some(InvocationOutcome::Completed);
  fixture.db.save_invocation(&invocation).expect("ended");
}

/// A conversation started at `start` by the person's message, whose answered turn took `actions`, each kept at its
/// time.
fn conversation(fixture: &mut Fixture, start: Timestamp, actions: &[(ChatAction, Timestamp)]) -> ChatId {
  let chat = Chat::named(ChatId::generate(), "Look after the runs", start);
  let text = MessageBody::new("Look after the runs").expect("text");
  let message = ChatMessageId::generate();
  fixture
    .db
    .keep_chat_message(&chat, (message, &text), start)
    .expect("kept");
  for (action, kept) in actions {
    fixture.db.add_chat_action(chat.id(), action, *kept).expect("kept");
  }
  let ended = actions.last().map_or(start, |(_, kept)| *kept);
  fixture
    .db
    .end_chat_turn(message, &TurnState::Answered, ended)
    .expect("answered");
  chat.id()
}

fn action(tool: &str, task: &str, outcome: ActionOutcome, settled: Option<Timestamp>) -> ChatAction {
  ChatAction {
    settled,
    ..ChatAction::new(tool, Some(task.to_owned()), outcome)
  }
}

/// The report of the day the tests tell, read at `now`.
fn report(fixture: &Fixture, now: Timestamp) -> DailyReport {
  let programs = Programs::processes();
  build_in((fixture.repo.path(), &programs), (day(), now)).expect("built")
}

/// Delivers the report of `day` at `now`, *answer agents* as `answers` says.
fn deliver(fixture: &Fixture, (day, now): (NaiveDate, Timestamp), answers: bool) -> Delivery {
  let programs = Programs::processes();
  deliver_in((fixture.repo.path(), &programs), answers, (day, now), "Daily report").expect("delivered")
}

/// The reports the project's conversations hold, with whether each started its turn.
fn kept_reports(fixture: &Fixture) -> Vec<(DailyReport, bool)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chats()
    .expect("chats")
    .into_iter()
    .flat_map(|listing| db.chat_entries(listing.chat.id()).expect("entries"))
    .filter_map(|entry| match entry.kind {
      EntryKind::Report { report, turn } => Some((*report, turn.is_some())),
      _ => None,
    })
    .collect()
}

/// A busy day: a task merged, a run ready, one failed, one at work with a question for the person, a queued task, the
/// Orchestrator's own message and the person's applied proposal, a proposal still waiting, and tokens spent.
fn busy_day(fixture: &mut Fixture) {
  let mut merged = started(fixture, "Sign in", at(8));
  fixture.succeed(&mut merged, at(9));
  merge(fixture, &merged, at(10));
  let mut ready = started(fixture, "Delivery log", at(9));
  fixture.succeed(&mut ready, at(11));
  let mut failed = started(fixture, "Export", at(9));
  fail(fixture, &mut failed, at(12));
  let working = started(fixture, "Search", at(13));
  spend(fixture, &working, AgentAccountId::generate(), (at(13), at(14)), 4_000);
  fixture.queued_task("Settings page", at(7));
  conversation(
    fixture,
    at(8),
    &[
      (action("send_message", "ORC-4", ActionOutcome::Done, None), at(14)),
      (
        action("merge_run", "ORC-1", ActionOutcome::Applied, Some(at(10))),
        at(9),
      ),
      (action("cancel_run", "ORC-3", ActionOutcome::Waits, None), at(15)),
      (
        action("send_message", "ORC-2", ActionOutcome::Done, None),
        on(day().pred_opt().expect("a date"), 16, 0),
      ),
    ],
  );
}

#[test]
fn report_of_a_busy_day_tells_what_was_done_and_how_the_work_stands() {
  let mut fixture = Fixture::new();
  busy_day(&mut fixture);
  let project = fixture.db.project().id();
  let read = report(&fixture, at(18));

  assert_eq!(read.day, day());
  let merged: Vec<(&str, u32)> = read
    .merged
    .iter()
    .map(|task| (task.task.as_str(), task.number))
    .collect();
  assert_eq!(merged, [("ORC-1", 1)]);
  assert_eq!(read.merged[0].link, format!("/project/{project}/tasks/ORC-1/summary"));
  assert_eq!(
    read.ready.iter().map(|run| run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-2"]
  );
  assert_eq!(read.ready[0].since, Some(at(11)), "since it succeeded");
  let [stopped] = read.stopped.as_slice() else {
    panic!("the failed run alone: {:?}", read.stopped);
  };
  assert_eq!((stopped.run.task.as_str(), stopped.end), ("ORC-3", RunEnd::Failed));
  assert_eq!(stopped.run.step.as_deref(), Some("Implement"), "the step it failed at");
  assert_eq!(
    read.working.iter().map(|run| run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-4"]
  );
  assert_eq!(
    read.queued.iter().map(|task| task.task.as_str()).collect::<Vec<_>>(),
    ["ORC-5"]
  );

  let ways: Vec<(&str, ActionWay)> = read
    .actions
    .iter()
    .map(|action| (action.tool.as_str(), action.way))
    .collect();
  assert_eq!(
    ways,
    [("merge_run", ActionWay::Applied), ("send_message", ActionWay::Direct)],
    "the person's application at when they applied it, the Orchestrator's own message; yesterday's is not the day's"
  );
  assert_eq!(read.actions[1].link, format!("/project/{project}/tasks/ORC-4/summary"));
  let [proposal] = read.waiting.as_slice() else {
    panic!("the waiting proposal alone: {:?}", read.waiting);
  };
  assert_eq!(proposal.kind, PersonWaitKind::Proposal);
  assert!(
    proposal
      .link
      .starts_with(&format!("/project/{project}/orchestrator?chat=")),
    "{}",
    proposal.link
  );

  assert_eq!(read.usage.spent, 4_000);
  assert_eq!(read.usage.active_ms, 60 * 60 * 1000, "one hour of agent time");
  assert_eq!(read.usage.budget, Some(20_000_000), "against the default daily budget");
  assert_eq!(read.usage.tasks.len(), 1);
  assert_eq!(
    (read.usage.tasks[0].task.as_str(), read.usage.tasks[0].spent),
    ("ORC-4", 4_000)
  );
  let [provider] = read.usage.providers.as_slice() else {
    panic!("the day's one provider: {:?}", read.usage.providers);
  };
  // The fixture's profile names a connection the project does not keep: it reads as a removed provider.
  assert!(
    matches!(
      &provider.by,
      SharedBy::Provider {
        name: None,
        removed: true,
        ..
      }
    ),
    "the connection the agent's profile names, never No account: {provider:?}"
  );
  assert_eq!(provider.spent, 4_000);
  assert!(provider.link.contains("provider="), "{}", provider.link);
  assert!(
    read.usage.link.contains("from=2026-10-20&to=2026-10-20"),
    "{}",
    read.usage.link
  );
}

/// The chat's merge of `run`, carried out itself, on its own when `on_its_own` (ORC-69).
fn merged_by_chat(run: &Run, on_its_own: bool) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Sign in".to_owned(),
      change: Change::Merge {
        run: run.id(),
        number: run.number().get(),
        checkpoint: Some("a".repeat(40)),
        into: Some("main".to_owned()),
      },
    })),
    on_its_own,
    ..ChatAction::new("merge_run", None, ActionOutcome::Done)
  }
}

#[test]
fn report_names_the_orchestrator_as_who_merged_what_it_merged_itself() {
  let mut fixture = Fixture::new();
  let mut on_its_own = started(&mut fixture, "Sign in", at(8));
  fixture.succeed(&mut on_its_own, at(9));
  merge(&mut fixture, &on_its_own, at(10));
  let mut asked = started(&mut fixture, "Sign out", at(8));
  fixture.succeed(&mut asked, at(9));
  merge(&mut fixture, &asked, at(11));
  let mut by_person = started(&mut fixture, "Delivery log", at(8));
  fixture.succeed(&mut by_person, at(9));
  merge(&mut fixture, &by_person, at(12));
  conversation(
    &mut fixture,
    at(8),
    &[
      (merged_by_chat(&on_its_own, true), at(10)),
      (merged_by_chat(&asked, false), at(11)),
    ],
  );
  let read = report(&fixture, at(18));
  let by: Vec<(&str, MergedBy)> = read.merged.iter().map(|task| (task.task.as_str(), task.by)).collect();
  assert_eq!(
    by,
    [
      ("ORC-1", MergedBy::OrchestratorOnItsOwn),
      ("ORC-2", MergedBy::Orchestrator),
      ("ORC-3", MergedBy::Person),
    ]
  );
}

#[test]
fn report_of_an_empty_day_is_not_delivered() {
  let mut fixture = Fixture::new();
  let yesterday = day().pred_opt().expect("a date");
  let mut ready = started(&mut fixture, "Delivery log", on(yesterday, 9, 0));
  fixture.succeed(&mut ready, on(yesterday, 11, 0));
  fixture.queued_task("Settings page", on(yesterday, 12, 0));

  let read = report(&fixture, at(18));
  assert!(read.merged.is_empty() && read.actions.is_empty() && read.usage.spent == 0);
  assert_eq!(read.ready.len(), 1, "what stands still stands");
  assert_eq!(deliver(&fixture, (day(), at(18)), false), Delivery::Quiet);
  assert!(
    kept_reports(&fixture).is_empty(),
    "a day without activity gets no report"
  );
}

#[test]
fn report_of_a_day_split_by_restarts_is_told_whole_on_the_next_start() {
  let mut fixture = Fixture::new();
  // The morning's session: the Orchestrator acts, then the app closes and interrupts the run.
  let mut continued = started(&mut fixture, "Search", at(8));
  conversation(
    &mut fixture,
    at(8),
    &[(action("send_message", "ORC-1", ActionOutcome::Done, None), at(9))],
  );
  let events = continued
    .interrupt(InterruptCause::QuitNow, at(11))
    .expect("interrupted");
  fixture.record(&continued, &events);
  // The afternoon's session: the run goes on, another conversation acts, another run fails.
  let events = continued.continue_run(at(14)).expect("continued");
  fixture.record(&continued, &events);
  conversation(
    &mut fixture,
    at(14),
    &[(action("send_message", "ORC-2", ActionOutcome::Done, None), at(15))],
  );
  let mut failed = started(&mut fixture, "Export", at(14));
  fail(&mut fixture, &mut failed, at(16));

  // The app was closed at the report's time, 18:00, and starts again the next morning.
  let schedule = ReportSchedule {
    time: ReportTime::DEFAULT,
    since: on(day(), 7, 0),
  };
  let next_start = on(next_day(), 8, 0);
  assert_eq!(
    due_days(schedule, next_start),
    [day()],
    "the day missed, and not yet the new one"
  );
  assert_eq!(deliver(&fixture, (day(), next_start), false), Delivery::Kept);

  let [(kept, turned)] = kept_reports(&fixture).try_into().expect("one report");
  assert!(!turned, "no turn while answer agents is off");
  assert_eq!(kept.day, day(), "labelled with its day");
  assert_eq!(kept.built, next_start);
  let tasks: Vec<&str> = kept
    .actions
    .iter()
    .filter_map(|action| action.task.as_deref())
    .collect();
  assert_eq!(tasks, ["ORC-1", "ORC-2"], "both sessions' actions, in order");
  assert_eq!(
    kept.working.iter().map(|run| run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-1"]
  );
  assert_eq!(
    kept.stopped.iter().map(|run| run.run.task.as_str()).collect::<Vec<_>>(),
    ["ORC-2"]
  );
}

#[test]
fn report_comes_once_a_day_and_get_daily_report_reads_the_same() {
  let mut fixture = Fixture::new();
  busy_day(&mut fixture);
  let in_repo = (fixture.repo.path(), &Programs::processes());
  let (built, delivered) = report_in(in_repo, (day(), at(18))).expect("read");
  assert!(!delivered, "built while the day has no report");
  assert_eq!(built, report(&fixture, at(18)));

  assert_eq!(deliver(&fixture, (day(), at(18)), false), Delivery::Kept);
  assert_eq!(
    deliver(&fixture, (day(), at(19)), false),
    Delivery::Known,
    "one report per day"
  );
  let [(kept, _)] = kept_reports(&fixture).try_into().expect("one report");
  assert_eq!(kept, built);
  let (read, delivered) = report_in(in_repo, (day(), at(21))).expect("read");
  assert!(delivered);
  assert_eq!(read, kept, "the report as it was delivered");
}

#[test]
fn report_days_due_follow_the_time_since_it_was_turned_on() {
  let schedule = |since: Timestamp| ReportSchedule {
    time: ReportTime::DEFAULT,
    since,
  };
  let before_time = on(day(), 17, 59);
  assert!(
    due_days(schedule(on(day(), 9, 0)), before_time).is_empty(),
    "not before its time"
  );
  assert_eq!(due_days(schedule(on(day(), 9, 0)), at(18)), [day()]);
  assert!(
    due_days(schedule(on(day(), 19, 0)), on(day(), 20, 0)).is_empty(),
    "turned on after the day's time: tomorrow's comes"
  );
  let long_ago = on(NaiveDate::from_ymd_opt(2026, 9, 1).expect("a date"), 9, 0);
  let due = due_days(schedule(long_ago), on(next_day(), 8, 0));
  assert_eq!(
    due.len(),
    usize::try_from(CATCH_UP_DAYS - 1).expect("small"),
    "today's time has not come"
  );
  assert_eq!(due.last(), Some(&day()));
  assert!(due.windows(2).all(|pair| pair[0] < pair[1]), "oldest first: {due:?}");
}

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

#[test]
fn report_with_answer_agents_on_starts_one_turn_that_only_proposes() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  busy_day(&mut fixture);
  assert_eq!(deliver(&fixture, (day(), at(18)), true), Delivery::Kept);
  let [(kept, _)] = kept_reports(&fixture).try_into().expect("one report");
  assert!(kept.automatic, "it starts a turn by itself");

  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let switches = Switches {
    answers: true,
    ..Switches::default()
  };
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let project = fixture.db.project().id();
  let followed = follow_in((root, project, &programs), &checks, (switches, true)).expect("followed");
  let turn = followed.turn.expect("the report's turn");
  let prompt = fixture
    .db
    .chat_turn(turn.message)
    .expect("read")
    .expect("the turn")
    .text;
  for part in [
    "[Orchester] The project's daily report for 2026-10-20",
    "get_daily_report",
    "Change nothing",
  ] {
    assert!(prompt.as_str().contains(part), "{part} in {}", prompt.as_str());
  }
  let again = follow_in((root, project, &programs), &checks, (switches, true)).expect("followed");
  assert_ne!(
    again.turn.map(|turn| turn.message),
    Some(turn.message),
    "one turn per report"
  );
  assert!(
    kept_reports(&fixture).iter().all(|(_, turned)| *turned),
    "the report owns its turn"
  );
}
