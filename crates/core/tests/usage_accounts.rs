//! Usage by the account it started on: series in Settings' order by CLI (Codex first, named accounts before
//! Default), a colour slot by position among every connected account, the fourth and later in Other with the
//! accounts no longer connected, No account last, the reviewer's reviews on no account, and the Account filter
//! with *No account*.
//! `cargo test -p orchester-core --test usage_accounts usage_accounts`.

use std::{num::NonZeroU32, path::PathBuf};

use chrono::NaiveDate;
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::AgentProvider,
  id::{AgentAccountId, InvocationId, ProjectId, RunId, TaskId},
  name::Name,
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    AccountOrder, AccountSeries, AccountUsage, DayRange, Grouping, IndexGiven, InvocationFacts, ProjectFacts,
    ReviewUsage, RunFacts, RunOutcome, TurnUsage, UsageFacts, UsageFilter, UsageQuery, UsageReport, day_start,
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
  assert_eq!(order.series(Some(expected[0].id())), AccountSeries::Account(0));
  assert_eq!(order.series(Some(expected[2].id())), AccountSeries::Account(2));
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
    split.of(AccountSeries::Account(0)),
    TokenUsage::default(),
    "Client spent nothing"
  );
  assert_eq!(
    split.of(AccountSeries::Account(2)),
    spent(3),
    "Work keeps the third slot, and its colour, without Client and Default"
  );
}

/// One run on Sep 25: an invocation on `on` spent 1,000, one on no account 200, and auto mode's reviewer
/// judged a request of the first for 50.
fn facts(on: AgentAccountId) -> ProjectFacts {
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
        model: "claude-haiku-4-5".into(),
        usage: spent(50),
      }],
      invocations: vec![agent, native],
      runs: vec![run],
      last_turn: Some(at),
    },
  }
}

fn report(facts: &ProjectFacts, filter: &UsageFilter) -> UsageReport {
  let day = |day: u32| NaiveDate::from_ymd_opt(2026, 9, day).expect("a day");
  let query = UsageQuery {
    range: DayRange::new(day(24), day(30)).expect("a week"),
    grouping: Grouping::Day,
    filter,
    now: Timestamp::from_unix_millis(day_start(day(30)).unix_millis() + 12 * 3_600_000),
  };
  UsageReport::build(std::slice::from_ref(facts), query)
}

#[test]
fn usage_accounts_the_reviewer_counts_as_no_account() {
  let client = AgentAccountId::generate();
  let facts = facts(client);
  let report = report(&facts, &UsageFilter::default());
  let mut by_account = AccountUsage::default();
  for bar in &report.series {
    for (account, usage) in bar.by_account.entries() {
      by_account.add(*account, *usage);
    }
  }
  let entries = by_account.entries();
  assert!(entries.contains(&(Some(client), spent(1_000))), "{entries:?}");
  assert!(
    entries.contains(&(None, spent(250))),
    "the native invocation's 200 and the reviewer's 50: {entries:?}"
  );
  let reviewer = report
    .breakdowns
    .profiles
    .iter()
    .find(|row| matches!(row.key, orchester_core::usage::BreakdownKey::Reviewer(_)))
    .expect("the reviewer's row");
  assert_eq!(reviewer.by_account.entries(), [(None, spent(50))]);
}

#[test]
fn usage_accounts_the_filter_chooses_accounts_and_no_account() {
  let client = AgentAccountId::generate();
  let facts = facts(client);
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
