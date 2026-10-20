//! Plan limits read outside runs (ORC-55) as the workspace keeps them: a read is the account's last reading, with its
//! source, where no turn reported a newer one; the Usage page's history draws it; and a workspace whose reader never
//! started (every test and QA copy) runs no *Check now*.

use orchester_core::{
  account::AccountCli,
  connection::plan_limits::{LimitState, LimitWindow, PlanLimits, ReadingSource},
  time::Timestamp,
};
use orchester_store::AppDb;
use orchester_workspace::{
  plan_limits::reads::PlanReads,
  runner::Runners,
  usage::{Scope, UsageData},
  workspace::Workspace,
};

fn limits(percent: f64) -> PlanLimits {
  PlanLimits {
    windows: vec![LimitWindow::new(300, Some(percent), None, LimitState::Normal, false).expect("a window")],
    plan: None,
    credits: None,
    blocked: None,
  }
}

#[test]
fn plan_reads_are_the_accounts_last_reading_and_part_of_the_history() {
  let data = tempfile::tempdir().expect("temporary directory");
  let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("the app database"));
  let codex = AccountCli::Codex.default_account();
  let now = Timestamp::now();
  workspace
    .save_plan_read((codex, AccountCli::Codex), now, &limits(33.0))
    .expect("kept");

  let (reading, source) = workspace
    .last_plan_reading(codex)
    .expect("read")
    .expect("Codex's Default was read");
  assert_eq!((reading.at, source), (now, ReadingSource::Read));
  assert_eq!(reading.limits, limits(33.0));
  let all = workspace.last_plan_readings().expect("read");
  assert_eq!(all.source_of(codex), Some(ReadingSource::Read));
  assert!(
    workspace
      .last_plan_reading(AccountCli::ClaudeCode.default_account())
      .expect("read")
      .is_none(),
    "Claude was never read"
  );

  let usage = UsageData::read(&workspace, Scope::All, Timestamp::from_unix_millis(0), now).expect("the usage");
  assert_eq!(usage.snapshots.len(), 1, "{:?}", usage.snapshots);
  assert_eq!(usage.snapshots[0].origin, ReadingSource::Read);
  assert_eq!(usage.snapshots[0].account, Some(codex));
}

#[test]
fn plan_reads_check_now_runs_nothing_where_the_reader_never_started() {
  let data = tempfile::tempdir().expect("temporary directory");
  let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("the app database"));
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = Runners::new(
    data.path().join("orchester-runner"),
    runtime.handle().clone(),
    orchester_workspace::activity::Activity::new(),
    orchester_ipc::SessionEnd::Clean,
  );
  let reads = PlanReads::new(workspace, runners);
  let refused = reads
    .check_now(AccountCli::ClaudeCode.default_account())
    .expect_err("nothing runs");
  assert_eq!(refused.key, "accounts.planCheck.off");
  assert!(!reads.checking(AccountCli::ClaudeCode.default_account()));
}
