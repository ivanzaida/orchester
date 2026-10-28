//! The Usage page by agent provider (decided 2026-10-28): series named as the Connections page names the providers
//! (*Claude · DeepSeek*), one gone since by the name it had, the providers a bar's tooltip opens under *Other
//! providers*, the Provider filter's options with *Earlier runs* last and never *No account*; and that plan limits,
//! which belong to accounts, are hidden by the CLI filter and not by a provider.
//! `cargo test -p orchester-workspace --test usage_providers usage_providers`.

use std::{num::NonZeroU32, sync::Arc};

use chrono::Days;
use orchester_core::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, ConnectionId, InvocationId, ProjectId, RunId, TaskId},
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    DayRange, IndexGiven, InvocationFacts, Period, ProjectFacts, ProviderName, ProviderSeries, RunFacts, RunOutcome,
    TurnUsage, UsageFacts,
    limits::{LimitSnapshot, WarningThreshold},
    local_date,
  },
};
use orchester_workspace::usage::{
  Dashboard, Facet, FilterValue, ProviderLabel, SeriesLabel, UsageData, UsageQuery, ValueLabel,
};

const HOUR: i64 = 3_600_000;

fn ago(now: Timestamp, ms: i64) -> Timestamp {
  Timestamp::from_unix_millis(now.unix_millis() - ms)
}

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
    origin: orchester_core::connection::plan_limits::ReadingSource::Turn,
  }
}

/// Yesterday, and how `choose` changes the default query.
fn yesterday(now: Timestamp, choose: impl FnOnce(&mut UsageQuery)) -> UsageQuery {
  let day = local_date(now).checked_sub_days(Days::new(1)).expect("a day");
  let mut query = UsageQuery {
    period: Period::Custom(DayRange::new(day, day).expect("a day")),
    ..UsageQuery::default()
  };
  choose(&mut query);
  query
}

fn limits_filtered_out(query: &UsageQuery, snapshots: Vec<LimitSnapshot>, now: Timestamp) -> bool {
  let page = Dashboard::of(None, query, &data(now, Vec::new(), snapshots), now);
  page.limits_filtered_out()
}

#[test]
fn usage_providers_plan_limits_are_hidden_by_the_cli_filter_not_by_a_provider() {
  let now = Timestamp::now();
  let codex = AccountCli::Codex.default_account();
  let before = vec![report(codex, ago(now, 30 * HOUR))];
  let provider = yesterday(now, |query| {
    query.filter.providers.push(ConnectionId::generate());
    query.filter.earlier = true;
  });
  let page = Dashboard::of(None, &provider, &data(now, Vec::new(), before.clone()), now);
  assert!(!page.limits.is_empty(), "a provider chooses no account's plan limits");
  assert!(!page.limits_filtered_out());
  let other_cli = yesterday(now, |query| query.filter.clis.push(AgentProvider::ClaudeCodeCli));
  assert!(limits_filtered_out(&other_cli, before, now));
  assert!(
    !limits_filtered_out(
      &yesterday(now, |query| query.filter.clis.push(AgentProvider::CodexCli)),
      vec![report(codex, ago(now, 60_000))],
      now
    ),
    "the chosen CLI reported only today: the period, not the filter, has no plan limits"
  );
}

/// One project whose agents ran on `connections` (`None`: no record names one), each at its time, with `providers`
/// as its Connections page lists them.
fn facts(connections: &[(Option<ConnectionId>, Timestamp)], providers: Vec<ProviderName>) -> ProjectFacts {
  let run = RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::MIN),
    task: TaskId::generate(),
    task_key: "OP-22".into(),
    task_title: "Investigate".into(),
    pipeline: None,
    pipeline_id: None,
    pipeline_version: None,
    tags: Vec::new(),
    outcome: RunOutcome::Succeeded,
  };
  let invocations: Vec<InvocationFacts> = connections
    .iter()
    .map(|(connection, at)| InvocationFacts {
      id: InvocationId::generate(),
      run: run.id,
      cli: Some(AgentProvider::ClaudeCodeCli),
      model: Some("deepseek-v4-pro".into()),
      profile: None,
      profile_id: None,
      connection: *connection,
      step: "Investigate".into(),
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
        output: 10,
      },
    })
    .collect();
  ProjectFacts {
    project: ProjectId::generate(),
    name: "Orchester Platform".into(),
    facts: UsageFacts {
      turns,
      reviews: Vec::new(),
      invocations,
      runs: vec![run],
      providers,
      last_turn: connections.iter().map(|(_, at)| *at).max(),
    },
  }
}

fn named(name: &str, removed: bool) -> ProviderName {
  ProviderName {
    id: ConnectionId::generate(),
    name: name.into(),
    removed,
  }
}

#[test]
fn usage_providers_name_each_series_as_the_connections_page_does() {
  let now = Timestamp::now();
  let (claude, deepseek, gone) = (
    named("Claude · Claude 2", false),
    named("Claude · DeepSeek", false),
    named("Old gateway", true),
  );
  let today = ago(now, 60_000);
  let runs = facts(
    &[
      (Some(claude.id), today),
      (Some(deepseek.id), today),
      (Some(gone.id), today),
      (None, today),
    ],
    vec![claude.clone(), deepseek.clone(), gone],
  );
  let page = Dashboard::of(None, &UsageQuery::default(), &data(now, vec![runs], Vec::new()), now);
  let labels: Vec<(ProviderSeries, SeriesLabel)> = page
    .shown_series()
    .into_iter()
    .map(|shown| (shown.series, shown.label))
    .collect();
  let connected = |name: &str| ProviderLabel::Connected { name: name.into() };
  assert_eq!(
    labels,
    [
      (
        ProviderSeries::ALL[0],
        SeriesLabel::Provider(connected("Claude · Claude 2"))
      ),
      (
        ProviderSeries::ALL[1],
        SeriesLabel::Provider(connected("Claude · DeepSeek"))
      ),
      (
        ProviderSeries::Other,
        SeriesLabel::OtherOne(ProviderLabel::Removed {
          name: Some("Old gateway".into())
        })
      ),
      (ProviderSeries::Earlier, SeriesLabel::Earlier),
    ],
    "the API-key run reads as its provider, the gone one by the name it had, and none as No account"
  );
  let options: Vec<(FilterValue, ValueLabel)> = page
    .options(Facet::Provider)
    .into_iter()
    .map(|option| (option.value, option.label))
    .collect();
  assert_eq!(
    options,
    [
      (
        FilterValue::Provider(claude.id),
        ValueLabel::Provider(connected("Claude · Claude 2"))
      ),
      (
        FilterValue::Provider(deepseek.id),
        ValueLabel::Provider(connected("Claude · DeepSeek"))
      ),
      (FilterValue::Earlier, ValueLabel::Earlier),
    ]
  );
  let chosen = page.toggled(Facet::Provider, &FilterValue::Provider(deepseek.id));
  assert_eq!(chosen.filter.providers, [deepseek.id]);
  let chosen = page.toggled(Facet::Provider, &FilterValue::Earlier);
  assert!(chosen.filter.earlier);
}

#[test]
fn usage_providers_a_bar_names_its_one_provider_under_other_providers() {
  let now = Timestamp::now();
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let today = ago(now, 60_000);
  // Neither is among the project's connections: both are in Other.
  let runs = facts(&[(Some(first), ago(now, 26 * HOUR)), (Some(second), today)], Vec::new());
  let page = Dashboard::of(None, &UsageQuery::default(), &data(now, vec![runs], Vec::new()), now);
  let other = page
    .shown_series()
    .into_iter()
    .find(|shown| shown.label == SeriesLabel::Other(2))
    .expect("Other providers · 2 over the week");
  assert_eq!(other.usage.input, 2_000);
  let bar = page
    .report
    .series
    .iter()
    .find(|bar| bar.bucket.date == local_date(today))
    .expect("today's bar");
  let split = page.split(&bar.by_provider);
  assert_eq!(
    page
      .opened_others(&split)
      .iter()
      .map(|(connection, _)| *connection)
      .collect::<Vec<_>>(),
    [second],
    "today's bar holds one of Other's providers and says which"
  );
  assert_eq!(
    page.provider_label(second),
    ProviderLabel::Removed { name: None },
    "a connection no record names reads as a removed provider"
  );
  let alone = data(now, vec![facts(&[(Some(first), today)], Vec::new())], Vec::new());
  let page = Dashboard::of(None, &UsageQuery::default(), &alone, now);
  let bar = page.report.series.last().expect("today's bar");
  assert!(
    page.opened_others(&page.split(&bar.by_provider)).is_empty(),
    "Other reads as its one provider already"
  );
}
