//! Usage grouped one way, as a tool or a link asks for it (`docs/design/orchestrator-toolset.md`, *Usage*):
//! tokens and active agent time per hour or day, task, pipeline, step, agent profile, model or account.
//! A turn counts in the group of the invocation it belongs to; a review of auto mode's reviewer counts
//! under its run's task and pipeline and the step whose request it judged, as its own agent with its model,
//! and under no account. Active time is the invocations': per hour or day it is each one's part of that
//! stretch, as the range's is (`report::active_between`).

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use super::{
  facts::{InvocationFacts, RunFacts},
  period::{Bucket, Grouping},
  report::{Launch, Matched, Spender, Turn, UsageQuery, Within, active_between},
};
use crate::{
  id::{AgentAccountId, ProjectId, ResourceId, RunId, TaskId},
  run::TokenUsage,
};

/// What rows group by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupBy {
  Hour,
  Day,
  Task,
  Pipeline,
  Step,
  /// The agent profile, and auto mode's reviewer per model.
  Agent,
  Model,
  Account,
}

/// A pipeline or an agent profile, as a row names it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Named {
  pub id: ResourceId,
  pub name: Arc<str>,
}

/// What one row stands for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GroupKey {
  /// An hour or a day of the range.
  Bucket(Bucket),
  Task {
    project: ProjectId,
    task: TaskId,
    key: Arc<str>,
    title: Arc<str>,
  },
  /// `None` for runs whose pipeline is gone.
  Pipeline(Option<Named>),
  Step(Arc<str>),
  /// `None` for command tools, which run under no profile.
  Agent(Option<Named>),
  /// Auto mode's reviewer with this model.
  Reviewer(Arc<str>),
  /// `None` for command tools.
  Model(Option<Arc<str>>),
  /// `None` where no account was recorded: a native agent, a command tool, a reviewer, or an invocation
  /// from before accounts.
  Account(Option<AgentAccountId>),
}

/// One group's usage in the range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
  pub key: GroupKey,
  pub usage: TokenUsage,
  /// Active agent time, waits for a person left out.
  pub agent_ms: u64,
  /// Runs with a turn or an invocation in the group.
  pub runs: usize,
}

impl GroupRow {
  /// Input − cached input + output.
  pub const fn spent(&self) -> u64 {
    self.usage.counted()
  }
}

/// The range's usage by `by`: by hour or day one row per bucket in order, empty ones included; otherwise
/// the groups that used tokens or time, most spent first, then most time.
pub fn grouped(projects: &[super::ProjectFacts], query: UsageQuery<'_>, by: GroupBy) -> Vec<GroupRow> {
  let matched = Matched::of(projects, query.filter);
  let within = matched.within(query.range, query.now);
  let facet = match by {
    GroupBy::Hour => return by_bucket(&within, &query.range.buckets(Grouping::Hour)),
    GroupBy::Day => return by_bucket(&within, &query.range.buckets(Grouping::Day)),
    GroupBy::Task => Facet::Task,
    GroupBy::Pipeline => Facet::Pipeline,
    GroupBy::Step => Facet::Step,
    GroupBy::Agent => Facet::Agent,
    GroupBy::Model => Facet::Model,
    GroupBy::Account => Facet::Account,
  };
  let mut rows = Rows::default();
  for turn in &within.turns {
    rows.add_turn(key_of_turn(turn, facet), turn);
  }
  for (launch, active) in &within.launches {
    rows.add_time(key_of_launch(*launch, facet), *launch, *active);
  }
  rows.sorted()
}

/// A grouping other than by time.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Facet {
  Task,
  Pipeline,
  Step,
  Agent,
  Model,
  Account,
}

fn by_bucket(within: &Within<'_>, buckets: &[Bucket]) -> Vec<GroupRow> {
  let mut rows: Vec<(GroupRow, HashSet<(ProjectId, RunId)>)> = buckets
    .iter()
    .map(|bucket| (GroupRow::empty(GroupKey::Bucket(*bucket)), HashSet::new()))
    .collect();
  for turn in &within.turns {
    if let Some(index) = super::period::bucket_of(buckets, within.end, turn.at) {
      rows[index].0.usage += turn.usage;
      rows[index].1.insert(run_of(turn.launch));
    }
  }
  for (launch, _) in &within.launches {
    for (index, bucket) in buckets.iter().enumerate() {
      let end = buckets.get(index + 1).map_or(within.end, |next| next.start);
      if let Some(active) = active_between(launch.invocation, (bucket.start, end), within.now) {
        rows[index].0.agent_ms += active;
        rows[index].1.insert(run_of(*launch));
      }
    }
  }
  rows
    .into_iter()
    .map(|(mut row, runs)| {
      row.runs = runs.len();
      row
    })
    .collect()
}

fn key_of_turn(turn: &Turn<'_>, facet: Facet) -> GroupKey {
  match (facet, turn.by) {
    (Facet::Agent, Spender::Reviewer { model, .. }) => GroupKey::Reviewer(model.clone()),
    (Facet::Model, _) => GroupKey::Model(turn.model().cloned()),
    (Facet::Account, Spender::Reviewer { .. }) => GroupKey::Account(None),
    _ => key_of_launch(turn.launch, facet),
  }
}

fn key_of_launch(launch: Launch<'_>, facet: Facet) -> GroupKey {
  let (run, invocation): (&RunFacts, &InvocationFacts) = (launch.run, launch.invocation);
  match facet {
    Facet::Task => GroupKey::Task {
      project: launch.project.project,
      task: run.task,
      key: run.task_key.clone(),
      title: run.task_title.clone(),
    },
    Facet::Pipeline => GroupKey::Pipeline(named(run.pipeline_id, run.pipeline.as_ref())),
    Facet::Step => GroupKey::Step(invocation.step.clone()),
    Facet::Agent => GroupKey::Agent(named(invocation.profile_id, invocation.profile.as_ref())),
    Facet::Model => GroupKey::Model(invocation.model.clone()),
    Facet::Account => GroupKey::Account(invocation.account),
  }
}

fn named(id: Option<ResourceId>, name: Option<&Arc<str>>) -> Option<Named> {
  Some(Named {
    id: id?,
    name: name?.clone(),
  })
}

const fn run_of(launch: Launch<'_>) -> (ProjectId, RunId) {
  (launch.project.project, launch.run.id)
}

impl GroupRow {
  fn empty(key: GroupKey) -> Self {
    Self {
      key,
      usage: TokenUsage::default(),
      agent_ms: 0,
      runs: 0,
    }
  }
}

#[derive(Default)]
struct Rows(HashMap<GroupKey, (GroupRow, HashSet<(ProjectId, RunId)>)>);

impl Rows {
  fn entry(&mut self, key: GroupKey) -> &mut (GroupRow, HashSet<(ProjectId, RunId)>) {
    self
      .0
      .entry(key.clone())
      .or_insert_with(|| (GroupRow::empty(key), HashSet::new()))
  }

  fn add_turn(&mut self, key: GroupKey, turn: &Turn<'_>) {
    let (row, runs) = self.entry(key);
    row.usage += turn.usage;
    runs.insert(run_of(turn.launch));
  }

  fn add_time(&mut self, key: GroupKey, launch: Launch<'_>, active: u64) {
    let (row, runs) = self.entry(key);
    row.agent_ms += active;
    runs.insert(run_of(launch));
  }

  fn sorted(self) -> Vec<GroupRow> {
    let mut rows: Vec<GroupRow> = self
      .0
      .into_values()
      .map(|(mut row, runs)| {
        row.runs = runs.len();
        row
      })
      .filter(|row| row.spent() > 0 || row.agent_ms > 0)
      .collect();
    rows.sort_by(|a, b| {
      b.spent()
        .cmp(&a.spent())
        .then_with(|| b.agent_ms.cmp(&a.agent_ms))
        .then_with(|| label(&a.key).cmp(&label(&b.key)))
    });
    rows
  }
}

/// What orders rows that spent the same, so the order is stable.
fn label(key: &GroupKey) -> String {
  match key {
    GroupKey::Bucket(bucket) => bucket.start.unix_millis().to_string(),
    GroupKey::Task { key, .. } => key.to_string(),
    GroupKey::Pipeline(named) | GroupKey::Agent(named) => {
      named.as_ref().map_or_else(String::new, |named| named.name.to_string())
    }
    GroupKey::Step(name) | GroupKey::Reviewer(name) => name.to_string(),
    GroupKey::Model(model) => model.as_deref().unwrap_or_default().to_owned(),
    GroupKey::Account(account) => account.map_or_else(String::new, |account| account.to_string()),
  }
}
