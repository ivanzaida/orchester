//! `get_usage`: the tokens and active agent time of one project's runs, or of every project's, over a
//! range of local days, filtered and grouped as the Usage page reads them (`crate::usage`), with the link
//! of the page that shows the same and, for every row, the page filtered to it. A range covers at most
//! 366 days, by hour at most 7; an empty value is refused, and a filter value nothing in the range matches
//! is named under `unmatched`.

use std::sync::Arc;

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::ProjectId,
  time::Timestamp,
  usage::{
    DayRange, GroupBy, GroupKey, GroupRow, Grouped, HOURLY_DAYS, Named, Period, ProjectFacts, Summary, UsageFilter,
    UsageQuery as ReportQuery, UsageReport, grouped, local_date,
  },
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  usage::{LONGEST_DAYS, NO_ACCOUNT, Scope, UsageData, UsageQuery, group_name, group_of},
};

/// How many days a range without `from` covers, ending with `to`: the page's default *7 days*.
const DEFAULT_DAYS: u64 = 7;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetUsage {
  /// The project's id, or its name. Without it, every project's usage, each task row naming its project.
  #[serde(default)]
  pub project: Option<String>,
  /// The first local day, `2026-10-01`. Without it, the 7 days that end with `to`.
  #[serde(default)]
  pub from: Option<String>,
  /// The last local day, `2026-10-15`, today included up to now. Without it, today.
  #[serde(default)]
  pub to: Option<String>,
  /// What rows group by: `hour` (ranges up to 7 days), `day` (the default), `task`, `pipeline`, `step`,
  /// `agent` (agent profile; auto mode's reviewer per model), `model` or `account`.
  #[serde(default)]
  pub group: Option<String>,
  /// Only this task's runs, by its key (`KON-12`) or id.
  #[serde(default)]
  pub task: Option<String>,
  /// Only this run, by its id.
  #[serde(default)]
  pub run: Option<String>,
  /// Only runs of this pipeline, by its id or name.
  #[serde(default)]
  pub pipeline: Option<String>,
  /// Only this step, by its name in whichever pipeline.
  #[serde(default)]
  pub step: Option<String>,
  /// Only this agent profile, by its id or name.
  #[serde(default)]
  pub agent: Option<String>,
  /// Only this model.
  #[serde(default)]
  pub model: Option<String>,
  /// Only usage that started on this CLI account, by its id, or `none` for usage on no account (auto mode's
  /// reviewer, native agents, command tools, runs from before accounts were recorded).
  #[serde(default)]
  pub account: Option<String>,
}

pub async fn get_usage(services: &Services, args: GetUsage) -> Result<Value, ToolError> {
  let now = Timestamp::now();
  let query = query(&args, local_date(now))?;
  let project = given("project", args.project.as_ref())?.map(str::to_owned);
  let services = services.clone();
  blocking(move || {
    let open = project.as_deref().map(|named| services.project(named)).transpose()?;
    let scope = open.as_ref().map_or(Scope::All, |open| Scope::Project(&open.repo_root));
    let since = query.range(local_date(now)).start();
    let data = UsageData::read(&services.workspace, scope, since, now)?;
    Ok(answer(open.map(|open| open.project.id()), &query, &data, now))
  })
  .await
}

/// The query the arguments ask for.
fn query(args: &GetUsage, today: NaiveDate) -> Result<UsageQuery, ToolError> {
  let invalid = ToolError::InvalidArguments;
  let (from, to) = (day("from", args.from.as_ref())?, day("to", args.to.as_ref())?);
  let period = if from.is_none() && to.is_none() {
    Period::Week
  } else {
    let last = to.unwrap_or(today);
    let first = match from {
      Some(first) => first,
      None => last
        .checked_sub_days(Days::new(DEFAULT_DAYS - 1))
        .ok_or_else(|| invalid(format!("`to` {last} is too early for a range")))?,
    };
    let range = DayRange::new(first, last).map_err(|_| invalid("`to` is before `from`".to_owned()))?;
    if range.days() > LONGEST_DAYS {
      return Err(invalid(format!(
        "a range covers at most {LONGEST_DAYS} days; {first} to {last} covers {}",
        range.days()
      )));
    }
    Period::Custom(range)
  };
  let group = match given("group", args.group.as_ref())? {
    None => GroupBy::Day,
    Some(name) => group_of(name).ok_or_else(|| {
      invalid(format!(
        "`group` is hour, day, task, pipeline, step, agent, model or account, not `{name}`"
      ))
    })?,
  };
  let days = period.range(today).days();
  if group == GroupBy::Hour && days > HOURLY_DAYS {
    return Err(invalid(format!(
      "`group=hour` covers ranges up to {HOURLY_DAYS} days and this one covers {days}: ask for `group=day` or a \
       shorter range"
    )));
  }
  Ok(UsageQuery {
    period,
    group,
    filter: filter(args)?,
  })
}

/// The local day given for `name`.
fn day(name: &str, value: Option<&String>) -> Result<Option<NaiveDate>, ToolError> {
  given(name, value)?
    .map(|value| {
      NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ToolError::InvalidArguments(format!("`{name}` is a local day such as 2026-10-01, not `{value}`")))
    })
    .transpose()
}

/// The value given for `name`; an empty one is refused.
fn given<'a>(name: &str, value: Option<&'a String>) -> Result<Option<&'a str>, ToolError> {
  match value {
    Some(value) if value.trim().is_empty() => Err(ToolError::InvalidArguments(format!(
      "`{name}` is empty; leave it out to not narrow by it"
    ))),
    value => Ok(value.map(String::as_str)),
  }
}

fn filter(args: &GetUsage) -> Result<UsageFilter, ToolError> {
  let account = given("account", args.account.as_ref())?;
  let text = |name: &str, value: &Option<String>| -> Result<Vec<_>, ToolError> {
    Ok(given(name, value.as_ref())?.map(Into::into).into_iter().collect())
  };
  Ok(UsageFilter {
    tasks: text("task", &args.task)?,
    runs: ids("run", given("run", args.run.as_ref())?)?,
    pipelines: text("pipeline", &args.pipeline)?,
    steps: text("step", &args.step)?,
    profiles: text("agent", &args.agent)?,
    models: text("model", &args.model)?,
    accounts: ids("account", account.filter(|value| *value != NO_ACCOUNT))?,
    no_account: account == Some(NO_ACCOUNT),
    ..UsageFilter::default()
  })
}

/// One of a filter's lists of names.
type TextField = fn(&mut UsageFilter) -> &mut Vec<Arc<str>>;

/// The filter values that, each on its own, match nothing in the range `query` covers, with their argument's
/// name.
fn unmatched(filter: &UsageFilter, projects: &[ProjectFacts], query: ReportQuery<'_>) -> Vec<Value> {
  let alone = |name: &str, only: UsageFilter, value: String| {
    let only_query = ReportQuery { filter: &only, ..query };
    (!UsageReport::matches_any(projects, only_query)).then(|| json!({ "filter": name, "value": value }))
  };
  let texts: [(&str, &[Arc<str>], TextField); 5] = [
    ("task", &filter.tasks, |only| &mut only.tasks),
    ("pipeline", &filter.pipelines, |only| &mut only.pipelines),
    ("step", &filter.steps, |only| &mut only.steps),
    ("agent", &filter.profiles, |only| &mut only.profiles),
    ("model", &filter.models, |only| &mut only.models),
  ];
  let mut found = Vec::new();
  for (name, values, field) in texts {
    for value in values {
      let mut only = UsageFilter::default();
      field(&mut only).push(value.clone());
      found.extend(alone(name, only, value.to_string()));
    }
  }
  for run in &filter.runs {
    let only = UsageFilter {
      runs: vec![*run],
      ..UsageFilter::default()
    };
    found.extend(alone("run", only, run.to_string()));
  }
  for account in &filter.accounts {
    let only = UsageFilter {
      accounts: vec![*account],
      ..UsageFilter::default()
    };
    found.extend(alone("account", only, account.to_string()));
  }
  if filter.no_account {
    let only = UsageFilter {
      no_account: true,
      ..UsageFilter::default()
    };
    found.extend(alone("account", only, NO_ACCOUNT.to_owned()));
  }
  found
}

/// The id `value` names, as the only one of a filter.
fn ids<T: std::str::FromStr>(name: &str, value: Option<&str>) -> Result<Vec<T>, ToolError> {
  value
    .map(|value| {
      value
        .parse()
        .map_err(|_| ToolError::InvalidArguments(format!("`{name}` is an id, not `{value}`")))
    })
    .into_iter()
    .collect()
}

/// The totals and the rows of `query` over `data`, with the page's link when one project was asked for.
fn answer(project: Option<ProjectId>, query: &UsageQuery, data: &UsageData, now: Timestamp) -> Value {
  let range = query.range(local_date(now));
  let report_query = ReportQuery {
    range,
    grouping: query.grouping(range),
    filter: &query.filter,
    now,
  };
  let Grouped { summary, rows } = grouped(&data.projects, report_query, query.group);
  // Each row's seconds are the step of the rounded running total, so they add up to the total's.
  let mut active_ms = 0;
  let rows: Vec<Value> = rows
    .iter()
    .map(|row| {
      let mut value = key(&row.key, data);
      let link = page_link(row_project(&row.key, project), &narrowed(query, &row.key));
      let seconds = seconds(active_ms + row.agent_ms) - seconds(active_ms);
      active_ms += row.agent_ms;
      let fields = value.as_object_mut().expect("a row's key is an object");
      fields.extend(
        figures(row, seconds)
          .as_object()
          .expect("figures are an object")
          .clone(),
      );
      fields.insert("link".to_owned(), json!(link));
      value
    })
    .collect();
  json!({
    "project": project.map(|project| project.to_string()),
    "projects": data.projects.iter().map(project_json).collect::<Vec<_>>(),
    "unread_projects": data.unread.iter().map(|name| &**name).collect::<Vec<_>>(),
    "unread_runs": data.unread_runs,
    "from": range.first().to_string(),
    "to": range.last().to_string(),
    "group": group_name(query.group),
    "total": total(&summary),
    "rows": rows,
    "unmatched": unmatched(&query.filter, &data.projects, report_query),
    "link": page_link(project, query),
  })
}

fn project_json(project: &ProjectFacts) -> Value {
  json!({ "id": project.project.to_string(), "name": &*project.name })
}

fn tokens(usage: orchester_core::run::TokenUsage) -> Value {
  json!({
    "spent": usage.counted(),
    "input": usage.input,
    "cached_input": usage.cached_input,
    "output": usage.output,
  })
}

fn total(summary: &Summary) -> Value {
  json!({
    "tokens": tokens(summary.usage),
    "active_seconds": seconds(summary.agent_ms),
    "runs": summary.runs,
    "finished_runs": summary.finished,
    "failed_runs": summary.failed,
  })
}

fn figures(row: &GroupRow, active_seconds: u64) -> Value {
  json!({
    "tokens": tokens(row.usage),
    "active_seconds": active_seconds,
    "runs": row.runs,
  })
}

/// `ms` in whole seconds, rounded to the nearest.
const fn seconds(ms: u64) -> u64 {
  ms.saturating_add(500) / 1000
}

/// What a row stands for.
fn key(key: &GroupKey, data: &UsageData) -> Value {
  let named = |named: Option<&Named>| named.map(|named| json!({ "id": named.id.to_string(), "name": &*named.name }));
  match key {
    GroupKey::Bucket(bucket) => match bucket.hour {
      Some(hour) => json!({ "day": bucket.date.to_string(), "hour": hour }),
      None => json!({ "day": bucket.date.to_string() }),
    },
    GroupKey::Task {
      project, key, title, ..
    } => json!({ "project": project.to_string(), "task": &**key, "title": &**title }),
    GroupKey::Pipeline(pipeline) => json!({ "pipeline": named(pipeline.as_ref()) }),
    GroupKey::Step(step) => json!({ "step": &**step }),
    GroupKey::Agent(profile) => json!({ "agent": named(profile.as_ref()) }),
    GroupKey::Reviewer(model) => json!({ "agent": null, "reviewer_model": &**model }),
    GroupKey::Model(model) => json!({ "model": model.as_deref() }),
    GroupKey::Account(account) => {
      let name = account.and_then(|id| data.accounts.iter().find(|known| known.id() == id));
      json!({
        "account": account.map(|id| json!({
          "id": id.to_string(),
          "name": name.and_then(|known| known.name()).map(|name| name.as_str()),
        })),
      })
    }
  }
}

/// The Usage page of `project`, or every project's, showing `query` as the page shows it: grouped by time.
fn page_link(project: Option<ProjectId>, query: &UsageQuery) -> String {
  let query = Box::new(query.on_page());
  match project {
    Some(project) => Link::Usage(project, query),
    None => Link::UsageEverywhere(query),
  }
  .path()
}

/// The project a row's link opens: the one asked for, or a task's own; `None`, every project's page.
fn row_project(key: &GroupKey, asked: Option<ProjectId>) -> Option<ProjectId> {
  match key {
    GroupKey::Task { project, .. } => Some(*project),
    _ => asked,
  }
}

/// `query` narrowed to what `key` stands for: one day by hour, or the row's task, pipeline, step, profile,
/// model or account as the filter. A row that names nothing to choose keeps the query.
fn narrowed(query: &UsageQuery, key: &GroupKey) -> UsageQuery {
  let mut narrowed = query.clone();
  let filter = &mut narrowed.filter;
  match key {
    GroupKey::Bucket(bucket) => {
      narrowed.period = Period::Custom(DayRange::new(bucket.date, bucket.date).expect("a day is a range of one day"));
      narrowed.group = GroupBy::Hour;
    }
    GroupKey::Task { key, .. } => filter.tasks = vec![key.clone()],
    GroupKey::Pipeline(Some(pipeline)) => filter.pipelines = vec![pipeline.id.to_string().into()],
    GroupKey::Step(step) => filter.steps = vec![step.clone()],
    GroupKey::Agent(Some(profile)) => filter.profiles = vec![profile.id.to_string().into()],
    GroupKey::Model(Some(model)) => filter.models = vec![model.clone()],
    GroupKey::Account(Some(account)) => filter.accounts = vec![*account],
    GroupKey::Account(None) => filter.no_account = true,
    GroupKey::Pipeline(None) | GroupKey::Agent(None) | GroupKey::Reviewer(_) | GroupKey::Model(None) => {}
  }
  narrowed
}
