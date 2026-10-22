//! Usage by the account it started on: series in Settings' order by CLI (Codex first, named accounts before Default), a
//! colour slot by position among every connected account, the fourth and later in Other with the accounts no longer
//! connected, No account last, the reviewer's reviews on the account it ran on (on none when it was not recorded), and
//! the Account filter with *No account*; plan limits per account, a report from before accounts as its CLI's Default,
//! and the Account filter choosing whose plan limits are shown.
//! `cargo test -p orchester-core --test usage_accounts usage_accounts`.

use std::{num::NonZeroU32, path::PathBuf};

use chrono::NaiveDate;
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentProvider,
    plan_limits::{LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, InvocationId, ProjectId, RunId, TaskId},
  name::Name,
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    AccountOrder, AccountSeries, AccountUsage, DayRange, GroupBy, GroupKey, Grouping, IndexGiven, InvocationFacts,
    ProjectFacts, ReviewUsage, RunFacts, RunOutcome, TurnUsage, UsageFacts, UsageFilter, UsageQuery, UsageReport,
    day_start, grouped,
    limits::{LimitHistory, LimitSnapshot},
  },
};

fn spent(input: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input: 0,
    output: 0,
  }
}

/// An account of `cli` named `name`, added at `added` (ms); Default when `name` is `None`.
fn account(cli: AccountCli, name: Option<&str>, added: i64) -> AgentAccount {
  match name {
    None => AgentAccount::default_of(cli),
    Some(name) => AgentAccount::new(
      AgentAccountId::generate(),
      cli,
      Name::new(name).expect("a name"),
      AccountFolder::Created(PathBuf::from(if cfg!(windows) {
        "C:\\accounts\\a"
      } else {
        "/accounts/a"
      })),
      Timestamp::from_unix_millis(added),
    )
    .expect("an account"),
  }
}

/// Five connected accounts as Settings keeps them: Claude Code's Work, Personal and Default, Codex's Client
/// and Default.
fn five() -> Vec<AgentAccount> {
  vec![
    account(AccountCli::ClaudeCode, None, 0),
    account(AccountCli::ClaudeCode, Some("Work"), 10),
    account(AccountCli::ClaudeCode, Some("Personal"), 20),
    account(AccountCli::Codex, None, 0),
    account(AccountCli::Codex, Some("Client"), 30),
  ]
}

fn named<'a>(accounts: &'a [AgentAccount], cli: AccountCli, name: Option<&str>) -> &'a AgentAccount {
  accounts
    .iter()
    .find(|account| account.cli() == cli && account.name().map(Name::as_str) == name)
    .expect("the account")
}

#[test]
fn usage_accounts_go_by_cli_then_as_settings_lists_them_and_keep_their_slot() {
  let accounts = five();
  let order = AccountOrder::of(&accounts);
  let expected = [
    named(&accounts, AccountCli::Codex, Some("Client")),
    named(&accounts, AccountCli::Codex, None),
    named(&accounts, AccountCli::ClaudeCode, Some("Work")),
    named(&accounts, AccountCli::ClaudeCode, Some("Personal")),
    named(&accounts, AccountCli::ClaudeCode, None),
  ];
  assert_eq!(
    order.accounts().collect::<Vec<_>>(),
    expected.iter().map(|account| account.id()).collect::<Vec<_>>(),
    "Codex first, named accounts before Default"
  );
  assert_eq!(order.series(Some(expected[0].id())), AccountSeries::ALL[0]);
  assert_eq!(order.series(Some(expected[2].id())), AccountSeries::ALL[2]);
  assert_eq!(order.series(Some(expected[3].id())), AccountSeries::Other, "the fourth");
  assert_eq!(order.series(None), AccountSeries::NoAccount);
  assert_eq!(
    AccountSeries::ALL.map(AccountSeries::index),
    [0, 1, 2, 3, 4],
    "accounts 1-3, Other, No account"
  );
}

#[test]
fn usage_accounts_past_the_third_and_removed_ones_share_other_each_listed() {
  let accounts = five();
  let order = AccountOrder::of(&accounts);
  let (personal, default) = (
    named(&accounts, AccountCli::ClaudeCode, Some("Personal")).id(),
    named(&accounts, AccountCli::ClaudeCode, None).id(),
  );
  let removed = AgentAccountId::generate();
  let mut usage = AccountUsage::default();
  usage.add(Some(removed), spent(5));
  usage.add(Some(default), spent(20));
  usage.add(Some(personal), spent(10));
  usage.add(None, spent(7));
  let split = order.split(&usage);
  assert_eq!(split.of(AccountSeries::Other), spent(35));
  assert_eq!(
    split.others,
    [(personal, spent(10)), (default, spent(20)), (removed, spent(5))],
    "in series order, the account no longer connected last"
  );
  assert_eq!(split.of(AccountSeries::NoAccount), spent(7));
}

#[test]
fn usage_accounts_an_empty_series_keeps_its_slot() {
  let accounts = five();
  let order = AccountOrder::of(&accounts);
  let work = named(&accounts, AccountCli::ClaudeCode, Some("Work")).id();
  let mut usage = AccountUsage::default();
  usage.add(Some(work), spent(3));
  let split = order.split(&usage);
  assert_eq!(
    split.of(AccountSeries::ALL[0]),
    TokenUsage::default(),
    "Client spent nothing"
  );
  assert_eq!(
    split.of(AccountSeries::ALL[2]),
    spent(3),
    "Work keeps the third slot, and its colour, without Client and Default"
  );
}

/// One run on Sep 25: an invocation on `on` spent 1,000, one on no account 200, and auto mode's reviewer on
/// `reviewer` (`None`: not recorded) judged a request of the first for 50.
fn facts(on: AgentAccountId, reviewer: Option<AgentAccountId>) -> ProjectFacts {
  let date = NaiveDate::from_ymd_opt(2026, 9, 25).expect("a day");
  let at = Timestamp::from_unix_millis(day_start(date).unix_millis() + 10 * 3_600_000);
  let run = RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::MIN),
    task: TaskId::generate(),
    task_key: "KON-1".into(),
    task_title: "A task".into(),
    pipeline: None,
    pipeline_id: None,
    pipeline_version: None,
    tags: Vec::new(),
    outcome: RunOutcome::Succeeded,
  };
  let invocation = |account: Option<AgentAccountId>| InvocationFacts {
    id: InvocationId::generate(),
    run: run.id,
    cli: Some(AgentProvider::CodexCli),
    model: Some("gpt-5.5".into()),
    profile: Some("Builder".into()),
    profile_id: None,
    account,
    step: "Implement".into(),
    started_at: at,
    ended_at: Some(at),
    active_ms: 0,
    edit: None,
    index: IndexGiven::Unknown,
  };
  let (agent, native) = (invocation(Some(on)), invocation(None));
  let turn = |invocation: &InvocationFacts, input: u64| TurnUsage {
    at,
    invocation: invocation.id,
    usage: spent(input),
  };
  ProjectFacts {
    project: ProjectId::generate(),
    name: "Kontur".into(),
    facts: UsageFacts {
      turns: vec![turn(&agent, 1_000), turn(&native, 200)],
      reviews: vec![ReviewUsage {
        at,
        invocation: agent.id,
        cli: AgentProvider::ClaudeCodeCli,
        account: reviewer,
        model: "claude-haiku-4-5".into(),
        usage: spent(50),
      }],
      invocations: vec![agent, native],
      runs: vec![run],
      last_turn: Some(at),
    },
  }
}

fn query(filter: &UsageFilter) -> UsageQuery<'_> {
  let day = |day: u32| NaiveDate::from_ymd_opt(2026, 9, day).expect("a day");
  UsageQuery {
    range: DayRange::new(day(24), day(30)).expect("a week"),
    grouping: Grouping::Day,
    filter,
    now: Timestamp::from_unix_millis(day_start(day(30)).unix_millis() + 12 * 3_600_000),
  }
}

fn report(facts: &ProjectFacts, filter: &UsageFilter) -> UsageReport {
  UsageReport::build(std::slice::from_ref(facts), query(filter))
}

/// The usage of `facts` by account, as *Tokens over time* draws it, and the reviewer's row's.
fn by_account(facts: &ProjectFacts) -> (AccountUsage, AccountUsage) {
  let report = report(facts, &UsageFilter::default());
  let mut by_account = AccountUsage::default();
  for bar in &report.series {
    for (account, usage) in bar.by_account.entries() {
      by_account.add(*account, *usage);
    }
  }
  let reviewer = report
    .breakdowns
    .profiles
    .iter()
    .find(|row| matches!(row.key, orchester_core::usage::BreakdownKey::Reviewer(_)))
    .expect("the reviewer's row");
  (by_account, reviewer.by_account.clone())
}

#[test]
fn usage_accounts_the_reviewer_counts_under_the_account_it_ran_on() {
  let (client, work) = (AgentAccountId::generate(), AgentAccountId::generate());
  let facts = facts(client, Some(work));
  let (all, reviewer) = by_account(&facts);
  let entries = all.entries();
  assert!(entries.contains(&(Some(client), spent(1_000))), "{entries:?}");
  assert!(entries.contains(&(Some(work), spent(50))), "{entries:?}");
  assert!(
    entries.contains(&(None, spent(200))),
    "the native invocation alone: {entries:?}"
  );
  assert_eq!(reviewer.entries(), [(Some(work), spent(50))]);
  let rows = grouped(
    std::slice::from_ref(&facts),
    query(&UsageFilter::default()),
    GroupBy::Account,
  )
  .rows;
  let work_row = rows
    .iter()
    .find(|row| row.key == GroupKey::Account(Some(work)))
    .expect("the reviewer's account has a row");
  assert_eq!(work_row.usage, spent(50));
}

#[test]
fn usage_accounts_a_review_whose_account_was_not_recorded_counts_as_no_account() {
  let client = AgentAccountId::generate();
  let (all, reviewer) = by_account(&facts(client, None));
  let entries = all.entries();
  assert!(entries.contains(&(Some(client), spent(1_000))), "{entries:?}");
  assert!(
    entries.contains(&(None, spent(250))),
    "the native invocation's 200 and the reviewer's 50: {entries:?}"
  );
  assert_eq!(reviewer.entries(), [(None, spent(50))]);
}

#[test]
fn usage_accounts_the_filter_chooses_a_reviewer_by_its_account() {
  let (client, work) = (AgentAccountId::generate(), AgentAccountId::generate());
  let facts = facts(client, Some(work));
  let spent_with = |filter: UsageFilter| report(&facts, &filter).summary.spent();
  let on = |accounts: Vec<AgentAccountId>| UsageFilter {
    accounts,
    ..UsageFilter::default()
  };
  assert_eq!(spent_with(on(vec![work])), 50, "the reviewer's account alone");
  assert_eq!(
    spent_with(on(vec![client])),
    1_000,
    "the agent's account leaves the review out"
  );
  assert_eq!(
    spent_with(UsageFilter {
      no_account: true,
      ..UsageFilter::default()
    }),
    200,
    "No account leaves out a review on an account"
  );
}

#[test]
fn usage_accounts_the_filter_chooses_accounts_and_no_account() {
  let client = AgentAccountId::generate();
  let facts = facts(client, None);
  let spent_with = |filter: UsageFilter| report(&facts, &filter).summary.spent();
  assert_eq!(spent_with(UsageFilter::default()), 1_250);
  assert_eq!(
    spent_with(UsageFilter {
      accounts: vec![client],
      ..UsageFilter::default()
    }),
    1_000,
    "the account alone"
  );
  assert_eq!(
    spent_with(UsageFilter {
      no_account: true,
      ..UsageFilter::default()
    }),
    250,
    "No account alone: the native invocation and the reviewer"
  );
  assert_eq!(
    spent_with(UsageFilter {
      accounts: vec![client],
      no_account: true,
      ..UsageFilter::default()
    }),
    1_250
  );
  let filter = UsageFilter {
    accounts: vec![AgentAccountId::generate()],
    ..UsageFilter::default()
  };
  assert!(!filter.admits_account(None), "another account leaves out usage on none");
}

/// A weekly report of `provider` on `account` (`None`: from before accounts were recorded), `hours` ago.
fn weekly(
  provider: AgentProvider,
  account: Option<AgentAccountId>,
  (now, hours): (Timestamp, i64),
  percent: f64,
) -> LimitSnapshot {
  let window = LimitWindow::new(10_080, Some(percent), None, LimitState::Normal, false).expect("a window");
  LimitSnapshot {
    provider,
    account,
    at: Timestamp::from_unix_millis(now.unix_millis() - hours * 3_600_000),
    limits: PlanLimits {
      windows: vec![window],
      plan: None,
      credits: None,
      blocked: None,
    },
    source: None,
    origin: orchester_core::connection::plan_limits::ReadingSource::Turn,
  }
}

/// The last day up to `now`.
fn last_day(now: Timestamp) -> (Timestamp, Timestamp) {
  (Timestamp::from_unix_millis(now.unix_millis() - 24 * 3_600_000), now)
}

#[test]
fn usage_accounts_plan_limits_draw_a_line_per_account_and_old_reports_are_default() {
  let now = Timestamp::now();
  let client = AgentAccountId::generate();
  let default = AccountCli::Codex.default_account();
  let snapshots = [
    weekly(AgentProvider::CodexCli, None, (now, 3), 10.0),
    weekly(AgentProvider::CodexCli, Some(default), (now, 2), 20.0),
    weekly(AgentProvider::CodexCli, Some(client), (now, 1), 30.0),
  ];
  let history = LimitHistory::of(&snapshots, last_day(now), now, &UsageFilter::default());
  let lines: Vec<(Option<AgentAccountId>, Vec<f64>)> = history
    .lines
    .iter()
    .map(|line| (line.account, line.points.iter().map(|point| point.percent).collect()))
    .collect();
  assert_eq!(lines.len(), 2, "one line per account: {lines:?}");
  assert!(
    lines.contains(&(Some(default), vec![10.0, 20.0])),
    "the report from before accounts joins Default's line: {lines:?}"
  );
  assert!(lines.contains(&(Some(client), vec![30.0])), "{lines:?}");
  assert_eq!(history.current.len(), 2, "a reading per account");
}

#[test]
fn usage_accounts_plan_limits_follow_the_account_filter() {
  let now = Timestamp::now();
  let client = AgentAccountId::generate();
  let snapshots = [
    weekly(AgentProvider::CodexCli, None, (now, 3), 10.0),
    weekly(AgentProvider::CodexCli, Some(client), (now, 1), 30.0),
  ];
  let accounts = |filter: &UsageFilter| -> Vec<Option<AgentAccountId>> {
    LimitHistory::of(&snapshots, last_day(now), now, filter)
      .current
      .iter()
      .map(|window| window.account)
      .collect()
  };
  let none = UsageFilter {
    no_account: true,
    ..UsageFilter::default()
  };
  assert!(
    accounts(&none).is_empty(),
    "plan limits are an account's: No account has none"
  );
  let only_client = UsageFilter {
    accounts: vec![client],
    ..UsageFilter::default()
  };
  assert_eq!(accounts(&only_client), [Some(client)]);
  let only_default = UsageFilter {
    accounts: vec![AccountCli::Codex.default_account()],
    ..UsageFilter::default()
  };
  assert_eq!(
    accounts(&only_default),
    [Some(AccountCli::Codex.default_account())],
    "the report from before accounts is Default's"
  );
  assert!(only_client.chooses_limits() && none.chooses_limits());
  let models = UsageFilter {
    models: vec!["gpt-5".into()],
    ..UsageFilter::default()
  };
  assert!(
    !models.chooses_limits(),
    "a model does not choose whose plan limits are shown"
  );
}
