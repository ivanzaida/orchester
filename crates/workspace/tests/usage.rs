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
use orchester_workspace::{
  links::Link,
  usage::{
    CustomRangeError, Dashboard, DashboardState, Facet, FilterValue, ParamsError, QueryError, TABLE_READINGS,
    UsageData, UsageQuery, ValueLabel, custom_range,
  },
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
  let link = |query: &UsageQuery| Link::Usage(project, Box::new(query.clone())).path();
  assert_eq!(link(&UsageQuery::default()), format!("/project/{project}/usage"));
  assert_eq!(
    link(&query),
    format!("/project/{project}/usage?group=agent&pipeline=Fix%20%26%20ship"),
  );
  assert_eq!(
    Link::UsageEverywhere(Box::new(query)).path(),
    "/usage?group=agent&pipeline=Fix%20%26%20ship"
  );
}

#[test]
fn usage_query_on_the_page_groups_by_time() {
  let by_agent = UsageQuery {
    group: GroupBy::Agent,
    ..UsageQuery::default()
  };
  assert_eq!(by_agent.on_page().group, GroupBy::Day, "the page groups by hour or day");
  let by_hour = UsageQuery {
    group: GroupBy::Hour,
    ..UsageQuery::default()
  };
  assert_eq!(by_hour.on_page(), by_hour);
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
    pipeline_version: None,
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
    unread_runs: 0,
    snapshots: Arc::from([]),
    threshold: WarningThreshold::DEFAULT,
    accounts: Arc::from([]),
    read_at: now,
  };
  (data, pipeline, profile)
}

fn dashboard(query: &UsageQuery, data: &UsageData, now: Timestamp) -> Dashboard {
  Dashboard::of(Some("Kontur".into()), query, data, now)
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

#[test]
fn usage_every_projects_page_chooses_projects_by_menu_link_and_row() {
  let now = Timestamp::now();
  let (data, ..) = data(now);
  let kontur = data.projects[0].project;
  let page = Dashboard::of(None, &UsageQuery::default(), &data, now);
  assert_eq!((page.project.clone(), page.projects), (None, 1));
  let projects = page.options(Facet::Project);
  assert_eq!(projects[0].value, FilterValue::Project(kontur));
  assert_eq!(projects[0].label, ValueLabel::Name("Kontur".into()));
  let chosen = page
    .with_row(&BreakdownKey::Project(kontur, "Kontur".into()))
    .expect("a project's row filters");
  assert_eq!(chosen.filter.projects, [kontur]);
  assert_eq!(
    read_back(&chosen.params()),
    Ok(chosen.clone()),
    "a link carries the project"
  );
  let page = dashboard(&chosen, &data, now);
  assert!(
    page
      .toggled(Facet::Project, &FilterValue::Project(kontur))
      .filter
      .is_empty()
  );
}

#[test]
fn usage_limit_table_lists_the_seven_newest_readings_where_use_moved() {
  use orchester_core::{
    connection::plan_limits::{LimitState, LimitWindow, PlanLimits},
    usage::limits::LimitSnapshot,
  };
  let now = Timestamp::now();
  let (mut data, ..) = data(now);
  // Twenty weekly readings an hour apart: 10%, 10%, 11%, 11%, … so every other one repeats its window's.
  let snapshots: Vec<LimitSnapshot> = (0..20_i64)
    .map(|index| {
      let percent = 10.0 + (index / 2) as f64;
      let window = LimitWindow::new(10_080, Some(percent), None, LimitState::Normal, false).expect("a window");
      LimitSnapshot {
        provider: AgentProvider::CodexCli,
        account: None,
        at: Timestamp::from_unix_millis(now.unix_millis() - (20 - index) * 3_600_000),
        limits: PlanLimits {
          windows: vec![window],
          plan: None,
          credits: None,
          blocked: None,
        },
        source: None,
      }
    })
    .collect();
  data.snapshots = Arc::from(snapshots);
  let page = dashboard(&UsageQuery::default(), &data, now);
  let used: Vec<f64> = page.limit_table().iter().map(|(_, point)| point.percent).collect();
  assert_eq!(TABLE_READINGS, 7);
  assert_eq!(
    used,
    [19.0, 18.0, 17.0, 16.0, 15.0, 14.0, 13.0],
    "the newest seven, each % once"
  );
}

#[test]
fn usage_pipeline_versions_are_each_pipelines_own() {
  let now = Timestamp::now();
  let (mut data, ..) = data(now);
  let version = |number: u32| {
    Some(orchester_core::resource::VersionNumber::new(
      NonZeroU32::new(number).expect("above zero"),
    ))
  };
  let mut kontur = data.projects[0].clone();
  kontur.facts.runs[0].pipeline_version = version(3);
  data.projects = Arc::from([kontur.clone()]);
  let page = dashboard(&UsageQuery::default(), &data, now);
  assert_eq!(page.pipeline_version("Feature delivery"), version(3));
  assert_eq!(page.run_pipeline_version(kontur.facts.runs[0].id), version(3));
  // Another project's pipeline of the same name has versions of its own: the name alone says no version.
  let mut atlas = kontur.clone();
  atlas.project = ProjectId::generate();
  atlas.facts.runs[0].pipeline_id = Some(ResourceId::generate());
  atlas.facts.runs[0].pipeline_version = version(1);
  data.projects = Arc::from([kontur, atlas]);
  let page = dashboard(&UsageQuery::default(), &data, now);
  assert_eq!(page.pipeline_version("Feature delivery"), None);
}

#[test]
fn usage_failed_tasks_open_the_one_project_their_runs_belong_to() {
  let now = Timestamp::now();
  let (mut data, ..) = data(now);
  let mut kontur = data.projects[0].clone();
  kontur.facts.runs[0].outcome = RunOutcome::Failed;
  data.projects = Arc::from([kontur.clone()]);
  let page = Dashboard::of(None, &UsageQuery::default(), &data, now);
  assert_eq!(page.failed_project(), Some(kontur.project));
  let mut atlas = kontur.clone();
  atlas.project = ProjectId::generate();
  data.projects = Arc::from([kontur, atlas]);
  let page = Dashboard::of(None, &UsageQuery::default(), &data, now);
  assert_eq!(
    page.failed_project(),
    None,
    "failed runs in two projects have no one list"
  );
}

#[test]
fn usage_account_filter_reads_none_and_offers_no_account_last() {
  let no_account = read_back(&[("account", "none".to_owned())]).expect("a query");
  assert!(no_account.filter.no_account && no_account.filter.accounts.is_empty());
  assert_eq!(no_account.params(), [("account", "none".to_owned())], "written back");
  let now = Timestamp::now();
  let (data, ..) = data(now);
  let page = dashboard(&UsageQuery::default(), &data, now);
  let options = page.options(Facet::Account);
  let last = options.last().expect("No account");
  assert_eq!(last.value, FilterValue::NoAccount);
  assert_eq!(last.label, ValueLabel::NoAccount);
  let chosen = page.toggled(Facet::Account, &FilterValue::NoAccount);
  assert!(chosen.filter.no_account);
  assert!(
    dashboard(&chosen, &data, now)
      .toggled(Facet::Account, &FilterValue::NoAccount)
      .filter
      .is_empty()
  );
}
