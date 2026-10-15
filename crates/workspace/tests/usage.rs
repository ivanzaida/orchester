//! The Usage page's query, its link and what the page works out of read usage: parameters that read back
//! as the query they were written from, the defaults left out, wrong values refused by name; *Today* by
//! hour and long ranges by day; the custom range's days; the page's state (charts, no runs, filters that
//! match nothing); and what a filter menu offers and a choice, a chip's cross or a row does.
//! `cargo test -p orchester-workspace --test usage usage`.

use std::{num::NonZeroU32, sync::Arc};

use chrono::{Days, NaiveDate};
use orchester_core::{
  connection::AgentProvider,
  id::{InvocationId, ProjectId, ResourceId, RunId, TaskId},
  run::{RunNumber, TokenUsage},
  time::Timestamp,
  usage::{
    BreakdownKey, DayRange, GroupBy, Grouping, IndexGiven, InvocationFacts, Period, ProjectFacts, RunFacts, RunOutcome,
    TurnUsage, UsageFacts, UsageFilter, limits::WarningThreshold, local_date,
  },
};
use orchester_workspace::usage::{
  CustomRangeError, Dashboard, DashboardState, Facet, FilterValue, ParamsError, QueryError, UsageData, UsageQuery,
  ValueLabel, custom_range, usage_link,
};

/// The parameters of `query` read back through `from_params`, as the router hands them out.
fn read_back(params: &[(&'static str, String)]) -> Result<UsageQuery, ParamsError<()>> {
  let mut left: Vec<(&str, String)> = params.to_vec();
  UsageQuery::from_params(|name| {
    Ok(
      left
        .iter()
        .position(|(param, _)| *param == name)
        .map(|index| left.remove(index).1),
    )
  })
}

fn day(text: &str) -> NaiveDate {
  NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a day")
}

#[test]
fn usage_query_parameters_read_back_and_leave_the_defaults_out() {
  assert!(UsageQuery::default().params().is_empty());
  let mut query = UsageQuery {
    period: Period::Custom(DayRange::new(day("2026-10-01"), day("2026-10-15")).expect("a range")),
    group: GroupBy::Task,
    filter: UsageFilter::default(),
  };
  query.filter.clis.push(AgentProvider::ClaudeCodeCli);
  query.filter.models.extend(["gpt-5".into(), "claude-opus-5-5".into()]);
  query.filter.profiles.push(ResourceId::generate().to_string().into());
  query.filter.pipelines.push("Quick fix & more".into());
  query.filter.tasks.push("KON-12".into());
  query.filter.runs.push(RunId::generate());
  query.filter.steps.push("Implement".into());
  assert_eq!(read_back(&query.params()), Ok(query.clone()));
  let today = UsageQuery {
    period: Period::Today,
    group: GroupBy::Hour,
    ..UsageQuery::default()
  };
  assert_eq!(
    today.params(),
    [("period", "today".to_owned())],
    "today reads by hour unless the link says otherwise"
  );
  assert_eq!(read_back(&today.params()), Ok(today.clone()));
  let today_by_day = UsageQuery {
    group: GroupBy::Day,
    ..today
  };
  assert_eq!(
    today_by_day.params(),
    [("period", "today".to_owned()), ("group", "day".to_owned())]
  );
  assert_eq!(read_back(&today_by_day.params()), Ok(today_by_day));
}

#[test]
fn usage_query_refuses_wrong_values_by_name() {
  let refused = |params: &[(&'static str, &str)]| {
    let params: Vec<(&'static str, String)> = params
      .iter()
      .map(|(name, value)| (*name, (*value).to_owned()))
      .collect();
    match read_back(&params) {
      Err(ParamsError::Query(QueryError { param, .. })) => param,
      other => panic!("{params:?} is refused, not {other:?}"),
    }
  };
  assert_eq!(refused(&[("period", "week")]), "period");
  assert_eq!(refused(&[("group", "month")]), "group");
  assert_eq!(refused(&[("from", "2026-10-01")]), "from");
  assert_eq!(refused(&[("from", "2026-10-15"), ("to", "2026-10-01")]), "to");
  assert_eq!(
    refused(&[("period", "today"), ("from", "2026-10-01"), ("to", "2026-10-02")]),
    "period"
  );
  assert_eq!(refused(&[("cli", "copilot")]), "cli");
  assert_eq!(refused(&[("run", "KON-1")]), "run");
  assert_eq!(refused(&[("group", "day"), ("group", "hour")]), "group");
  assert_eq!(refused(&[("model", "")]), "model", "an empty value");
  assert_eq!(
    refused(&[("from", "2025-01-01"), ("to", "2026-01-02")]),
    "to",
    "367 days"
  );
  assert!(read_back(&[("from", "2025-01-01".to_owned()), ("to", "2026-01-01".to_owned())]).is_ok());
}

#[test]
fn usage_link_names_the_project_and_encodes_the_query() {
  let project = ProjectId::generate();
  let mut query = UsageQuery {
    group: GroupBy::Agent,
    ..UsageQuery::default()
  };
  query.filter.pipelines.push("Fix & ship".into());
  assert_eq!(
    usage_link(project, &UsageQuery::default()),
    format!("/project/{project}/usage")
  );
  assert_eq!(
    usage_link(project, &query),
    format!("/project/{project}/usage?group=agent&pipeline=Fix%20%26%20ship"),
  );
}

#[test]
fn usage_query_periods_choose_how_the_chart_groups() {
  let today = day("2026-10-15");
  let by_hour = UsageQuery {
    group: GroupBy::Hour,
    ..UsageQuery::default()
  };
  assert_eq!(by_hour.grouping(by_hour.range(today)), Grouping::Hour);
  let month = by_hour.with_period(Period::Month, today);
  assert_eq!(
    (month.group, month.grouping(month.range(today))),
    (GroupBy::Day, Grouping::Day)
  );
  assert_eq!(
    UsageQuery::default().with_period(Period::Today, today).group,
    GroupBy::Hour
  );
  let by_task = UsageQuery {
    group: GroupBy::Task,
    ..UsageQuery::default()
  };
  assert_eq!(
    by_task.with_period(Period::Today, today).group,
    GroupBy::Task,
    "another grouping stays"
  );
  assert_eq!(by_task.grouping(by_task.range(today)), Grouping::Day);
}

#[test]
fn usage_custom_range_reads_typed_days() {
  assert_eq!(
    custom_range(" 2026-09-01 ", "2026-09-30"),
    Ok(DayRange::new(day("2026-09-01"), day("2026-09-30")).expect("a range")),
  );
  assert_eq!(
    custom_range("2026-09-30", "2026-09-01"),
    Err(CustomRangeError::Reversed)
  );
  assert_eq!(custom_range("Sep 1", "2026-09-30"), Err(CustomRangeError::Unreadable));
  assert_eq!(
    custom_range("2024-01-01", "2024-12-31").map(|range| range.days()),
    Ok(366)
  );
  assert_eq!(custom_range("2025-01-01", "2026-01-02"), Err(CustomRangeError::TooLong));
}

/// One run today of KON-1 on Feature delivery: Builder on Codex with gpt-5.5 spent 1,000 tokens.
fn data(now: Timestamp) -> (UsageData, ResourceId, ResourceId) {
  let (pipeline, profile) = (ResourceId::generate(), ResourceId::generate());
  let run = RunFacts {
    id: RunId::generate(),
    number: RunNumber::new(NonZeroU32::MIN),
    task: TaskId::generate(),
    task_key: "KON-1".into(),
    task_title: "Add retry".into(),
    pipeline: Some("Feature delivery".into()),
    pipeline_id: Some(pipeline),
    tags: vec!["webhooks".into()],
    outcome: RunOutcome::Succeeded,
  };
  let started = Timestamp::from_unix_millis(now.unix_millis() - 60_000);
  let invocation = InvocationFacts {
    id: InvocationId::generate(),
    run: run.id,
    cli: Some(AgentProvider::CodexCli),
    model: Some("gpt-5.5".into()),
    profile: Some("Builder".into()),
    profile_id: Some(profile),
    account: None,
    step: "Implement".into(),
    started_at: started,
    ended_at: Some(now),
    active_ms: 60_000,
    edit: None,
    index: IndexGiven::Unknown,
  };
  let turn = TurnUsage {
    at: now,
    invocation: invocation.id,
    usage: TokenUsage {
      input: 1_000,
      cached_input: 0,
      output: 0,
    },
  };
  let facts = ProjectFacts {
    project: ProjectId::generate(),
    name: "Kontur".into(),
    facts: UsageFacts {
      turns: vec![turn],
      reviews: Vec::new(),
      invocations: vec![invocation],
      runs: vec![run],
      last_turn: Some(now),
    },
  };
  let data = UsageData {
    projects: Arc::from([facts]),
    unread: Arc::from([]),
    snapshots: Arc::from([]),
    threshold: WarningThreshold::DEFAULT,
    accounts: Arc::from([]),
    read_at: now,
  };
  (data, pipeline, profile)
}

fn dashboard(query: &UsageQuery, data: &UsageData, now: Timestamp) -> Dashboard {
  Dashboard::of("Kontur".into(), query, data, now)
}

#[test]
fn usage_dashboard_says_which_state_the_page_is_in() {
  let now = Timestamp::now();
  let (data, ..) = data(now);
  assert_eq!(
    dashboard(&UsageQuery::default(), &data, now).state,
    DashboardState::Charts
  );
  let mut unmatched = UsageQuery::default();
  unmatched.filter.models.push("claude-opus-5-5".into());
  assert_eq!(
    dashboard(&unmatched, &data, now).state,
    DashboardState::FilteredOut { runs: 1 },
  );
  let today = local_date(now);
  let before = today.checked_sub_days(Days::new(20)).expect("a day");
  let earlier = UsageQuery {
    period: Period::Custom(DayRange::new(before, before).expect("a day")),
    ..UsageQuery::default()
  };
  let idle = dashboard(&earlier, &data, now);
  assert_eq!(idle.state, DashboardState::NoRuns);
  assert_eq!(idle.report.last_turn, Some(now));
  assert_eq!(
    Dashboard::since(&UsageQuery::default(), now),
    UsageQuery::default().range(today).previous().start()
  );
}

#[test]
fn usage_filters_offer_what_the_period_has_and_choose_profiles_by_id() {
  let now = Timestamp::now();
  let (data, pipeline, profile) = data(now);
  let page = dashboard(&UsageQuery::default(), &data, now);
  let clis = page.options(Facet::Cli);
  assert_eq!(clis.len(), 1);
  assert_eq!(clis[0].value, FilterValue::Cli(AgentProvider::CodexCli));
  let profiles = page.options(Facet::Profile);
  assert_eq!(profiles[0].label, ValueLabel::Name("Builder".into()));
  assert_eq!(profiles[0].value, FilterValue::Text(profile.to_string().into()));
  assert_eq!(page.options(Facet::Tag)[0].runs, Some(1));
  let chosen = page.toggled(Facet::Profile, &profiles[0].value);
  assert_eq!(chosen.filter.profiles, [profile.to_string().into()]);
  let page = dashboard(&chosen, &data, now);
  assert!(page.options(Facet::Profile)[0].chosen);
  assert_eq!(
    page.chosen(Facet::Profile),
    [(
      FilterValue::Text(profile.to_string().into()),
      ValueLabel::Name("Builder".into())
    )],
  );
  assert!(page.toggled(Facet::Profile, &profiles[0].value).filter.is_empty());
  let pipeline_row = page
    .with_row(&BreakdownKey::Pipeline(Some("Feature delivery".into())))
    .expect("a pipeline row filters");
  assert_eq!(pipeline_row.filter.pipelines, ["Feature delivery".into()]);
  let page = dashboard(&pipeline_row, &data, now);
  let by_id = FilterValue::Text(pipeline.to_string().into());
  assert!(
    page.options(Facet::Pipeline)[0].chosen,
    "a name chooses the pipeline its id names"
  );
  assert!(
    page.without(Facet::Pipeline, &by_id).filter.pipelines.is_empty(),
    "its id lets its name go"
  );
  assert_eq!(page.with_row(&BreakdownKey::Reviewer("haiku".into())), None);
  assert!(page.unfiltered().filter.is_empty());
  assert!(page.cleared(Facet::Profile).filter.profiles.is_empty());
}

#[test]
fn usage_filters_name_links_runs_and_tasks_that_no_menu_offers() {
  let now = Timestamp::now();
  let (data, ..) = data(now);
  let run = data.projects[0].facts.runs[0].id;
  let mut query = UsageQuery::default();
  query.filter.runs.extend([run, RunId::generate()]);
  query.filter.tasks.push("kon-1".into());
  let page = dashboard(&query, &data, now);
  let runs = page.chosen(Facet::Run);
  assert!(matches!(&runs[0].1, ValueLabel::Run { task, .. } if &**task == "KON-1"));
  assert!(
    matches!(&runs[1].1, ValueLabel::Unknown(_)),
    "a run the page did not read shows its id"
  );
  assert_eq!(page.chosen(Facet::Task)[0].1, ValueLabel::Name("kon-1".into()));
  assert!(page.options(Facet::Run).is_empty());
}
