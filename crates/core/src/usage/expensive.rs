//! *Most expensive*: the tasks and the runs of the range by spent tokens, most first.

use std::{collections::HashMap, sync::Arc};

use super::{
  facts::{PipelineLabel, RunFacts, RunOutcome},
  report::Within,
};
use crate::{
  id::{ProjectId, RunId, TaskId},
  run::{RunNumber, TokenUsage},
  time::Timestamp,
};

/// A task's usage in the range, over all its runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskCost {
  pub project: ProjectId,
  pub project_name: Arc<str>,
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  /// The pipeline of its latest run in the range.
  pub pipeline: Option<PipelineLabel>,
  pub runs: usize,
  pub usage: TokenUsage,
  /// How its latest run in the range stands.
  pub outcome: RunOutcome,
}

/// A run's usage in the range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCost {
  pub project: ProjectId,
  pub project_name: Arc<str>,
  pub run: RunId,
  pub number: RunNumber,
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub pipeline: Option<PipelineLabel>,
  /// When its first invocation in the range started.
  pub started: Timestamp,
  pub agent_ms: u64,
  pub usage: TokenUsage,
  pub outcome: RunOutcome,
}

pub(super) fn runs(within: &Within<'_>) -> Vec<RunCost> {
  let mut costs: HashMap<(ProjectId, RunId), RunCost> = HashMap::new();
  for (launch, active) in &within.launches {
    let cost = costs.entry((launch.project.project, launch.run.id)).or_insert_with(|| {
      empty_run(
        launch.project.project,
        &launch.project.name,
        launch.run,
        launch.invocation.started_at,
      )
    });
    cost.agent_ms += active;
    cost.started = cost.started.min(launch.invocation.started_at);
  }
  for turn in &within.turns {
    let project = turn.launch.project;
    let cost = costs.entry((project.project, turn.run().id)).or_insert_with(|| {
      empty_run(
        project.project,
        &project.name,
        turn.run(),
        turn.launch.invocation.started_at,
      )
    });
    cost.usage += turn.usage;
  }
  let mut runs: Vec<RunCost> = costs.into_values().collect();
  runs.sort_by(|a, b| {
    b.usage
      .counted()
      .cmp(&a.usage.counted())
      .then_with(|| b.number.cmp(&a.number))
  });
  runs
}

fn empty_run(project: ProjectId, project_name: &Arc<str>, run: &RunFacts, started: Timestamp) -> RunCost {
  RunCost {
    project,
    project_name: project_name.clone(),
    run: run.id,
    number: run.number,
    task: run.task,
    key: run.task_key.clone(),
    title: run.task_title.clone(),
    pipeline: run.pipeline_label(),
    started,
    agent_ms: 0,
    usage: TokenUsage::default(),
    outcome: run.outcome,
  }
}

pub(super) fn tasks(within: &Within<'_>) -> Vec<TaskCost> {
  let mut costs: HashMap<(ProjectId, TaskId), (TaskCost, RunNumber)> = HashMap::new();
  for run in runs(within) {
    let (cost, latest) = costs.entry((run.project, run.task)).or_insert_with(|| {
      let cost = TaskCost {
        project: run.project,
        project_name: run.project_name.clone(),
        task: run.task,
        key: run.key.clone(),
        title: run.title.clone(),
        pipeline: run.pipeline.clone(),
        runs: 0,
        usage: TokenUsage::default(),
        outcome: run.outcome,
      };
      (cost, run.number)
    });
    cost.runs += 1;
    cost.usage += run.usage;
    if run.number > *latest {
      *latest = run.number;
      cost.pipeline.clone_from(&run.pipeline);
      cost.outcome = run.outcome;
    }
  }
  let mut tasks: Vec<TaskCost> = costs.into_values().map(|(cost, _)| cost).collect();
  tasks.sort_by(|a, b| {
    b.usage
      .counted()
      .cmp(&a.usage.counted())
      .then_with(|| a.key.cmp(&b.key))
  });
  tasks
}
