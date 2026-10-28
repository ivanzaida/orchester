//! What auto mode's reviewer spent: one record per review, linked to its run, step execution, invocation
//! and request, with the account and the connection it ran on; known tokens count toward the day and reach the Usage
//! dashboard, unknown ones count nothing; the migration that adds the records keeps a database's runs, and the one that
//! adds the account leaves the reviews before it on no account.
//! `cargo test -p orchester-store --test reviewer_usage reviewer_usage`.
mod support;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, InvocationOutcome},
  },
  id::{AgentAccountId, ConnectionId},
  resource::ModelId,
  run::{
    Invocation, PendingToolApproval, TokenUsage,
    review::{ReviewEnd, ReviewFailure, ReviewRecord},
  },
  store::ActivityStore,
  time::Timestamp,
  tokens::TokensUsed,
  usage::{ReviewUsage, UsedProvider},
};
use orchester_store::ProjectDb;
use support::{at, copied_at_version, model, started_run};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

/// A request of `invocation`'s agent to run a command, waiting.
fn request(db: &mut ProjectDb, invocation: &Invocation) -> PendingToolApproval {
  let approval = PendingToolApproval {
    id: format!("{}:call_1", invocation.id),
    run: invocation.run,
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo build".to_owned(),
    target: None,
    requested_at: at(25),
    native: None,
  };
  db.request_tool_approval(&approval).expect("request");
  approval
}

/// A review of `approval` by Claude Code's Haiku that ended at `ended` having spent `usage`.
fn review(approval: &PendingToolApproval, ended: Timestamp, usage: Option<TokenUsage>, end: ReviewEnd) -> ReviewRecord {
  ReviewRecord {
    run: approval.run,
    execution: approval.execution,
    invocation: approval.invocation,
    approval: approval.id.clone(),
    provider: AgentProvider::ClaudeCodeCli,
    account: None,
    connection: None,
    model: ModelId::new("claude-haiku-4-5").expect("model"),
    started_at: Timestamp::from_unix_millis(ended.unix_millis() - 5),
    ended_at: ended,
    usage,
    end,
  }
}

#[test]
fn reviewer_usage_is_kept_per_review_and_counts_toward_the_day_and_the_dashboard() {
  let (_repo, mut db, run, execution) = started_run();
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let turn = AgentEvent::TurnCompleted {
    usage: usage(1_000, 600, 200),
  };
  db.append_activity(&invocation, &[(at(20), turn)]).expect("transcript");
  invocation.ended_at = Some(at(50));
  invocation.outcome = Some(InvocationOutcome::Completed);
  db.save_invocation(&invocation).expect("ended");
  let approval = request(&mut db, &invocation);
  let known = review(&approval, at(30), Some(usage(200, 50, 7)), ReviewEnd::Approved);
  let unknown = review(&approval, at(40), None, ReviewEnd::Failed(ReviewFailure::CouldNotRun));
  let late = review(&approval, at(2 * DAY), Some(usage(10, 0, 5)), ReviewEnd::Asked);
  for record in [&late, &unknown, &known] {
    db.record_reviewer_usage(record).expect("recorded");
  }
  assert_eq!(
    db.reviewer_usage(run.id()).expect("reviews"),
    [known.clone(), unknown, late],
    "in the order they ended, unknown usage kept unknown"
  );
  assert_eq!(
    db.tokens_used_since(at(0)).expect("the day"),
    TokensUsed::new(600 + 157 + 15)
  );

  let facts = db.usage_facts(at(0), at(3 * DAY)).expect("facts");
  let seen: Vec<_> = facts.reviews.iter().map(|review| review.at).collect();
  assert_eq!(seen, [at(30), at(2 * DAY)], "only reviews whose usage is known");
  assert_eq!(
    facts.reviews[0],
    ReviewUsage {
      at: at(30),
      invocation: invocation.id,
      cli: AgentProvider::ClaudeCodeCli,
      connection: None,
      model: "claude-haiku-4-5".into(),
      usage: usage(200, 50, 7),
    }
  );
  // The invocation ended long before; its late review still brings it and its run.
  let later = db.usage_facts(at(DAY), at(3 * DAY)).expect("facts");
  assert_eq!(later.reviews.len(), 1);
  assert_eq!(later.invocations.len(), 1);
  assert_eq!(later.runs.len(), 1);
}

fn schema_version(path: &std::path::Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

#[test]
fn reviewer_usage_is_added_to_a_database_of_the_schema_before_and_its_runs_are_kept() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = request(&mut db, &invocation);
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, version_before_reviewer_usage(latest));
  let old_database = old.path().join(".orchester/orchester.db");
  assert!(
    !has_reviewer_usage(&rusqlite::Connection::open(&old_database).expect("open")),
    "the schema before has no reviewer usage"
  );
  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old_database), latest);
  assert!(upgraded.reviewer_usage(run.id()).expect("none yet").is_empty());
  let record = review(&approval, at(30), Some(usage(200, 50, 7)), ReviewEnd::Approved);
  upgraded
    .record_reviewer_usage(&record)
    .expect("a review of the kept run's request");
  assert_eq!(upgraded.reviewer_usage(run.id()).expect("reviews"), [record]);
}

fn has_reviewer_usage(connection: &rusqlite::Connection) -> bool {
  connection
    .prepare("SELECT 1 FROM sqlite_master WHERE name = 'reviewer_usage'")
    .expect("schema")
    .exists([])
    .expect("schema")
}

/// The newest schema without the reviewer's usage, wherever later migrations moved the latest.
fn version_before_reviewer_usage(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !has_reviewer_usage(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before the reviewer's usage")
}

#[test]
fn reviewer_usage_keeps_the_account_and_the_connection_the_reviewer_ran_on() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = request(&mut db, &invocation);
  let account = AgentAccountId::generate();
  let connection = UsedProvider {
    id: ConnectionId::generate(),
    name: "Claude · Reviewer".into(),
  };
  let record = ReviewRecord {
    account: Some(account),
    connection: Some(connection.clone()),
    ..review(&approval, at(30), Some(usage(200, 50, 7)), ReviewEnd::Approved)
  };
  db.record_reviewer_usage(&record).expect("recorded");
  assert_eq!(db.reviewer_usage(run.id()).expect("reviews"), [record]);
  let facts = db.usage_facts(at(0), at(DAY)).expect("facts");
  assert_eq!(facts.reviews.len(), 1);
  assert_eq!(
    facts.reviews[0].connection,
    Some(connection.id),
    "the dashboard gets the provider"
  );
  let named = facts
    .providers
    .iter()
    .find(|provider| provider.id == connection.id)
    .expect("a connection gone since is named by the name the review recorded");
  assert_eq!((&*named.name, named.removed), ("Claude · Reviewer", true));
}

#[test]
fn reviewer_usage_from_before_the_account_was_kept_is_given_no_account() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = request(&mut db, &invocation);
  let record = ReviewRecord {
    account: Some(AgentAccountId::generate()),
    ..review(&approval, at(30), Some(usage(200, 50, 7)), ReviewEnd::Approved)
  };
  db.record_reviewer_usage(&record).expect("recorded");
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, version_before_reviewer_accounts(latest));
  let upgraded = ProjectDb::open(old.path()).expect("migrate");
  let reviews = upgraded.reviewer_usage(run.id()).expect("reviews");
  assert_eq!(
    reviews,
    [ReviewRecord {
      account: None,
      ..record
    }],
    "a review recorded before the account was kept is not given one"
  );
}

/// The newest schema whose reviewer usage has no account.
fn version_before_reviewer_accounts(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      let old = ProjectDb::create_at_version(repo.path(), version).expect("an old database");
      !old
        .prepare("SELECT 1 FROM pragma_table_info('reviewer_usage') WHERE name = 'account_id'")
        .expect("schema")
        .exists([])
        .expect("schema")
    })
    .expect("a schema before the reviewer's account")
}
