//! The latest run of each task as the task list shows it ([`super::data::ProjectTasks`]): where it stands, what it
//! waits for and who works on it, from the project's runs read at once ([`ProjectRuns`]).

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  conversation::{Message, latest_question},
  id::{RunId, StepExecutionId, TaskId},
  resource::{AgentProfile, StepKey},
  run::{Run, RunStatus, StepExecutor, TaskPhase, can_run_again_after},
  store::{ConversationStore, ResourceStore, RunStore},
  task::Task,
  task_group::GroupHold,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  approvals::ShownApproval,
  at_work::{Agents, FoundAgent, RunFacts},
  data::AutoApprovingRun,
  delivery::Unmerged,
  detail::run_store_error,
  list::{AwaitedPlan, AwaitedQuestion, AwaitedStep, CurrentStep, Latest, LatestRun, Phases, StepWait},
  project_runs::{ProjectRuns, RunEnds},
  run_approvals,
  unreadable::Unreadable,
};

/// What one reading finds of the tasks' runs: the latest run of each task that has one, how many runs of each
/// can be read, the active runs that auto-approve, who works on the running ones and when the merged ones merged.
pub(super) struct LatestRuns {
  pub(super) phases: Phases,
  pub(super) counts: BTreeMap<TaskId, usize>,
  pub(super) auto_approving: Vec<AutoApprovingRun>,
  pub(super) facts: RunFacts,
}

/// The latest run of each of `tasks` among the project's `runs`, a run waiting in its turn on one of the `pending`
/// tool requests or on a question of the `asking` runs waiting for a person. A task whose latest run is no group's
/// leaves `group_holds`; a latest run that succeeded and was not merged goes to `unmerged`.
pub(super) fn latest_runs<'a>(
  db: &ProjectDb,
  (tasks, runs): (impl Iterator<Item = &'a Task>, &ProjectRuns),
  (pending, asking): (&[ShownApproval], &[RunId]),
  (group_holds, unmerged): (&mut BTreeMap<TaskId, GroupHold>, &mut Unmerged),
) -> Result<LatestRuns, StoreError> {
  let mut found = LatestRuns {
    phases: Phases::new(),
    counts: BTreeMap::new(),
    auto_approving: Vec::new(),
    facts: RunFacts::default(),
  };
  let mut agents = Agents::default();
  for task in tasks {
    let listed = runs.runs_of(task.id());
    found
      .counts
      .insert(task.id(), listed.iter().filter(|run| run.is_ok()).count());
    let latest = match listed.last() {
      Some(Ok(latest)) => {
        // Only a group's first run is scheduled by the group ([`ProjectDb::task_group_for_run`]).
        if !runs.is_first_run(db, task, latest)? {
          group_holds.remove(&task.id());
        }
        let ends = runs.ends(latest);
        let (mut shown, agent) = latest_run_and_agent(db, &mut agents, task, (latest, &ends))?;
        // A run whose agent waits for a tool decision or an answer in its turn waits for a person, as one at
        // a plan does.
        if shown.phase == TaskPhase::Running
          && (pending.iter().any(|shown| shown.approval.run == latest.id()) || asking.contains(&latest.id()))
        {
          shown.phase = TaskPhase::Waiting;
        }
        if shown.phase == TaskPhase::Succeeded {
          unmerged.add(task.id(), latest);
        }
        found.note(db, (latest, &shown), (agent, &ends))?;
        if run_approvals::counts_as_auto_approving(db, task, latest)? {
          found.auto_approving.push(AutoApprovingRun {
            run: latest.id(),
            number: latest.number(),
            task: task.id(),
            key: task.key().to_string().into(),
          });
        }
        Latest::Run(Box::new(shown))
      }
      Some(Err(unreadable)) => Latest::Unreadable(Unreadable::of(unreadable)),
      None => continue,
    };
    found.phases.insert(task.id(), latest);
  }
  Ok(found)
}

impl LatestRuns {
  /// Keeps who works on `latest` while it runs (`agent`, its step's agent as reading it found it), and when it merged
  /// once it was merged in Orchester, as `ends` records it.
  fn note(
    &mut self,
    db: &ProjectDb,
    (latest, shown): (&Run, &LatestRun),
    (agent, ends): (Option<FoundAgent>, &RunEnds),
  ) -> Result<(), StoreError> {
    match shown.phase {
      TaskPhase::Running => {
        let at_work = super::at_work::read(db, latest, agent)?;
        self.facts.at_work.insert(latest.id(), at_work);
      }
      TaskPhase::Merged => {
        if let Some(merge) = &ends.merge {
          self.facts.merged_at.insert(latest.id(), merge.merged_at);
        }
      }
      _ => {}
    }
    Ok(())
  }
}

/// `latest`, a run of `task`, with when it last changed, what it waits for, who runs its step's agent and, once it was
/// interrupted, whether a person removed its worktree so that its task runs again after it (ORC-63).
pub(super) fn latest_run(db: &ProjectDb, task: &Task, latest: &Run) -> Result<LatestRun, StoreError> {
  let ends = RunEnds::read(db, latest)?;
  latest_run_and_agent(db, &mut Agents::default(), task, (latest, &ends)).map(|(shown, _)| shown)
}

/// [`latest_run`] with what the list shows of it beside the run itself, `ends`, and its step's agent as it was found,
/// so that a running run's facts don't look it up again.
fn latest_run_and_agent(
  db: &ProjectDb,
  agents: &mut Agents,
  task: &Task,
  (latest, ends): (&Run, &RunEnds),
) -> Result<(LatestRun, Option<FoundAgent>), StoreError> {
  let awaited = awaited(db, latest)?;
  let mut shown = LatestRun::of(task, (latest, ends.merge.as_ref()), ends.updated_at, awaited);
  let agent = super::at_work::step_agent(db, agents, latest, shown.step.as_ref())?;
  shown.agent = agent.as_ref().map(|found| found.provider);
  match latest.status() {
    RunStatus::Interrupted(_) => shown.ended_for_run_again = can_run_again_after(latest, ends.worktree_removed),
    RunStatus::Succeeded => shown.completed = ends.completed,
    _ => {}
  }
  Ok((shown, agent))
}

/// Each step of `run` that waits on a decision on its plan or for an answer to its question, with the
/// agent role of the step that proposed or asked it; only such a run needs its plans or its conversation
/// and its profiles read here. A plan that cannot be read is left out.
fn awaited(db: &ProjectDb, run: &Run) -> Result<Vec<AwaitedStep>, StoreError> {
  let (plans, answers) = (run.awaiting_plan(), run.awaiting_answer());
  if plans.is_empty() && answers.is_empty() {
    return Ok(Vec::new());
  }
  let mut roles = BTreeMap::new();
  for step in run.snapshot().steps() {
    if let StepExecutor::Agent { profile, .. } = step.executor()
      && let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)?
    {
      roles.insert(step.key().clone(), Arc::<str>::from(version.body().role().as_str()));
    }
  }
  let role = |step: &StepKey| roles.get(step).cloned();
  let mut waiting = Vec::new();
  if !plans.is_empty() {
    let stored = db.plans(run.id()).map_err(run_store_error)?;
    waiting.extend(plans.into_iter().filter_map(|execution| {
      Some(AwaitedStep {
        execution,
        step: CurrentStep::of_execution(run, execution),
        wait: StepWait::Plan(AwaitedPlan::of(run, &stored, execution, role)?),
      })
    }));
  }
  if !answers.is_empty() {
    let messages = db.messages(run.task())?;
    waiting.extend(answers.into_iter().map(|execution| AwaitedStep {
      execution,
      step: CurrentStep::of_execution(run, execution),
      wait: StepWait::Answer(awaited_question(run, &messages, execution, role)),
    }));
  }
  Ok(waiting)
}

/// The question `execution`, waiting for an answer, asked, among `messages`, and who asks.
fn awaited_question(
  run: &Run,
  messages: &[Message],
  execution: StepExecutionId,
  role: impl Fn(&StepKey) -> Option<Arc<str>>,
) -> Option<AwaitedQuestion> {
  let question = latest_question(messages, execution)?;
  let step = run.execution(execution)?.step();
  let agent = role(step).or_else(|| run.snapshot().body().step(step).map(|step| step.name.as_str().into()))?;
  Some(AwaitedQuestion {
    agent,
    text: question.body().as_str().into(),
  })
}
