//! The plan limits reports a project stored, each with its account (`account_readings_in`): what the Usage page's plan
//! limits legend draws one entry per account and window from. A report from before accounts was its CLI's Default's.

mod support;

use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::AgentAccountId,
  time::Timestamp,
};
use orchester_workspace::plan_limits::account_readings_in;

fn reading(provider: AgentProvider, account: Option<AgentAccountId>, at: i64) -> LimitReading {
  LimitReading {
    provider,
    account,
    at: support::at(at),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10_080, Some(41.0), None, LimitState::Normal, false).expect("a window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn account_readings_name_the_account_of_every_report() {
  let mut fixture = support::Fixture::new();
  let task = fixture.queued_task("Sign in", support::at(10));
  let run = fixture.started_run(task, support::at(20));
  let work = AgentAccountId::generate();
  for reading in [
    reading(AgentProvider::ClaudeCodeCli, None, 100),
    reading(AgentProvider::CodexCli, Some(work), 200),
  ] {
    fixture.db.save_limit_snapshot(run.id(), &reading).expect("stored");
  }
  let readings = account_readings_in(fixture.repo.path(), Timestamp::from_unix_millis(0)).expect("read");
  let accounts: Vec<(AgentProvider, AgentAccountId)> = readings
    .iter()
    .map(|reading| (reading.snapshot.provider, reading.account))
    .collect();
  assert_eq!(
    accounts,
    [
      (AgentProvider::ClaudeCodeCli, AccountCli::ClaudeCode.default_account()),
      (AgentProvider::CodexCli, work),
    ]
  );
}
