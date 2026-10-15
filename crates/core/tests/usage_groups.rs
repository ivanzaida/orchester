//! Usage grouped and narrowed as `get_usage` and a link ask for it: tokens and active agent time per day,
//! task, pipeline, step, agent profile, model and account, auto mode's reviewer as an agent of its own;
//! filters by a task's key or id, a run, a step, an account, and a profile or pipeline by its id or name.
//! `cargo test -p orchester-core --test usage_groups usage_groups`.

use std::{num::NonZeroU32, sync::Arc};

use chrono::NaiveDate;
use orchester_core::{
  connection::AgentProvider,
  id::{AgentAccountId, InvocationId, ProjectId, ResourceId, RunId, TaskId},
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    DayRange, GroupBy, GroupKey, Grouping, IndexGiven, InvocationFacts, ProjectFacts, ReviewUsage, RunFacts,
    RunOutcome, TurnUsage, UsageFacts, UsageFilter, UsageQuery, UsageReport, day_start, grouped,
  },
};

const HOUR: i64 = 60 * 60 * 1000;

fn date(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 9, day).expect("a September day")
}

fn on(day: u32, hours: i64) -> Timestamp {
  Timestamp::from_unix_millis(day_start(date(day)).unix_millis() + hours * HOUR)
}

fn spent(input: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input: 0,
    output: 0,
  }
}

struct Fixture {
  facts: ProjectFacts,
  feature: ResourceId,
  builder: ResourceId,
  account: AgentAccountId,
  first_run: RunId,
}

fn invocation(
  run: &RunFacts,
  (cli, model): (AgentProvider, &str),
  (profile, name, step): (ResourceId, &str, &str),
  (from, to): (Timestamp, Timestamp),
) -> InvocationFacts {
  InvocationFacts {
    id: InvocationId::generate(),
    run: run.id,
    cli: Some(cli),
    model: Some(model.into()),
    profile: Some(name.into()),
    profile_id: Some(profile),
    account: None,
    step: step.into(),
    started_at: from,
    ended_at: Some(to),
    // Three quarters of the time active, a quarter waiting for a person.
    active_ms: u64::try_from(to.unix_millis() - from.unix_millis()).expect("ends after it starts") / 4 * 3,
    edit: None,
    index: IndexGiven::Unknown,
  }
}

/// KON-1 on Feature delivery, run 1: Builder implements on Codex (account `account`), 2 h on Sep 25; KON-2
/// on Quick fix, run 2: Reviewer reviews on Claude, 1 h on Sep 26, and auto mode's reviewer judged one of
/// its requests on haiku.
fn fixture() -> Fixture {
  let (feature, quick) = (ResourceId::generate(), ResourceId::generate());
  let (builder, reviewer) = (ResourceId::generate(), ResourceId::generate());
  let account = AgentAccountId::generate();
  let run = |number: u32, key: &str, pipeline: (ResourceId, &str)| RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(number).expect("above zero")),
    task: TaskId::generate(),
    task_key: key.into(),
    task_title: format!("Task {key}").into(),
    pipeline: Some(pipeline.1.into()),
    pipeline_id: Some(pipeline.0),
    tags: Vec::new(),
    outcome: RunOutcome::Succeeded,
  };
  let (one, two) = (
    run(1, "KON-1", (feature, "Feature delivery")),
    run(2, "KON-2", (quick, "Quick fix")),
  );
  let mut implement = invocation(
    &one,
    (AgentProvider::CodexCli, "gpt-5.5"),
    (builder, "Builder", "Implement"),
    (on(25, 9), on(25, 11)),
  );
  implement.account = Some(account);
  let review = invocation(
    &two,
    (AgentProvider::ClaudeCodeCli, "claude-sonnet-5-5"),
    (reviewer, "Reviewer", "Review"),
    (on(26, 14), on(26, 15)),
  );
  let turns = vec![
    TurnUsage {
      at: on(25, 10),
      invocation: implement.id,
      usage: spent(10_000),
    },
    TurnUsage {
      at: on(26, 15),
      invocation: review.id,
      usage: spent(4_000),
    },
  ];
  let reviews = vec![ReviewUsage {
    at: on(26, 15),
    invocation: review.id,
    cli: AgentProvider::ClaudeCodeCli,
    model: "claude-haiku-4-5".into(),
    usage: spent(500),
  }];
  let first_run = one.id;
  Fixture {
    facts: ProjectFacts {
      project: ProjectId::generate(),
      name: "Kontur".into(),
      facts: UsageFacts {
        turns,
        reviews,
        invocations: vec![implement, review],
        runs: vec![one, two],
        last_turn: Some(on(26, 15)),
      },
    },
    feature,
    builder,
    account,
    first_run,
  }
}

fn week() -> DayRange {
  DayRange::new(date(24), date(30)).expect("a week")
}

fn rows(facts: &ProjectFacts, filter: &UsageFilter, by: GroupBy) -> Vec<orchester_core::usage::GroupRow> {
  let query = UsageQuery {
    range: week(),
    grouping: Grouping::Day,
    filter,
    now: on(30, 12),
  };
  grouped(std::slice::from_ref(facts), query, by).rows
}

fn spent_of(facts: &ProjectFacts, filter: &UsageFilter) -> u64 {
  let query = UsageQuery {
    range: week(),
    grouping: Grouping::Day,
    filter,
    now: on(30, 12),
  };
  UsageReport::build(std::slice::from_ref(facts), query).summary.spent()
}

#[test]
fn usage_groups_by_day_list_every_day_with_its_tokens_time_and_runs() {
  let fixture = fixture();
  let days = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Day);
  assert_eq!(days.len(), 7);
  let thursday = &days[1];
  assert!(matches!(&thursday.key, GroupKey::Bucket(bucket) if bucket.date == date(25)));
  assert_eq!(
    (thursday.spent(), thursday.agent_ms, thursday.runs),
    (10_000, 2 * 2_700_000, 1)
  );
  assert_eq!((days[2].spent(), days[2].agent_ms, days[2].runs), (4_500, 2_700_000, 1));
  assert_eq!(days[0].spent() + days[3].spent(), 0);
}

#[test]
fn usage_groups_by_hour_split_an_invocation_into_its_hours() {
  let fixture = fixture();
  let hours = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Hour);
  assert_eq!(hours.len(), 7 * 24);
  let nine = hours
    .iter()
    .find(|row| matches!(&row.key, GroupKey::Bucket(bucket) if bucket.date == date(25) && bucket.hour == Some(9)));
  assert_eq!(
    nine.map(|row| row.agent_ms),
    Some(2_700_000),
    "half of the invocation's active time"
  );
}

#[test]
fn usage_groups_by_agent_keep_the_reviewer_apart_and_by_task_name_the_task() {
  let fixture = fixture();
  let agents = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Agent);
  assert_eq!(agents.len(), 3);
  assert!(matches!(&agents[0].key, GroupKey::Agent(Some(named)) if named.id == fixture.builder));
  assert!(
    agents
      .iter()
      .any(|row| row.key == GroupKey::Reviewer("claude-haiku-4-5".into()) && row.spent() == 500 && row.agent_ms == 0)
  );
  let tasks = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Task);
  assert!(matches!(&tasks[0].key, GroupKey::Task { key, .. } if &**key == "KON-1"));
  assert_eq!(tasks[1].spent(), 4_500, "the review counts under its run's task");
  let steps = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Step);
  assert_eq!(
    steps
      .iter()
      .find(|row| row.key == GroupKey::Step("Review".into()))
      .map(|row| row.spent()),
    Some(4_500)
  );
}

#[test]
fn usage_groups_by_account_put_what_names_none_in_one_row() {
  let fixture = fixture();
  let accounts = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Account);
  assert_eq!(accounts[0].key, GroupKey::Account(Some(fixture.account)));
  assert_eq!(accounts[1].key, GroupKey::Account(None));
  assert_eq!(accounts[1].spent(), 4_500);
}

#[test]
fn usage_groups_filters_narrow_to_a_task_run_step_account_and_resource() {
  let fixture = fixture();
  let facts = &fixture.facts;
  let with = |change: fn(&mut UsageFilter, &Fixture)| {
    let mut filter = UsageFilter::default();
    change(&mut filter, &fixture);
    spent_of(facts, &filter)
  };
  assert_eq!(with(|filter, _| filter.tasks.push("kon-1".into())), 10_000);
  assert_eq!(
    with(|filter, fixture| filter.tasks.push(fixture.facts.facts.runs[1].task.to_string().into())),
    4_500
  );
  assert_eq!(with(|filter, fixture| filter.runs.push(fixture.first_run)), 10_000);
  assert_eq!(
    with(|filter, _| filter.steps.push("Review".into())),
    4_500,
    "the reviewer's review counts under its step"
  );
  assert_eq!(
    with(|filter, fixture| filter.accounts.push(fixture.account)),
    10_000,
    "a review names no account"
  );
  assert_eq!(
    with(|filter, fixture| filter.profiles.push(fixture.builder.to_string().into())),
    10_000
  );
  assert_eq!(with(|filter, _| filter.profiles.push("Builder".into())), 10_000);
  assert_eq!(
    with(|filter, fixture| filter.pipelines.push(fixture.feature.to_string().into())),
    10_000
  );
  assert_eq!(with(|filter, _| filter.pipelines.push("Quick fix".into())), 4_500);
  assert_eq!(
    with(|filter, _| filter.pipelines.push("quick FIX".into())),
    4_500,
    "a name in any case"
  );
  assert_eq!(with(|filter, _| filter.profiles.push("builder".into())), 10_000);
  assert_eq!(
    with(|filter, _| filter.tasks.push("KON-9".into())),
    0,
    "a task with no runs matches nothing"
  );
  assert_eq!(with(|filter, _| filter.profiles.push(Arc::from("Nobody"))), 0);
}

#[test]
fn usage_groups_spread_an_invocations_active_time_over_its_days() {
  let mut fixture = fixture();
  // Mon 09:00 to Wed 09:00, of which an hour was active: the days share that hour, as wide as each one's part.
  let long = &mut fixture.facts.facts.invocations[0];
  long.started_at = on(28, 9);
  long.ended_at = Some(on(30, 9));
  long.active_ms = 3_600_000;
  let days = rows(&fixture.facts, &UsageFilter::default(), GroupBy::Day);
  let active: Vec<u64> = days.iter().map(|row| row.agent_ms).collect();
  assert_eq!(
    active.iter().sum::<u64>(),
    3_600_000 + 2_700_000,
    "the hour, and the review's three quarters of one"
  );
  assert_eq!(
    &active[4..],
    &[3_600_000 * 15 / 48, 3_600_000 * 24 / 48, 3_600_000 * 9 / 48]
  );
}

#[test]
fn usage_groups_total_what_the_rows_add_up_to_and_say_whether_a_filter_matches_anything() {
  let fixture = fixture();
  let query = UsageQuery {
    range: week(),
    grouping: Grouping::Day,
    filter: &UsageFilter::default(),
    now: on(30, 12),
  };
  let facts = std::slice::from_ref(&fixture.facts);
  for by in [GroupBy::Day, GroupBy::Agent, GroupBy::Task] {
    let grouped = grouped(facts, query, by);
    assert_eq!(
      grouped.summary.spent(),
      grouped.rows.iter().map(|row| row.spent()).sum::<u64>()
    );
    assert_eq!(
      grouped.summary.agent_ms,
      grouped.rows.iter().map(|row| row.agent_ms).sum::<u64>()
    );
    assert_eq!(grouped.summary.runs, 2);
  }
  let only = |change: fn(&mut UsageFilter)| {
    let mut filter = UsageFilter::default();
    change(&mut filter);
    filter.admits_any(facts)
  };
  assert!(
    only(|filter| filter.models.push("claude-haiku-4-5".into())),
    "the reviewer's model"
  );
  assert!(only(|filter| filter.tasks.push("KON-2".into())));
  assert!(!only(|filter| filter.tasks.push("KON-9".into())));
  assert!(!only(|filter| filter.steps.push("Plan".into())));
}
