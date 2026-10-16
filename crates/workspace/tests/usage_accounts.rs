//! The Usage page by account: whether the Account filter, and not the period, hides the plan limits the card
//! would show, and the accounts a bar's tooltip opens under *Other accounts*.
//! `cargo test -p orchester-workspace --test usage_accounts usage_accounts`.

use std::{num::NonZeroU32, sync::Arc};

use chrono::Days;
use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, InvocationId, ProjectId, RunId, TaskId},
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    DayRange, IndexGiven, InvocationFacts, Period, ProjectFacts, RunFacts, RunOutcome, TurnUsage, UsageFacts,
    limits::{LimitSnapshot, WarningThreshold},
    local_date,
  },
};
use orchester_workspace::usage::{Dashboard, SeriesLabel, UsageData, UsageQuery};

const HOUR: i64 = 3_600_000;

fn ago(now: Timestamp, ms: i64) -> Timestamp {
  Timestamp::from_unix_millis(now.unix_millis() - ms)
}

/// `projects` and `snapshots`, with no connected account: every account usage started on is in Other.
fn data(now: Timestamp, projects: Vec<ProjectFacts>, snapshots: Vec<LimitSnapshot>) -> UsageData {
  UsageData {
    projects: Arc::from(projects),
    unread: Arc::from([]),
    unread_runs: 0,
    snapshots: Arc::from(snapshots),
    threshold: WarningThreshold::DEFAULT,
    accounts: Arc::from([]),
    read_at: now,
  }
}

/// Codex's weekly window as `account` reported it at `at`.
fn report(account: AgentAccountId, at: Timestamp) -> LimitSnapshot {
  let window = LimitWindow::new(10_080, Some(40.0), None, LimitState::Normal, false).expect("a window");
  LimitSnapshot {
    provider: AgentProvider::CodexCli,
    account: Some(account),
    at,
    limits: PlanLimits {
      windows: vec![window],
      plan: None,
      credits: None,
      blocked: None,
    },
    source: None,
  }
}

/// Yesterday, and how `filter` changes the default query.
fn yesterday(now: Timestamp, choose: impl FnOnce(&mut UsageQuery)) -> UsageQuery {
  let day = local_date(now).checked_sub_days(Days::new(1)).expect("a day");
  let mut query = UsageQuery {
    period: Period::Custom(DayRange::new(day, day).expect("a day")),
    ..UsageQuery::default()
  };
  choose(&mut query);
  query
}

fn filtered_out(query: &UsageQuery, snapshots: Vec<LimitSnapshot>, now: Timestamp) -> bool {
  let data = data(now, Vec::new(), snapshots);
  let page = Dashboard::of(None, query, &data, now);
  assert!(page.limits.is_empty(), "the card has no plan limits to show");
  page.limits_filtered_out()
}

#[test]
fn usage_accounts_a_first_report_after_the_period_blames_no_filter() {
  let now = Timestamp::now();
  let codex = AccountCli::Codex.default_account();
  assert!(
    !filtered_out(&yesterday(now, |_| {}), vec![report(codex, ago(now, 60_000))], now),
    "nothing was reported by yesterday's end, and no filter is set"
  );
}

#[test]
fn usage_accounts_a_first_report_after_the_period_blames_no_chosen_account() {
  let now = Timestamp::now();
  let codex = AccountCli::Codex.default_account();
  let query = yesterday(now, |query| query.filter.accounts.push(codex));
  assert!(
    !filtered_out(&query, vec![report(codex, ago(now, 60_000))], now),
    "the chosen account reported only today: the period, not the filter, has no plan limits"
  );
}

#[test]
fn usage_accounts_reports_the_filter_leaves_out_are_filtered_out() {
  let now = Timestamp::now();
  let codex = AccountCli::Codex.default_account();
  let before = vec![report(codex, ago(now, 30 * HOUR))];
  let other_account = yesterday(now, |query| {
    query.filter.accounts.push(AccountCli::ClaudeCode.default_account());
  });
  assert!(filtered_out(&other_account, before.clone(), now));
  let no_account = yesterday(now, |query| query.filter.no_account = true);
  assert!(filtered_out(&no_account, before.clone(), now));
  let other_cli = yesterday(now, |query| query.filter.clis.push(AgentProvider::ClaudeCodeCli));
  assert!(filtered_out(&other_cli, before, now));
}

/// One project whose runs started on `accounts`, each with its time.
fn facts(accounts: &[(AgentAccountId, Timestamp)]) -> ProjectFacts {
  let run = RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::MIN),
    task: TaskId::generate(),
    task_key: "KON-1".into(),
    task_title: "Add retry".into(),
    pipeline: None,
    pipeline_id: None,
    pipeline_version: None,
    tags: Vec::new(),
    outcome: RunOutcome::Succeeded,
  };
  let invocations: Vec<InvocationFacts> = accounts
    .iter()
    .map(|(account, at)| InvocationFacts {
      id: InvocationId::generate(),
      run: run.id,
      cli: Some(AgentProvider::CodexCli),
      model: Some("gpt-5.5".into()),
      profile: None,
      profile_id: None,
      account: Some(*account),
      step: "Implement".into(),
      started_at: *at,
      ended_at: Some(*at),
      active_ms: 0,
      edit: None,
      index: IndexGiven::Unknown,
    })
    .collect();
  let turns = invocations
    .iter()
    .map(|invocation| TurnUsage {
      at: invocation.started_at,
      invocation: invocation.id,
      usage: TokenUsage {
        input: 1_000,
        cached_input: 0,
        output: 0,
      },
    })
    .collect();
  ProjectFacts {
    project: ProjectId::generate(),
    name: "Kontur".into(),
    facts: UsageFacts {
      turns,
      reviews: Vec::new(),
      invocations,
      runs: vec![run],
      last_turn: accounts.iter().map(|(_, at)| *at).max(),
    },
  }
}

#[test]
fn usage_accounts_a_bar_names_its_one_account_under_other_accounts() {
  let now = Timestamp::now();
  let (first, second) = (AgentAccountId::generate(), AgentAccountId::generate());
  let today = ago(now, 60_000);
  let runs = facts(&[(first, ago(now, 26 * HOUR)), (second, today)]);
  let data = data(now, vec![runs], Vec::new());
  let page = Dashboard::of(None, &UsageQuery::default(), &data, now);
  let other = page
    .shown_series()
    .into_iter()
    .find(|shown| shown.label == SeriesLabel::Other(2))
    .expect("Other accounts · 2 over the week");
  assert_eq!(other.usage.input, 2_000);
  let bar = page
    .report
    .series
    .iter()
    .find(|bar| bar.bucket.date == local_date(today))
    .expect("today's bar");
  let split = page.split(&bar.by_account);
  assert_eq!(
    page
      .opened_others(&split)
      .iter()
      .map(|(account, _)| *account)
      .collect::<Vec<_>>(),
    [second],
    "today's bar holds one of Other's accounts and says which"
  );
  let alone = data_with(now, &[(first, today)]);
  let page = Dashboard::of(None, &UsageQuery::default(), &alone, now);
  let bar = page.report.series.last().expect("today's bar");
  assert!(
    page.opened_others(&page.split(&bar.by_account)).is_empty(),
    "Other reads as its one account already"
  );
}

fn data_with(now: Timestamp, accounts: &[(AgentAccountId, Timestamp)]) -> UsageData {
  data(now, vec![facts(accounts)], Vec::new())
}
