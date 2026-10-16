//! What the Usage dashboard reads of a project, the plan limits history kept 90 days, and the
//! migrations that add them.
//! `cargo test -p orchester-store --test usage usage`.
mod support;

use orchester_core::{
  code_index::CodeIndexUse,
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
    stream::AgentEvent,
  },
  id::ConnectionId,
  revision::Revision,
  run::{EditProgress, FirstEdit, TokenUsage},
  store::{ActivityStore, ConnectionStore},
  task::organization::{ChangeActor, TaskOrganization},
  time::Timestamp,
  usage::{IndexGiven, RunOutcome, limits::RETENTION_DAYS},
};
use orchester_store::ProjectDb;
use support::{at, copied_at_version, model, name, started_run};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

/// The profile of [`started_run`] runs on a stored Codex connection.
fn codex_connection(repo: &std::path::Path, db: &mut ProjectDb) {
  let codex = Connection::new(
    ConnectionId::generate(),
    name("Codex"),
    ConnectionKind::Agent(AgentProvider::CodexCli),
    AgentEndpoint::Program(std::env::temp_dir().join("codex")),
    at(1),
  )
  .expect("absolute executable");
  db.create_connection(&codex).expect("stored");
  rusqlite::Connection::open(repo.join(".orchester/orchester.db"))
    .expect("open")
    .execute(
      "UPDATE connections SET id = (SELECT json_extract(body, '$.body.connection') FROM resource_versions \
       JOIN resources ON resources.id = resource_versions.resource_id WHERE resources.kind = 'agent_profile')",
      [],
    )
    .expect("the profile's connection");
}

#[test]
fn usage_facts_give_each_turn_with_its_invocation_and_run() {
  let (repo, mut db, run, execution) = started_run();
  codex_connection(repo.path(), &mut db);
  let tag = db.create_project_tag(name("auth")).expect("tag");
  db.set_task_organization(
    run.task(),
    &TaskOrganization::default().with_tags(vec![tag.id()]),
    ChangeActor::Person,
    at(8),
  )
  .expect("tagged");
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let revision = Revision::new(&"b".repeat(40)).expect("revision");
  db.save_invocation_code_index(
    invocation.id,
    &CodeIndexUse::Given {
      revision,
      ripgrep: None,
    },
  )
  .expect("index");
  let turns = [
    (at(20), usage(1_000, 600, 200)),
    (at(5 * DAY), usage(3_000, 1_000, 500)),
  ];
  let events: Vec<_> = turns
    .iter()
    .map(|(at, usage)| (*at, AgentEvent::TurnCompleted { usage: *usage }))
    .collect();
  db.append_activity(&invocation, &events).expect("transcript");
  let progress = EditProgress {
    before: usage(1_000, 600, 200),
    first_edit: Some(FirstEdit {
      at: at(30),
      path: "src/lib.rs".into(),
    }),
    at_most: false,
  };
  db.save_edit_progress(invocation.id, &progress).expect("progress");
  let account = orchester_core::id::AgentAccountId::generate();
  db.save_invocation_account(invocation.id, account).expect("account");
  invocation.ended_at = Some(at(5 * DAY + 10));
  invocation.outcome = Some(orchester_core::connection::stream::InvocationOutcome::Completed);
  db.save_invocation(&invocation).expect("ended");

  let facts = db.usage_facts(at(DAY), at(6 * DAY)).expect("facts");
  assert_eq!(facts.turns.len(), 1, "a turn before the start is left out");
  assert_eq!(facts.turns[0].usage, usage(3_000, 1_000, 500));
  assert_eq!(facts.turns[0].invocation, invocation.id);
  assert_eq!(facts.last_turn, Some(at(5 * DAY)));
  let [read] = facts.invocations.as_slice() else {
    panic!("the invocation that ran since: {:?}", facts.invocations);
  };
  assert_eq!(read.cli, Some(AgentProvider::CodexCli));
  assert_eq!(read.model.as_deref(), Some(model().as_str()));
  assert_eq!(read.profile.as_deref(), Some("Builder"));
  assert!(read.profile_id.is_some(), "the profile's resource");
  assert_eq!(read.account, Some(account));
  assert_eq!(&*read.step, "Implement", "the step's name, not its key");
  assert_eq!(read.index, IndexGiven::Yes);
  assert_eq!(read.edit.as_ref(), Some(&progress));
  assert_eq!(read.active_ms, u64::try_from(5 * DAY).expect("positive"));
  let [run_facts] = facts.runs.as_slice() else {
    panic!("the invocation's run: {:?}", facts.runs);
  };
  assert_eq!(run_facts.id, run.id());
  assert_eq!(&*run_facts.task_key, "ORC-1");
  assert_eq!(&*run_facts.task_title, "Say hello");
  assert_eq!(run_facts.pipeline.as_deref(), Some("Delivery"));
  assert!(run_facts.pipeline_id.is_some(), "the pipeline's resource");
  assert_eq!(
    run_facts.pipeline_version.map(|version| version.get()),
    Some(1),
    "the version the run pinned"
  );
  assert_eq!(run_facts.tags, ["auth".into()]);
  assert_eq!(run_facts.outcome, RunOutcome::Running);
}

#[test]
fn usage_facts_leave_out_invocations_that_ended_before_the_start_and_keep_running_ones() {
  let (_repo, mut db, run, execution) = started_run();
  let mut ended = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("first");
  ended.ended_at = Some(at(20));
  ended.outcome = Some(orchester_core::connection::stream::InvocationOutcome::Completed);
  db.save_invocation(&ended).expect("ended");
  let running = db
    .start_invocation(run.id(), execution, &model(), at(30))
    .expect("second");

  let facts = db.usage_facts(at(DAY), at(2 * DAY)).expect("facts");
  let ids: Vec<_> = facts.invocations.iter().map(|invocation| invocation.id).collect();
  assert_eq!(ids, [running.id]);
  assert_eq!(facts.invocations[0].cli, None, "the profile's connection is not stored");
  assert_eq!(facts.invocations[0].index, IndexGiven::Unknown);
  assert_eq!(
    facts.invocations[0].active_ms,
    u64::try_from(2 * DAY - 30).expect("positive")
  );
  assert_eq!(facts.last_turn, None);
}

fn reading(provider: AgentProvider, at: Timestamp, percent: f64) -> LimitReading {
  LimitReading {
    provider,
    account: None,
    at,
    limits: PlanLimits {
      windows: vec![
        LimitWindow::new(
          10_080,
          Some(percent),
          Some(Timestamp::from_unix_millis(at.unix_millis() + DAY)),
          LimitState::Normal,
          false,
        )
        .expect("window"),
      ],
      plan: Some("pro".into()),
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn usage_limit_snapshots_are_kept_with_their_run_for_90_days() {
  let (_repo, mut db, run, _) = started_run();
  let old = reading(AgentProvider::CodexCli, at(DAY), 12.5);
  db.save_limit_snapshot(run.id(), &old).expect("old report");
  let recent = reading(AgentProvider::ClaudeCodeCli, at(2 * DAY), 41.0);
  db.save_limit_snapshot(run.id(), &recent).expect("recent report");
  let history = db.limit_snapshots(at(0)).expect("history");
  assert_eq!(history.len(), 2);
  assert_eq!(history[0].limits, old.limits);
  assert_eq!(history[1].provider, AgentProvider::ClaudeCodeCli);
  let source = history[1].source.as_ref().expect("the run");
  assert_eq!(
    (&*source.project, &*source.task_key, source.run),
    ("Kontur", "ORC-1", run.number())
  );
  assert_eq!(db.limit_snapshots(at(2 * DAY)).expect("since").len(), 1);

  let later = reading(AgentProvider::CodexCli, at((RETENTION_DAYS + 1) * DAY + 1), 50.0);
  db.save_limit_snapshot(run.id(), &later).expect("a report 91 days on");
  let kept: Vec<_> = db
    .limit_snapshots(at(0))
    .expect("history")
    .into_iter()
    .map(|snapshot| snapshot.at)
    .collect();
  assert_eq!(kept, [at(2 * DAY), later.at], "reports older than 90 days are removed");
}

fn schema_version(path: &std::path::Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

#[test]
fn usage_history_is_added_to_a_database_of_the_schema_before_and_its_runs_are_kept() {
  let (repo, db, run, _) = started_run();
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, version_before_plan_limits(latest));
  let mut db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old.path().join(".orchester/orchester.db")), latest);
  assert!(db.limit_snapshots(at(0)).expect("an empty history").is_empty());
  db.save_limit_snapshot(run.id(), &reading(AgentProvider::CodexCli, at(DAY), 30.0))
    .expect("store a report on the kept run");
  assert_eq!(db.limit_snapshots(at(0)).expect("history").len(), 1);
}

/// The newest schema without the plan limits history, wherever later migrations moved the latest.
fn version_before_plan_limits(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      let old = ProjectDb::create_at_version(repo.path(), version).expect("an old database");
      !old
        .prepare("SELECT 1 FROM sqlite_master WHERE name = 'plan_limit_snapshots'")
        .expect("schema")
        .exists([])
        .expect("schema")
    })
    .expect("a schema before the plan limits history")
}
