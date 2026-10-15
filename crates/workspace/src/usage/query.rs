//! What the Usage page shows and `get_usage` answers: the period, the grouping and the filters
//! (`docs/design/orchestrator-toolset.md`, *Usage* and *Links*), and how they read and write as the
//! parameters of a link's query. A parameter left out is the page's default: the last 7 days, by day, no
//! filter. Repeated parameters choose several values (`model=gpt-5&model=claude-opus-5-5`).
//!
//! | Parameter | Values |
//! |---|---|
//! | `period` | `today`, `7d`, `30d` (not with `from`/`to`) |
//! | `from`, `to` | a custom range of local days, `2026-10-01`, both given, at most 366 days |
//! | `project` | a project's id, on every project's page |
//! | `group` | `hour`, `day`, `task`, `pipeline`, `step`, `agent`, `model`, `account`; left out, `hour` for today |
//! | `cli` | `codex_cli`, `claude_code_cli`, `anthropic_api`, `openai_api` |
//! | `model`, `tag`, `step` | a name |
//! | `agent`, `pipeline` | a profile's or pipeline's id, or its name |
//! | `task` | a task's key (`KON-12`) or id |
//! | `run`, `account` | an id |
//!
//! An empty value (`model=`) is refused, as is a range longer than [`LONGEST_DAYS`].

use std::sync::Arc;

use chrono::NaiveDate;
use orchester_core::{
  connection::AgentProvider,
  usage::{DayRange, GroupBy, Grouping, Period, UsageFilter},
};
use thiserror::Error;

/// The parameters, in the order a link writes them.
const PERIOD: &str = "period";
const FROM: &str = "from";
const TO: &str = "to";
const GROUP: &str = "group";
const PROJECT: &str = "project";
const CLI: &str = "cli";
const MODEL: &str = "model";
const AGENT: &str = "agent";
const PIPELINE: &str = "pipeline";
const TAG: &str = "tag";
const TASK: &str = "task";
const RUN: &str = "run";
const STEP: &str = "step";
const ACCOUNT: &str = "account";

/// The period, the grouping and the filters. Only every project's page chooses projects: a project's page
/// is the project's, and `get_usage` takes its project apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UsageQuery {
  pub period: Period,
  pub group: GroupBy,
  pub filter: UsageFilter,
}

impl Default for UsageQuery {
  fn default() -> Self {
    Self {
      period: Period::Week,
      group: GroupBy::Day,
      filter: UsageFilter::default(),
    }
  }
}

/// A parameter the query does not understand.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("`{value}` is not a value of `{param}`")]
pub struct QueryError {
  pub param: &'static str,
  pub value: String,
}

impl UsageQuery {
  /// The days shown when today is `today`.
  pub fn range(&self, today: NaiveDate) -> DayRange {
    self.period.range(today)
  }

  /// How *Tokens over time* groups `range`: by hour when asked and offered (up to seven days), else by day.
  pub fn grouping(&self, range: DayRange) -> Grouping {
    if self.group == GroupBy::Hour && range.offers_hours() {
      Grouping::Hour
    } else {
      Grouping::Day
    }
  }

  /// The query with `period` chosen: *Today* reads best by hour, and a range longer than a week only by
  /// day; another grouping is kept.
  pub fn with_period(&self, period: Period, today: NaiveDate) -> Self {
    let group = match (period, self.group) {
      (Period::Today, GroupBy::Hour | GroupBy::Day) => GroupBy::Hour,
      (_, GroupBy::Hour) if !period.range(today).offers_hours() => GroupBy::Day,
      (_, group) => group,
    };
    Self {
      period,
      group,
      filter: self.filter.clone(),
    }
  }

  /// The query as the page shows it: the page groups *Tokens over time* by hour or by day only (*Group by*,
  /// `diJNR`), so a grouping by task, pipeline, step, agent, model or account, which `get_usage`'s rows
  /// have, is by day on the page.
  pub fn on_page(&self) -> Self {
    let group = match self.group {
      GroupBy::Hour | GroupBy::Day => self.group,
      GroupBy::Task | GroupBy::Pipeline | GroupBy::Step | GroupBy::Agent | GroupBy::Model | GroupBy::Account => {
        GroupBy::Day
      }
    };
    Self { group, ..self.clone() }
  }

  /// The query with `filter` instead of its own.
  pub fn with_filter(&self, filter: UsageFilter) -> Self {
    Self { filter, ..self.clone() }
  }

  /// The parameters that differ from the defaults, decoded, in their order.
  pub fn params(&self) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    match self.period {
      Period::Week => {}
      Period::Today => params.push((PERIOD, "today".to_owned())),
      Period::Month => params.push((PERIOD, "30d".to_owned())),
      Period::Custom(range) => {
        params.push((FROM, range.first().to_string()));
        params.push((TO, range.last().to_string()));
      }
    }
    if self.group != default_group(self.period) {
      params.push((GROUP, group_name(self.group).to_owned()));
    }
    let filter = &self.filter;
    params.extend(filter.projects.iter().map(|project| (PROJECT, project.to_string())));
    params.extend(filter.clis.iter().map(|cli| (CLI, cli_name(*cli).to_owned())));
    let texts = [
      (MODEL, &filter.models),
      (AGENT, &filter.profiles),
      (PIPELINE, &filter.pipelines),
      (TAG, &filter.tags),
      (TASK, &filter.tasks),
    ];
    for (name, values) in texts {
      params.extend(values.iter().map(|value| (name, value.to_string())));
    }
    params.extend(filter.runs.iter().map(|run| (RUN, run.to_string())));
    params.extend(filter.steps.iter().map(|step| (STEP, step.to_string())));
    params.extend(filter.accounts.iter().map(|account| (ACCOUNT, account.to_string())));
    params
  }

  /// The query the parameters `take` hands out name: it is asked for a parameter by name, decoded, once for
  /// each value, until it has none left. Parameters it is not asked for are the caller's to refuse.
  pub fn from_params<E>(
    mut take: impl FnMut(&'static str) -> Result<Option<String>, E>,
  ) -> Result<Self, ParamsError<E>> {
    let mut all = |name: &'static str| -> Result<Vec<String>, ParamsError<E>> {
      let mut values = Vec::new();
      while let Some(value) = take(name).map_err(ParamsError::Source)? {
        values.push(value);
      }
      Ok(values)
    };
    let period = one(PERIOD, all(PERIOD)?)?;
    let (from, to) = (one(FROM, all(FROM)?)?, one(TO, all(TO)?)?);
    let group = one(GROUP, all(GROUP)?)?;
    let mut filter = UsageFilter {
      projects: parsed(PROJECT, all(PROJECT)?, |value| value.parse().ok())?,
      clis: parsed(CLI, all(CLI)?, cli_of)?,
      ..UsageFilter::default()
    };
    let text = |value: &str| (!value.trim().is_empty()).then(|| Arc::from(value));
    filter.models = parsed(MODEL, all(MODEL)?, text)?;
    filter.profiles = parsed(AGENT, all(AGENT)?, text)?;
    filter.pipelines = parsed(PIPELINE, all(PIPELINE)?, text)?;
    filter.tags = parsed(TAG, all(TAG)?, text)?;
    filter.tasks = parsed(TASK, all(TASK)?, text)?;
    filter.runs = parsed(RUN, all(RUN)?, |value| value.parse().ok())?;
    filter.steps = parsed(STEP, all(STEP)?, text)?;
    filter.accounts = parsed(ACCOUNT, all(ACCOUNT)?, |value| value.parse().ok())?;
    let period = period_of(period, from, to)?;
    Ok(Self {
      period,
      group: match group {
        Some(value) => group_of(&value).ok_or_else(|| invalid(GROUP, value))?,
        None => default_group(period),
      },
      filter,
    })
  }
}

/// Why parameters do not read as a query: one of them, or the source that hands them out.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParamsError<E> {
  #[error(transparent)]
  Query(#[from] QueryError),
  #[error("the query could not be read")]
  Source(E),
}

/// The grouping of a link that names none: *Today* by hour, as pressing it shows, any other period by day.
fn default_group(period: Period) -> GroupBy {
  if period == Period::Today {
    GroupBy::Hour
  } else {
    GroupBy::Day
  }
}

fn invalid(param: &'static str, value: String) -> QueryError {
  QueryError { param, value }
}

/// The one value of `param`; a second one is refused.
fn one(param: &'static str, values: Vec<String>) -> Result<Option<String>, QueryError> {
  let mut values = values.into_iter();
  let first = values.next();
  match values.next() {
    Some(second) => Err(invalid(param, second)),
    None => Ok(first),
  }
}

fn parsed<T>(param: &'static str, values: Vec<String>, read: impl Fn(&str) -> Option<T>) -> Result<Vec<T>, QueryError> {
  values
    .into_iter()
    .map(|value| read(&value).ok_or_else(|| invalid(param, value)))
    .collect()
}

fn period_of(period: Option<String>, from: Option<String>, to: Option<String>) -> Result<Period, QueryError> {
  let day = |param: &'static str, value: String| {
    NaiveDate::parse_from_str(&value, DAY_FORMAT).map_err(|_| invalid(param, value))
  };
  match (period, from, to) {
    (None, None, None) => Ok(Period::Week),
    (Some(period), None, None) => match period.as_str() {
      "today" => Ok(Period::Today),
      "7d" => Ok(Period::Week),
      "30d" => Ok(Period::Month),
      _ => Err(invalid(PERIOD, period)),
    },
    (None, Some(from), Some(to)) => {
      let (first, last) = (day(FROM, from)?, day(TO, to.clone())?);
      DayRange::new(first, last)
        .ok()
        .filter(|range| range.days() <= LONGEST_DAYS)
        .map(Period::Custom)
        .ok_or_else(|| invalid(TO, to))
    }
    (Some(period), ..) => Err(invalid(PERIOD, period)),
    (None, Some(from), None) => Err(invalid(FROM, from)),
    (None, None, Some(to)) => Err(invalid(TO, to)),
  }
}

/// Why the days typed for *Custom…* show no range.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CustomRangeError {
  #[error("a day is typed as 2026-09-01")]
  Unreadable,
  #[error("the range ends before it starts")]
  Reversed,
  #[error("a range covers at most 366 days")]
  TooLong,
}

/// The most days a range covers: a year, a leap day included.
pub const LONGEST_DAYS: u64 = 366;

/// The range *Custom…* shows for the days typed as `from` and `to` (`2026-09-01`, spaces around them
/// ignored), at most [`LONGEST_DAYS`] long.
pub fn custom_range(from: &str, to: &str) -> Result<DayRange, CustomRangeError> {
  let day = |text: &str| NaiveDate::parse_from_str(text.trim(), DAY_FORMAT).map_err(|_| CustomRangeError::Unreadable);
  let range = DayRange::new(day(from)?, day(to)?).map_err(|_| CustomRangeError::Reversed)?;
  if range.days() > LONGEST_DAYS {
    return Err(CustomRangeError::TooLong);
  }
  Ok(range)
}

/// How a day is written in a link and typed in *Custom…*.
pub const DAY_FORMAT: &str = "%Y-%m-%d";

/// The name a grouping has in a link and in `get_usage`.
pub const fn group_name(group: GroupBy) -> &'static str {
  match group {
    GroupBy::Hour => "hour",
    GroupBy::Day => "day",
    GroupBy::Task => "task",
    GroupBy::Pipeline => "pipeline",
    GroupBy::Step => "step",
    GroupBy::Agent => "agent",
    GroupBy::Model => "model",
    GroupBy::Account => "account",
  }
}

pub fn group_of(name: &str) -> Option<GroupBy> {
  [
    GroupBy::Hour,
    GroupBy::Day,
    GroupBy::Task,
    GroupBy::Pipeline,
    GroupBy::Step,
    GroupBy::Agent,
    GroupBy::Model,
    GroupBy::Account,
  ]
  .into_iter()
  .find(|group| group_name(*group) == name)
}

/// The name a CLI has in a link and in `get_usage`, as Orchester's tools name providers.
pub const fn cli_name(cli: AgentProvider) -> &'static str {
  match cli {
    AgentProvider::CodexCli => "codex_cli",
    AgentProvider::ClaudeCodeCli => "claude_code_cli",
    AgentProvider::AnthropicApi => "anthropic_api",
    AgentProvider::OpenAiApi => "openai_api",
  }
}

pub fn cli_of(name: &str) -> Option<AgentProvider> {
  [
    AgentProvider::CodexCli,
    AgentProvider::ClaudeCodeCli,
    AgentProvider::AnthropicApi,
    AgentProvider::OpenAiApi,
  ]
  .into_iter()
  .find(|cli| cli_name(*cli) == name)
}
