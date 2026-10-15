//! `get_usage`: the tokens and active agent time of one project's runs, or of every project's, over a
//! range of local days, filtered and grouped as the Usage page reads them (`crate::usage`), with the link
//! of the page that shows the same and, for every row, the page filtered to it.

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::ProjectId,
  time::Timestamp,
  usage::{
    DayRange, GroupBy, GroupKey, GroupRow, Named, Period, ProjectFacts, Summary, UsageFilter,
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
  usage::{Dashboard, Scope, UsageData, UsageQuery, group_name, group_of, usage_link},
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
  /// Only invocations on this CLI account, by its id.
  #[serde(default)]
  pub account: Option<String>,
}

pub async fn get_usage(services: &Services, args: GetUsage) -> Result<Value, ToolError> {
  let now = Timestamp::now();
  let query = query(&args, local_date(now))?;
  let (services, since) = (services.clone(), Dashboard::since(&query, now));
  let (project, data) = blocking(move || {
    let open = args
      .project
      .as_deref()
      .map(|named| services.project(named))
      .transpose()?;
    let scope = open.as_ref().map_or(Scope::All, |open| Scope::Project(&open.repo_root));
    let data = UsageData::read(&services.workspace, scope, since, now)?;
    Ok((open.map(|open| open.project.id()), data))
  })
  .await?;
  Ok(answer(project, &query, &data, now))
}

/// The query the arguments ask for.
fn query(args: &GetUsage, today: NaiveDate) -> Result<UsageQuery, ToolError> {
  let day = |name: &str, value: &str| {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
      .map_err(|_| ToolError::InvalidArguments(format!("`{name}` is a local day such as 2026-10-01, not `{value}`")))
  };
  let from = args.from.as_deref().map(|from| day("from", from)).transpose()?;
  let to = args.to.as_deref().map(|to| day("to", to)).transpose()?;
  let period = if from.is_none() && to.is_none() {
    Period::Week
  } else {
    let last = to.unwrap_or(today);
    let first = from.unwrap_or_else(|| last - Days::new(DEFAULT_DAYS - 1));
    let range =
      DayRange::new(first, last).map_err(|_| ToolError::InvalidArguments("`to` is before `from`".to_owned()))?;
    Period::Custom(range)
  };
  let group = match args.group.as_deref() {
    None => GroupBy::Day,
    Some(name) => group_of(name).ok_or_else(|| {
      ToolError::InvalidArguments(format!(
        "`group` is hour, day, task, pipeline, step, agent, model or account, not `{name}`"
      ))
    })?,
  };
  Ok(UsageQuery {
    period,
    group,
    filter: filter(args)?,
  })
}

fn filter(args: &GetUsage) -> Result<UsageFilter, ToolError> {
  let text = |value: &Option<String>| value.iter().map(|value| value.as_str().into()).collect();
  Ok(UsageFilter {
    tasks: text(&args.task),
    runs: ids("run", args.run.as_deref())?,
    pipelines: text(&args.pipeline),
    steps: text(&args.step),
    profiles: text(&args.agent),
    models: text(&args.model),
    accounts: ids("account", args.account.as_deref())?,
    ..UsageFilter::default()
  })
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
  let summary = UsageReport::build(&data.projects, report_query).summary;
  let rows: Vec<Value> = grouped(&data.projects, report_query, query.group)
    .iter()
    .map(|row| {
      let mut value = key(&row.key, data);
      let link = row_project(&row.key, project).map(|project| usage_link(project, &narrowed(query, &row.key)));
      let fields = value.as_object_mut().expect("a row's key is an object");
      fields.extend(figures(row).as_object().expect("figures are an object").clone());
      fields.insert("link".to_owned(), json!(link));
      value
    })
    .collect();
  json!({
    "project": project.map(|project| project.to_string()),
    "projects": data.projects.iter().map(project_json).collect::<Vec<_>>(),
    "unread_projects": data.unread.iter().map(|name| &**name).collect::<Vec<_>>(),
    "from": range.first().to_string(),
    "to": range.last().to_string(),
    "group": group_name(query.group),
    "total": total(&summary),
    "rows": rows,
    "link": project.map(|project| usage_link(project, query)),
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
    "active_seconds": summary.agent_ms / 1000,
    "runs": summary.runs,
    "finished_runs": summary.finished,
    "failed_runs": summary.failed,
  })
}

fn figures(row: &GroupRow) -> Value {
  json!({
    "tokens": tokens(row.usage),
    "active_seconds": row.agent_ms / 1000,
    "runs": row.runs,
  })
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

/// The project a row's link opens: the one asked for, or a task's own.
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
    GroupKey::Pipeline(None)
    | GroupKey::Agent(None)
    | GroupKey::Reviewer(_)
    | GroupKey::Model(None)
    | GroupKey::Account(None) => {}
  }
  narrowed
}
