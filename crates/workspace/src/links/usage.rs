//! The filters of a link to Usage (`docs/design/orchestrator-toolset.md`, "Tools", `get_usage`): the days, how
//! the usage is grouped, and what it is narrowed to, each a query parameter of the page's path, e.g.
//! `/project/<id>/usage?from=2026-10-01&to=2026-10-15&group=task&agent=<profile id>`. The router reads them with
//! [`UsageLink::set`] and writes them with [`UsageLink::path`], so the names live here alone.

use chrono::NaiveDate;
use orchester_core::{
  id::{AgentAccountId, ResourceId, RunId},
  project::TaskKey,
  resource::{ModelId, StepKey},
};

use super::with_query;

/// What a link to Usage shows; every filter left out admits everything. A plain record: each filter stands
/// alone, and the page says what it makes of a range that ends before it starts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UsageLink {
  /// The first day, in the computer's time zone.
  pub from: Option<NaiveDate>,
  /// The last day, included.
  pub to: Option<NaiveDate>,
  pub group: Option<UsageGroup>,
  pub task: Option<TaskKey>,
  pub run: Option<RunId>,
  pub pipeline: Option<ResourceId>,
  pub step: Option<StepKey>,
  /// An agent profile.
  pub agent: Option<ResourceId>,
  pub model: Option<ModelId>,
  pub account: Option<AgentAccountId>,
}

/// What Usage groups its figures by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UsageGroup {
  Day,
  Task,
  Pipeline,
  Step,
  Agent,
  Model,
  Account,
}

impl UsageGroup {
  pub const ALL: [Self; 7] = [
    Self::Day,
    Self::Task,
    Self::Pipeline,
    Self::Step,
    Self::Agent,
    Self::Model,
    Self::Account,
  ];

  /// The name in a link's query and in the tools' arguments.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Day => "day",
      Self::Task => "task",
      Self::Pipeline => "pipeline",
      Self::Step => "step",
      Self::Agent => "agent",
      Self::Model => "model",
      Self::Account => "account",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|group| group.name() == name)
  }
}

/// A query parameter of a link to Usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UsageParam {
  From,
  To,
  Group,
  Task,
  Run,
  Pipeline,
  Step,
  Agent,
  Model,
  Account,
}

impl UsageParam {
  /// In the order a link writes them.
  pub const ALL: [Self; 10] = [
    Self::From,
    Self::To,
    Self::Group,
    Self::Task,
    Self::Run,
    Self::Pipeline,
    Self::Step,
    Self::Agent,
    Self::Model,
    Self::Account,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::From => "from",
      Self::To => "to",
      Self::Group => "group",
      Self::Task => "task",
      Self::Run => "run",
      Self::Pipeline => "pipeline",
      Self::Step => "step",
      Self::Agent => "agent",
      Self::Model => "model",
      Self::Account => "account",
    }
  }
}

/// A value a Usage parameter cannot hold: a day that is not `YYYY-MM-DD`, an unknown grouping, a malformed id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("not a value of this usage filter")]
pub struct InvalidUsageValue;

impl UsageLink {
  /// The value of `param` as a link writes it; `None` when it is left out.
  pub fn get(&self, param: UsageParam) -> Option<String> {
    match param {
      UsageParam::From => shown(self.from.as_ref()),
      UsageParam::To => shown(self.to.as_ref()),
      UsageParam::Group => self.group.map(|group| group.name().to_owned()),
      UsageParam::Task => shown(self.task.as_ref()),
      UsageParam::Run => shown(self.run.as_ref()),
      UsageParam::Pipeline => shown(self.pipeline.as_ref()),
      UsageParam::Step => self.step.as_ref().map(|step| step.as_str().to_owned()),
      UsageParam::Agent => shown(self.agent.as_ref()),
      UsageParam::Model => self.model.as_ref().map(|model| model.as_str().to_owned()),
      UsageParam::Account => shown(self.account.as_ref()),
    }
  }

  /// Sets `param` from its value as a link writes it.
  pub fn set(&mut self, param: UsageParam, value: &str) -> Result<(), InvalidUsageValue> {
    match param {
      UsageParam::From => self.from = Some(day(value)?),
      UsageParam::To => self.to = Some(day(value)?),
      UsageParam::Group => self.group = Some(UsageGroup::from_name(value).ok_or(InvalidUsageValue)?),
      UsageParam::Task => self.task = Some(value.parse().map_err(|_| InvalidUsageValue)?),
      UsageParam::Run => self.run = Some(value.parse().map_err(|_| InvalidUsageValue)?),
      UsageParam::Pipeline => self.pipeline = Some(value.parse().map_err(|_| InvalidUsageValue)?),
      UsageParam::Step => self.step = Some(StepKey::new(value).map_err(|_| InvalidUsageValue)?),
      UsageParam::Agent => self.agent = Some(value.parse().map_err(|_| InvalidUsageValue)?),
      UsageParam::Model => self.model = Some(ModelId::new(value).map_err(|_| InvalidUsageValue)?),
      UsageParam::Account => self.account = Some(value.parse().map_err(|_| InvalidUsageValue)?),
    }
    Ok(())
  }

  /// `usage`, the path of Usage, with these filters as its query, in [`UsageParam::ALL`]'s order.
  pub fn path(&self, usage: String) -> String {
    let values = UsageParam::ALL.map(|param| self.get(param));
    let params: Vec<(&str, Option<&str>)> = UsageParam::ALL
      .iter()
      .zip(&values)
      .map(|(param, value)| (param.name(), value.as_deref()))
      .collect();
    with_query(usage, &params)
  }
}

fn shown(value: Option<&impl ToString>) -> Option<String> {
  value.map(ToString::to_string)
}

/// A day written `YYYY-MM-DD`.
fn day(value: &str) -> Result<NaiveDate, InvalidUsageValue> {
  NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| InvalidUsageValue)
}
