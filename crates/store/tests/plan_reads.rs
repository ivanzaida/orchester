//! The plan limits Orchester read outside runs (`plan_limit_reads`, ORC-55): kept per account in the application
//! database, read back as history snapshots that say they were read, and kept 90 days like the turns' reports.

use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{LimitState, LimitWindow, PlanLimits, ReadingSource},
  },
  id::AgentAccountId,
  time::Timestamp,
  usage::limits::RETENTION_DAYS,
};
use orchester_store::AppDb;

const DAY: i64 = 24 * 60 * 60 * 1000;

fn limits(percent: f64) -> PlanLimits {
  PlanLimits {
    windows: vec![LimitWindow::new(300, Some(percent), None, LimitState::Normal, false).expect("a window")],
    plan: Some("pro".to_owned()),
    credits: None,
    blocked: None,
  }
}

#[test]
fn plan_reads_are_kept_per_account_and_read_back_as_reads() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let work = AgentAccountId::generate();
  let default = AccountCli::Codex.default_account();
  let at = |day: i64| Timestamp::from_unix_millis(100 * DAY + day * DAY);
  db.save_plan_read((work, AccountCli::Codex), at(0), &limits(40.0))
    .expect("saved");
  db.save_plan_read((default, AccountCli::Codex), at(1), &limits(10.0))
    .expect("saved");
  db.save_plan_read((work, AccountCli::Codex), at(2), &limits(55.0))
    .expect("saved");

  let last = db.last_plan_read(work).expect("read").expect("Work was read");
  assert_eq!(
    (last.provider, last.account, last.at),
    (AgentProvider::CodexCli, Some(work), at(2))
  );
  assert_eq!(last.limits, limits(55.0));
  assert!(db.last_plan_read(AgentAccountId::generate()).expect("read").is_none());

  let history = db.plan_reads(at(0)).expect("history");
  let seen: Vec<_> = history.iter().map(|snapshot| (snapshot.account, snapshot.at)).collect();
  assert_eq!(seen, [(Some(work), at(0)), (Some(default), at(1)), (Some(work), at(2))]);
  assert!(
    history
      .iter()
      .all(|snapshot| snapshot.origin == ReadingSource::Read && snapshot.source.is_none())
  );
  assert_eq!(db.plan_reads(at(1)).expect("since").len(), 2);
}

#[test]
fn plan_reads_older_than_the_history_keeps_go_with_the_next_read() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let work = AgentAccountId::generate();
  db.save_plan_read(
    (work, AccountCli::ClaudeCode),
    Timestamp::from_unix_millis(DAY),
    &limits(5.0),
  )
  .expect("saved");
  let later = Timestamp::from_unix_millis((RETENTION_DAYS + 2) * DAY);
  db.save_plan_read((work, AccountCli::ClaudeCode), later, &limits(6.0))
    .expect("saved");
  let history = db.plan_reads(Timestamp::from_unix_millis(0)).expect("history");
  assert_eq!(history.len(), 1, "the read from before 90 days went");
  assert_eq!(history[0].provider, AgentProvider::ClaudeCodeCli);
}
