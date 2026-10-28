//! *At work*: one line per run under way, in the task list's order, with the step it stands at and who works on it
//! (`tasks::at_work`, read with the tasks). A run that reads interrupted only because no runner looks after it stays,
//! as what was read: the page says when.

use std::sync::Arc;

use orchester_core::{
  id::{AgentAccountId, RunId, TaskId},
  run::TaskPhase,
};

use crate::tasks::{at_work::RunAtWork, data::ProjectTasks, list::CurrentStep};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtWorkLine {
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub run: RunId,
  pub step: Option<CurrentStep>,
  /// When it started, its steps and its agent; `None` when the run was read before it ran.
  pub work: Option<RunAtWork>,
  /// No runner looks after it now: it is shown as it was read.
  pub lost: bool,
}

/// The runs at work in `tasks`.
pub fn at_work(tasks: &ProjectTasks) -> Vec<AtWorkLine> {
  tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let latest = tasks.phases.get(&task.id())?.run()?;
      let lost = match latest.phase {
        TaskPhase::Running => false,
        TaskPhase::Interrupted if latest.lost_runner => true,
        _ => return None,
      };
      Some(AtWorkLine {
        task: task.id(),
        key: task.key().to_string().into(),
        title: task.details().title.as_str().into(),
        run: latest.id,
        step: latest.step.clone(),
        work: tasks.run_facts.at_work.get(&latest.id).cloned(),
        lost,
      })
    })
    .collect()
}

/// How many of `lines` run on `account`: *N runs on it*.
pub fn runs_on(lines: &[AtWorkLine], account: AgentAccountId) -> usize {
  lines
    .iter()
    .filter(|line| {
      line
        .work
        .as_ref()
        .and_then(|work| work.agent.as_ref())
        .is_some_and(|agent| agent.account == Some(account))
    })
    .count()
}
