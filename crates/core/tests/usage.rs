//! The Usage dashboard's figures: spent, cached and output tokens, runs, agent time, cache share and
//! failed runs by hour and day and by CLI, model, profile · step, pipeline, tag and project, against
//! the previous period, auto mode's reviews counted as turns of their runs on the reviewer's CLI and
//! model; and the plan limits history with its gaps.
//! `cargo test -p orchester-core --test usage usage`.

use std::{num::NonZeroU32, sync::Arc};

use chrono::NaiveDate;
use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{LimitState, LimitWindow, PlanLimits},
  },
  id::{InvocationId, ProjectId, RunId, TaskId},
  run::{EditProgress, FirstEdit, RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    BreakdownKey, Change, CliSeries, DayRange, FilterOptions, Grouping, IndexGiven, InvocationFacts, Period,
    ProjectFacts, ReviewUsage, RunFacts, RunOutcome, Summary, TurnUsage, UsageFacts, UsageFilter, UsageQuery,
    UsageReport, day_start,
    limits::{Join, LimitHistory, LimitSnapshot, WarningThreshold},
  },
};

const HOUR: i64 = 60 * 60 * 1000;

fn date(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 9, day).expect("a September day")
}

/// `hours` into September `day`, local time.
fn on(day: u32, hours: i64) -> Timestamp {
  Timestamp::from_unix_millis(day_start(date(day)).unix_millis() + hours * HOUR)
}

fn later(at: Timestamp, millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(at.unix_millis() + millis)
}

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

fn run(number: u32, task: TaskId, key: &str, pipeline: &str, tags: &[&str], outcome: RunOutcome) -> RunFacts {
  RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(number).expect("above zero")),
    task,
    task_key: key.into(),
    task_title: format!("Task {key}").into(),
    pipeline: Some(pipeline.into()),
    pipeline_id: None,
    pipeline_version: None,
    tags: tags.iter().map(|tag| Arc::from(*tag)).collect(),
    outcome,
  }
}

fn invocation(
  run: &RunFacts,
  cli: AgentProvider,
  model: &str,
  profile: &str,
  (from, to): (Timestamp, Timestamp),
) -> InvocationFacts {
  InvocationFacts {
    id: InvocationId::generate(),
    run: run.id,
    cli: Some(cli),
    model: Some(model.into()),
    profile: Some(profile.into()),
    profile_id: None,
    account: None,
    step: "implement".into(),
    started_at: from,
    ended_at: Some(to),
    active_ms: u64::try_from(to.unix_millis() - from.unix_millis()).expect("ends after it starts"),
    edit: None,
    index: IndexGiven::Unknown,
  }
}

fn turn(invocation: &InvocationFacts, at: Timestamp, usage: TokenUsage) -> TurnUsage {
  TurnUsage {
    at,
    invocation: invocation.id,
    usage,
  }
}

/// Kontur, Sep 24 – 30 and the week before: a Codex run that succeeded (ORC-1), a Claude run that
/// failed (ORC-2), and a Codex run of the week before (ORC-3).
fn kontur() -> ProjectFacts {
  let (one, two, three) = (TaskId::generate(), TaskId::generate(), TaskId::generate());
  let ok = run(1, one, "ORC-1", "Feature delivery", &["auth"], RunOutcome::Succeeded);
  let failed = run(2, two, "ORC-2", "Bug fix", &["billing"], RunOutcome::Failed);
  let before = run(3, three, "ORC-3", "Feature delivery", &[], RunOutcome::Succeeded);
  let codex = invocation(
    &ok,
    AgentProvider::CodexCli,
    "gpt-5.5",
    "Builder",
    (on(25, 9), on(25, 11)),
  );
  let claude = invocation(
    &failed,
    AgentProvider::ClaudeCodeCli,
    "claude-opus-5-5",
    "Reviewer",
    (on(29, 14), on(29, 15)),
  );
  let earlier = invocation(
    &before,
    AgentProvider::CodexCli,
    "gpt-5.5",
    "Builder",
    (on(18, 9), on(18, 10)),
  );
  let turns = vec![
    turn(&codex, on(25, 10), usage(10_000, 6_000, 2_000)),
    turn(&codex, on(25, 11), usage(20_000, 14_000, 4_000)),
    turn(&claude, on(29, 15), usage(8_000, 2_000, 1_000)),
    turn(&earlier, on(18, 10), usage(4_000, 1_000, 1_000)),
  ];
  ProjectFacts {
    project: ProjectId::generate(),
    name: "Kontur".into(),
    facts: UsageFacts {
      turns,
      reviews: Vec::new(),
      invocations: vec![codex, claude, earlier],
      runs: vec![ok, failed, before],
      last_turn: Some(on(29, 15)),
    },
  }
}

fn week() -> DayRange {
  DayRange::new(date(24), date(30)).expect("a week")
}

fn report(projects: &[ProjectFacts], grouping: Grouping, filter: &UsageFilter) -> UsageReport {
  UsageReport::build(
    projects,
    UsageQuery {
      range: week(),
      grouping,
      filter,
      now: on(30, 23),
    },
  )
}

#[test]
fn usage_periods_end_today_and_compare_with_as_many_days_before() {
  let today = date(30);
  assert_eq!(Period::Today.range(today), DayRange::new(today, today).expect("a day"));
  assert_eq!(Period::Week.range(today), week());
  assert_eq!(Period::Month.range(today).days(), 30);
  assert_eq!(
    week().previous(),
    DayRange::new(date(17), date(23)).expect("the week before")
  );
  assert!(week().offers_hours());
  assert!(!Period::Month.range(today).offers_hours());
  assert_eq!(week().buckets(Grouping::Day).len(), 7);
  assert!(DayRange::new(date(2), date(1)).is_err());
}

#[test]
fn usage_summary_counts_spent_cached_runs_agent_time_cache_share_and_failed_runs() {
  let projects = [kontur()];
  let report = report(&projects, Grouping::Day, &UsageFilter::default());
  let summary = report.summary;
  assert_eq!(summary.usage, usage(38_000, 22_000, 7_000));
  assert_eq!(summary.spent(), 38_000 - 22_000 + 7_000);
  assert_eq!(summary.cache_share(), Some(58));
  assert_eq!((summary.runs, summary.finished, summary.failed), (2, 1, 1));
  assert_eq!(summary.agent_ms, u64::try_from(3 * HOUR).expect("positive"));
  assert_eq!(summary.failed_spent, 7_000);
  assert_eq!(summary.failed_share(), Some(50));
  assert_eq!(report.previous.failed_share(), Some(0));
  assert_eq!(Summary::default().failed_share(), None);
  assert_eq!(report.previous.spent(), 4_000);
  assert_eq!(report.previous.runs, 1);
  assert_eq!(
    Change::between(summary.spent(), report.previous.spent()),
    Change::More(475)
  );
  assert_eq!(
    Change::points(summary.cache_share(), report.previous.cache_share()),
    Some(Change::More(33))
  );
}

#[test]
fn usage_changes_are_more_or_less_never_good_or_bad() {
  assert_eq!(Change::between(88, 100), Change::Less(12));
  assert_eq!(Change::between(112, 100), Change::More(12));
  assert_eq!(Change::between(5, 5), Change::Same);
  assert_eq!(Change::between(5, 0), Change::NoBaseline);
  assert_eq!(Change::between(0, 0), Change::Same);
  assert_eq!(Change::points(None, Some(40)), None);
  assert_eq!(Change::count(64, 55), Change::More(9));
  assert_eq!(Change::count(3, 5), Change::Less(2));
  assert_eq!(Change::count(0, 0), Change::Same);
}

#[test]
fn usage_series_has_a_bar_per_day_or_hour_split_by_cli() {
  let projects = [kontur()];
  let days = report(&projects, Grouping::Day, &UsageFilter::default()).series;
  assert_eq!(days.len(), 7);
  assert_eq!(days[1].bucket.date, date(25));
  assert_eq!(days[1].by_cli[CliSeries::Codex.index()], usage(30_000, 20_000, 6_000));
  assert_eq!(days[1].runs, 1);
  assert_eq!(days[5].by_cli[CliSeries::Claude.index()], usage(8_000, 2_000, 1_000));
  assert_eq!(days[0].total(), TokenUsage::default());

  let hours = report(&projects, Grouping::Hour, &UsageFilter::default()).series;
  let busy: Vec<_> = hours
    .iter()
    .filter(|bar| bar.total() != TokenUsage::default())
    .map(|bar| (bar.bucket.date, bar.bucket.hour))
    .collect();
  assert_eq!(busy, [(date(25), Some(10)), (date(25), Some(11)), (date(29), Some(15))]);
}

#[test]
fn usage_breakdowns_split_spent_tokens_by_profile_step_model_pipeline_and_project() {
  let projects = [
    kontur(),
    ProjectFacts {
      project: ProjectId::generate(),
      name: "Atlas".into(),
      facts: UsageFacts::default(),
    },
  ];
  let breakdowns = report(&projects, Grouping::Day, &UsageFilter::default()).breakdowns;
  assert_eq!(
    breakdowns.profiles[0].key,
    BreakdownKey::ProfileStep {
      profile: "Builder".into(),
      step: "implement".into(),
    }
  );
  assert_eq!(breakdowns.profiles[0].by_cli, [16_000, 0, 0]);
  assert_eq!(breakdowns.models[1].key, BreakdownKey::Model("claude-opus-5-5".into()));
  assert_eq!(breakdowns.models[1].by_cli, [0, 7_000, 0]);
  let pipelines: Vec<_> = breakdowns
    .pipelines
    .iter()
    .map(|row| (row.key.clone(), row.total()))
    .collect();
  assert_eq!(
    pipelines,
    [
      (BreakdownKey::Pipeline(Some("Feature delivery".into())), 16_000),
      (BreakdownKey::Pipeline(Some("Bug fix".into())), 7_000),
    ]
  );
  assert_eq!(breakdowns.projects.len(), 1);
  assert_eq!(breakdowns.idle_projects, ["Atlas".into()]);
}

#[test]
fn usage_filters_admit_cli_model_profile_pipeline_tag_and_project() {
  let projects = [kontur()];
  let by_cli = UsageFilter {
    clis: vec![AgentProvider::ClaudeCodeCli],
    ..UsageFilter::default()
  };
  assert_eq!(report(&projects, Grouping::Day, &by_cli).summary.spent(), 7_000);
  let by_tag = UsageFilter {
    tags: vec!["auth".into(), "unknown".into()],
    ..UsageFilter::default()
  };
  assert_eq!(report(&projects, Grouping::Day, &by_tag).summary.runs, 1);
  let by_pipeline = UsageFilter {
    pipelines: vec!["Bug fix".into()],
    models: vec!["claude-opus-5-5".into()],
    profiles: vec!["Reviewer".into()],
    ..UsageFilter::default()
  };
  assert_eq!(report(&projects, Grouping::Day, &by_pipeline).summary.failed, 1);
  let elsewhere = UsageFilter {
    projects: vec![ProjectId::generate()],
    ..UsageFilter::default()
  };
  let nothing = report(&projects, Grouping::Day, &elsewhere);
  assert_eq!(nothing.summary.runs, 0);
  let query = UsageQuery {
    range: week(),
    grouping: Grouping::Day,
    filter: &elsewhere,
    now: on(30, 23),
  };
  assert!(
    UsageReport::hidden_by(&projects, query),
    "the filters hide the week's runs"
  );
  let options = FilterOptions::of(&projects);
  assert_eq!(options.clis, [AgentProvider::CodexCli, AgentProvider::ClaudeCodeCli]);
  assert_eq!(options.tags, [("auth".into(), 1), ("billing".into(), 1)]);
}

#[test]
fn usage_most_expensive_lists_tasks_and_runs_by_spent_tokens() {
  let projects = [kontur()];
  let report = report(&projects, Grouping::Day, &UsageFilter::default());
  let tasks: Vec<_> = report
    .tasks
    .iter()
    .map(|task| (&*task.key, task.usage.counted(), task.runs))
    .collect();
  assert_eq!(tasks, [("ORC-1", 16_000, 1), ("ORC-2", 7_000, 1)]);
  assert_eq!(report.runs[0].started, on(25, 9));
  assert_eq!(report.runs[0].agent_ms, u64::try_from(2 * HOUR).expect("positive"));
  assert_eq!(report.runs[1].outcome, RunOutcome::Failed);
}

/// [`kontur`] with a review by Claude Code's Haiku of a request of its Codex Builder (ORC-1): 700 spent.
fn kontur_reviewed() -> ProjectFacts {
  let mut kontur = kontur();
  let builder = kontur.facts.invocations[0].id;
  kontur.facts.reviews.push(ReviewUsage {
    at: later(on(25, 10), HOUR / 2),
    invocation: builder,
    cli: AgentProvider::ClaudeCodeCli,
    account: None,
    model: "claude-haiku-4-5".into(),
    usage: usage(1_000, 400, 100),
  });
  kontur
}

#[test]
fn usage_counts_auto_mode_reviews_with_their_run_on_the_reviewers_cli_and_model() {
  let projects = [kontur_reviewed()];
  let dashboard = report(&projects, Grouping::Day, &UsageFilter::default());
  assert_eq!(dashboard.summary.spent(), 23_000 + 700, "totals include the reviewer");
  assert_eq!(dashboard.summary.runs, 2, "a review is no run of its own");
  let reviewer = BreakdownKey::Reviewer("claude-haiku-4-5".into());
  let row = dashboard.breakdowns.profiles.iter().find(|row| row.key == reviewer);
  assert_eq!(
    row.map(|row| row.by_cli),
    Some([0, 700, 0]),
    "a row of its own, on Claude Code"
  );
  let model = BreakdownKey::Model("claude-haiku-4-5".into());
  assert!(dashboard.breakdowns.models.iter().any(|row| row.key == model));
  assert_eq!(
    dashboard.series[1].by_cli[CliSeries::Claude.index()],
    usage(1_000, 400, 100)
  );
  let tasks: Vec<_> = dashboard
    .tasks
    .iter()
    .map(|task| (&*task.key, task.usage.counted()))
    .collect();
  assert_eq!(
    tasks,
    [("ORC-1", 16_700), ("ORC-2", 7_000)],
    "its run's figures include it"
  );
  assert_eq!(dashboard.runs[0].usage.counted(), 16_700);

  let spent = |filter: UsageFilter| report(&projects, Grouping::Day, &filter).summary.spent();
  let clis = |cli| UsageFilter {
    clis: vec![cli],
    ..UsageFilter::default()
  };
  assert_eq!(spent(clis(AgentProvider::ClaudeCodeCli)), 7_000 + 700);
  assert_eq!(spent(clis(AgentProvider::CodexCli)), 16_000);
  let builder = UsageFilter {
    profiles: vec!["Builder".into()],
    ..UsageFilter::default()
  };
  assert_eq!(spent(builder), 16_000, "the reviewer is no agent profile");
  let options = FilterOptions::of(&projects);
  assert!(options.models.contains(&"claude-haiku-4-5".into()));
}

#[test]
fn usage_efficiency_has_the_cache_share_per_day_edit_tokens_by_index_and_failed_runs() {
  let mut kontur = kontur();
  let edited = |before: u64| EditProgress {
    before: usage(before, 0, 0),
    first_edit: Some(FirstEdit {
      at: on(25, 10),
      path: "src/lib.rs".into(),
    }),
    at_most: false,
    index_searches: None,
  };
  kontur.facts.invocations[0].edit = Some(edited(9_000));
  kontur.facts.invocations[0].index = IndexGiven::Yes;
  kontur.facts.invocations[1].edit = Some(EditProgress::default());
  kontur.facts.invocations[1].index = IndexGiven::No;
  let efficiency = report(&[kontur], Grouping::Day, &UsageFilter::default()).efficiency;
  assert_eq!(efficiency.cache_by_day[1], (date(25), Some(67)));
  assert_eq!(
    efficiency.cache_by_day[0],
    (date(24), None),
    "a day without input has no share"
  );
  assert_eq!(
    (efficiency.with_index.median, efficiency.with_index.edited),
    (Some(9_000), 1)
  );
  assert_eq!(
    (efficiency.without_index.edited, efficiency.without_index.without_edit),
    (0, 1)
  );
  assert_eq!((efficiency.failed.runs, efficiency.failed.of_runs), (1, 2));
  assert_eq!((efficiency.failed.spent, efficiency.failed.of_spent), (7_000, 23_000));
  assert_eq!(efficiency.failed.largest.map(|run| run.key), Some("ORC-2".into()));
}

/// ORC-90: *Code index use* counts every agent invocation that ended, those that changed no file too, by what it did
/// with the index; one recorded before its searches were counted is left out.
#[test]
fn usage_index_use_counts_read_only_invocations_by_their_index_searches() {
  let mut kontur = kontur();
  let read_only = |searches: Option<u64>| EditProgress {
    before: usage(1_000, 0, 0),
    index_searches: searches,
    ..EditProgress::default()
  };
  // The Codex run searched the index 4 times and changed no file; the Claude run got it and never searched it.
  kontur.facts.invocations[0].edit = Some(read_only(Some(4)));
  kontur.facts.invocations[0].index = IndexGiven::Yes;
  kontur.facts.invocations[1].edit = Some(read_only(Some(0)));
  kontur.facts.invocations[1].index = IndexGiven::Yes;
  let mut without = kontur.facts.invocations[1].clone();
  without.id = InvocationId::generate();
  without.index = IndexGiven::No;
  let mut unknown = without.clone();
  unknown.id = InvocationId::generate();
  unknown.edit = Some(read_only(None));
  let mut running = without.clone();
  running.id = InvocationId::generate();
  running.ended_at = None;
  kontur
    .facts
    .turns
    .push(turn(&without, on(29, 15), usage(3_000, 0, 500)));
  kontur.facts.invocations.extend([without, unknown, running]);
  let efficiency = report(&[kontur], Grouping::Day, &UsageFilter::default()).efficiency;
  let index = efficiency.index_use;
  assert_eq!(
    (
      index.searched.invocations,
      index.searched.median,
      index.searched.searches
    ),
    (1, Some(16_000), 4)
  );
  assert_eq!(
    (
      index.not_searched.invocations,
      index.not_searched.median,
      index.not_searched.searches
    ),
    (1, Some(7_000), 0)
  );
  assert_eq!(
    (index.without_index.invocations, index.without_index.median),
    (1, Some(3_500)),
    "the invocation recorded before searches were counted and the running one are left out"
  );
  assert_eq!(
    (efficiency.with_index.edited, efficiency.with_index.without_edit),
    (0, 2),
    "Tokens before first edit leaves the read-only invocations out"
  );
}

fn weekly(at: Timestamp, percent: f64, resets: Timestamp) -> LimitSnapshot {
  LimitSnapshot {
    provider: AgentProvider::CodexCli,
    account: None,
    at,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10_080, Some(percent), Some(resets), LimitState::Normal, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
    source: None,
    origin: orchester_core::connection::plan_limits::ReadingSource::Turn,
  }
}

#[test]
fn usage_limit_history_joins_reports_marks_gaps_and_resets_and_invents_nothing() {
  let reset = on(27, 9);
  let snapshots = [
    weekly(
      on(20, 9),
      5.0,
      on(20, 9), /* before the range: only the legend and the first report */
    ),
    weekly(on(25, 9), 30.0, reset),
    weekly(later(on(25, 9), 20 * 60_000), 32.0, reset),
    weekly(on(26, 9), 60.0, reset),
    weekly(on(28, 9), 4.0, later(on(30, 23), 6 * HOUR)),
  ];
  let range = (week().start(), week().end());
  let history = LimitHistory::of(&snapshots, range, on(30, 23), &UsageFilter::default());
  let [line] = history.lines.as_slice() else {
    panic!("one weekly line: {:?}", history.lines);
  };
  assert_eq!(line.points.len(), 4, "the report before the range is not drawn");
  assert_eq!(line.joins, [Join::Solid, Join::Gap, Join::Reset]);
  assert_eq!(line.resets, [reset]);
  assert_eq!(history.first, [(AgentProvider::CodexCli, on(20, 9))]);
  assert_eq!(history.current[0].percent, Some(4.0));
  let claude_only = UsageFilter {
    clis: vec![AgentProvider::ClaudeCodeCli],
    ..UsageFilter::default()
  };
  assert!(LimitHistory::of(&snapshots, range, on(30, 23), &claude_only).is_empty());
}

#[test]
fn usage_warning_threshold_is_a_whole_percentage_from_1_to_100() {
  assert_eq!(WarningThreshold::default().percent(), 80);
  assert_eq!(WarningThreshold::parse(" 70 ").map(WarningThreshold::percent), Ok(70));
  for wrong in ["0", "101", "7.5", "", "70%", "-1"] {
    assert!(WarningThreshold::parse(wrong).is_err(), "{wrong}");
  }
}
