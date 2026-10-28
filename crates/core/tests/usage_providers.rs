//! Usage by the agent provider it was spent on (decided 2026-10-28): series in the order the Connections page lists the
//! providers, a colour slot by position among the connected ones, the fourth and later in Other with the providers no
//! longer connected, *Earlier runs* last for usage no record names a provider for, never *No account*; the reviewer's
//! reviews on the provider it ran on; command tools in no series, their time on a row of its own; the Provider filter
//! with *Earlier runs*; plan limits per account, following the CLI filter only; and the output of agents whose CLI
//! reported input but no output counted as unknown.
//! `cargo test -p orchester-core --test usage_providers usage_providers`.

use std::num::NonZeroU32;

use chrono::NaiveDate;
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
    BreakdownKey, DayRange, GroupBy, GroupKey, Grouping, IndexGiven, InvocationFacts, ProjectFacts, ProviderName,
    ProviderOrder, ProviderSeries, ProviderUsage, ReviewUsage, RunFacts, RunOutcome, SpentOn, TurnUsage, UsageFacts,
    UsageFilter, UsageQuery, UsageReport, day_start, grouped,
    limits::{LimitHistory, LimitSnapshot},
  },
};

fn spent(input: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input: 0,
    output: 1,
  }
}

fn provider(name: &str, removed: bool) -> ProviderName {
  ProviderName {
    id: ConnectionId::generate(),
    name: name.into(),
    removed,
  }
}

/// Four connected providers as the Connections page lists them, by name, then one gone since.
fn five() -> Vec<ProviderName> {
  vec![
    provider("Claude · 2", false),
    provider("Claude · Claude 2", false),
    provider("Claude · DeepSeek", false),
    provider("Deepseek", false),
    provider("Old gateway", true),
  ]
}

#[test]
fn usage_providers_go_as_the_connections_page_lists_them_and_keep_their_slot() {
  let providers = five();
  let order = ProviderOrder::of(&providers);
  assert_eq!(
    order.providers().collect::<Vec<_>>(),
    providers[..4].iter().map(|provider| provider.id).collect::<Vec<_>>(),
    "the connected ones, in the order given; the removed one is no series of its own"
  );
  let on = |index: usize| SpentOn::Provider(providers[index].id);
  assert_eq!(order.series(on(0)), ProviderSeries::ALL[0]);
  assert_eq!(order.series(on(2)), ProviderSeries::ALL[2]);
  assert_eq!(order.series(on(3)), ProviderSeries::Other, "the fourth");
  assert_eq!(order.series(on(4)), ProviderSeries::Other, "one no longer connected");
  assert_eq!(order.series(SpentOn::Earlier), ProviderSeries::Earlier);
  assert_eq!(
    ProviderSeries::ALL.map(ProviderSeries::index),
    [0, 1, 2, 3, 4],
    "providers 1-3, Other, Earlier runs"
  );
}

#[test]
fn usage_providers_past_the_third_and_removed_ones_share_other_each_listed() {
  let providers = five();
  let order = ProviderOrder::of(&providers);
  let (fourth, removed, first) = (providers[3].id, providers[4].id, providers[0].id);
  let mut usage = ProviderUsage::default();
  usage.add(SpentOn::Provider(removed), spent(5));
  usage.add(SpentOn::Provider(fourth), spent(20));
  usage.add(SpentOn::Provider(first), spent(3));
  usage.add(SpentOn::Earlier, spent(7));
  let split = order.split(&usage);
  assert_eq!(split.of(ProviderSeries::Other), [spent(20), spent(5)].into_iter().sum());
  assert_eq!(
    split.others,
    [(fourth, spent(20)), (removed, spent(5))],
    "in series order, the provider no longer connected last"
  );
  assert_eq!(split.of(ProviderSeries::Earlier), spent(7));
  assert_eq!(split.of(ProviderSeries::ALL[0]), spent(3));
  assert_eq!(
    split.of(ProviderSeries::ALL[1]),
    TokenUsage::default(),
    "an empty series keeps its slot"
  );
}

/// The connections of [`facts`]: the agent's, the reviewer's, and the one an invocation from before names.
struct Ran {
  agent: ConnectionId,
  reviewer: ConnectionId,
}

/// One run on Sep 25: an invocation on `ran.agent` spent 1,000, one no record names a provider for 200, a command
/// tool ran for a minute and spent nothing, and auto mode's reviewer on `reviewer` (`None`: not recorded) judged a
/// request of the first for 50.
fn facts(ran: &Ran, reviewer: Option<ConnectionId>) -> ProjectFacts {
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
  let invocation = |connection: Option<ConnectionId>, agent: bool| InvocationFacts {
    id: InvocationId::generate(),
    run: run.id,
    cli: agent.then_some(AgentProvider::ClaudeCodeCli),
    model: agent.then(|| "deepseek-v4-pro".into()),
    profile: agent.then(|| "Builder".into()),
    profile_id: None,
    connection,
    step: "Implement".into(),
    started_at: at,
    ended_at: Some(Timestamp::from_unix_millis(at.unix_millis() + 60_000)),
    active_ms: 60_000,
    edit: None,
    index: IndexGiven::Unknown,
  };
  let (agent, earlier, command) = (
    invocation(Some(ran.agent), true),
    invocation(None, true),
    invocation(None, false),
  );
  let turn = |invocation: &InvocationFacts, input: u64| TurnUsage {
    at,
    invocation: invocation.id,
    usage: spent(input),
  };
  ProjectFacts {
    project: ProjectId::generate(),
    name: "Orchester Platform".into(),
    facts: UsageFacts {
      turns: vec![turn(&agent, 1_000), turn(&earlier, 200)],
      reviews: vec![ReviewUsage {
        at,
        invocation: agent.id,
        cli: AgentProvider::ClaudeCodeCli,
        connection: reviewer,
        model: "claude-haiku-4-5".into(),
        usage: spent(50),
      }],
      invocations: vec![agent, earlier, command],
      runs: vec![run],
      providers: Vec::new(),
      last_turn: Some(at),
    },
  }
}

fn ran() -> Ran {
  Ran {
    agent: ConnectionId::generate(),
    reviewer: ConnectionId::generate(),
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

/// The usage of `facts` by provider, as *Tokens over time* draws it, and the reviewer's row's.
fn by_provider(facts: &ProjectFacts) -> (ProviderUsage, ProviderUsage) {
  let report = report(facts, &UsageFilter::default());
  let mut all = ProviderUsage::default();
  for bar in &report.series {
    for (on, usage) in bar.by_provider.entries() {
      all.add(*on, *usage);
    }
  }
  let reviewer = report
    .breakdowns
    .profiles
    .iter()
    .find(|row| matches!(row.key, BreakdownKey::Reviewer(_)))
    .expect("the reviewer's row");
  (all, reviewer.by_provider.clone())
}

#[test]
fn usage_providers_name_what_each_token_ran_on_and_earlier_runs_what_none_names() {
  let ran = ran();
  let facts = facts(&ran, Some(ran.reviewer));
  let (all, reviewer) = by_provider(&facts);
  let entries = all.entries();
  assert!(
    entries.contains(&(SpentOn::Provider(ran.agent), spent(1_000))),
    "{entries:?}"
  );
  assert!(
    entries.contains(&(SpentOn::Provider(ran.reviewer), spent(50))),
    "{entries:?}"
  );
  assert!(
    entries.contains(&(SpentOn::Earlier, spent(200))),
    "the invocation no record names a provider for: {entries:?}"
  );
  assert_eq!(
    entries.len(),
    3,
    "the command tool spent no tokens, so it is in no series: {entries:?}"
  );
  assert_eq!(reviewer.entries(), [(SpentOn::Provider(ran.reviewer), spent(50))]);
}

#[test]
fn usage_providers_a_review_whose_provider_was_not_recorded_is_an_earlier_run() {
  let ran = ran();
  let (all, reviewer) = by_provider(&facts(&ran, None));
  let entries = all.entries();
  assert!(
    entries.contains(&(SpentOn::Earlier, [spent(200), spent(50)].into_iter().sum())),
    "the unnamed invocation's and the reviewer's: {entries:?}"
  );
  assert_eq!(reviewer.entries(), [(SpentOn::Earlier, spent(50))]);
}

#[test]
fn usage_providers_rows_by_provider_put_command_tools_time_on_a_row_of_its_own() {
  let ran = ran();
  let facts = facts(&ran, Some(ran.reviewer));
  let rows = grouped(
    std::slice::from_ref(&facts),
    query(&UsageFilter::default()),
    GroupBy::Provider,
  )
  .rows;
  let row = |key: GroupKey| rows.iter().find(|row| row.key == key).cloned();
  let agent = row(GroupKey::Provider(SpentOn::Provider(ran.agent))).expect("the agent's provider");
  assert_eq!((agent.usage, agent.agent_ms), (spent(1_000), 60_000));
  let reviewer = row(GroupKey::Provider(SpentOn::Provider(ran.reviewer))).expect("the reviewer's provider");
  assert_eq!((reviewer.usage, reviewer.agent_ms), (spent(50), 0));
  let earlier = row(GroupKey::Provider(SpentOn::Earlier)).expect("Earlier runs");
  assert_eq!(earlier.usage, spent(200));
  let command = row(GroupKey::CommandTools).expect("command tools' time");
  assert_eq!((command.usage, command.agent_ms), (TokenUsage::default(), 60_000));
  assert_eq!(rows.len(), 4, "{rows:?}");
}

#[test]
fn usage_providers_the_filter_chooses_providers_and_earlier_runs() {
  let ran = ran();
  let facts = facts(&ran, Some(ran.reviewer));
  let with = |filter: UsageFilter| {
    let summary = report(&facts, &filter).summary;
    (summary.spent(), summary.agent_ms)
  };
  let on = |providers: Vec<ConnectionId>| UsageFilter {
    providers,
    ..UsageFilter::default()
  };
  assert_eq!(with(UsageFilter::default()), (1_253, 180_000));
  assert_eq!(with(on(vec![ran.reviewer])), (51, 0), "the reviewer's provider alone");
  assert_eq!(
    with(on(vec![ran.agent])),
    (1_001, 60_000),
    "the agent's provider leaves the review and the command tool out"
  );
  let earlier = UsageFilter {
    earlier: true,
    ..UsageFilter::default()
  };
  assert_eq!(
    with(earlier.clone()),
    (201, 60_000),
    "Earlier runs alone, no command tool"
  );
  assert!(!earlier.admits_provider(None), "a command tool runs on no provider");
  assert!(UsageFilter::default().admits_provider(None));
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
fn usage_providers_plan_limits_draw_a_line_per_account_and_old_reports_are_default() {
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
}

#[test]
fn usage_providers_plan_limits_follow_the_cli_filter_only() {
  let now = Timestamp::now();
  let snapshots = [
    weekly(AgentProvider::CodexCli, None, (now, 3), 10.0),
    weekly(
      AgentProvider::ClaudeCodeCli,
      Some(AgentAccountId::generate()),
      (now, 1),
      30.0,
    ),
  ];
  let clis = |filter: &UsageFilter| -> Vec<AgentProvider> {
    LimitHistory::of(&snapshots, last_day(now), now, filter)
      .current
      .iter()
      .map(|window| window.provider)
      .collect()
  };
  let providers = UsageFilter {
    providers: vec![ConnectionId::generate()],
    earlier: true,
    ..UsageFilter::default()
  };
  assert_eq!(
    clis(&providers).len(),
    2,
    "a provider does not choose whose plan limits are shown"
  );
  assert!(!providers.chooses_limits());
  let claude = UsageFilter {
    clis: vec![AgentProvider::ClaudeCodeCli],
    ..UsageFilter::default()
  };
  assert_eq!(clis(&claude), [AgentProvider::ClaudeCodeCli]);
  assert!(claude.chooses_limits());
}

#[test]
fn usage_providers_output_reported_by_no_turn_of_an_agent_is_unknown() {
  let ran = ran();
  let mut facts = facts(&ran, Some(ran.reviewer));
  // DeepSeek's messages report no output until the turn's result: the agent read 1,000 and wrote nothing yet.
  facts.facts.turns[0].usage = TokenUsage {
    input: 1_000,
    cached_input: 900,
    output: 0,
  };
  let summary = report(&facts, &UsageFilter::default()).summary;
  assert_eq!(summary.output_unreported, 1, "the other agent reported its output");
  // The result that ends the turn adds what it wrote.
  let result = TurnUsage {
    usage: TokenUsage {
      input: 0,
      cached_input: 0,
      output: 31_263,
    },
    ..facts.facts.turns[0]
  };
  facts.facts.turns.push(result);
  let summary = report(&facts, &UsageFilter::default()).summary;
  assert_eq!(summary.output_unreported, 0, "the result reported it");
  let unreported = TokenUsage {
    input: 5,
    cached_input: 0,
    output: 0,
  };
  assert!(unreported.output_unreported());
  assert!(
    !TokenUsage::default().output_unreported(),
    "nothing read, nothing to write"
  );
  assert!(!spent(5).output_unreported());
}
