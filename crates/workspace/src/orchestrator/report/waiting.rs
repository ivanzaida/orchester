//! What waits for the person in the daily report, as it stands when the report is read: the agents' tool requests and
//! their questions with suggested answers, the steps of tasks' latest runs that wait for a message (but those of such
//! a question) or whose plan waits for a decision, and the Orchestrator's proposals no person decided yet. Each links
//! to its card: a run's on its task's Conversation, a proposal on its Orchestrator conversation.

use orchester_core::{
  id::{ProjectId, RunId, StepExecutionId},
  orchestrator::{
    EntryKind,
    change::StepName,
    report::{PersonWait, PersonWaitKind},
  },
  run::Run,
  task::Task,
};

use super::facts::DayFacts;
use crate::links::{Link, TaskPage};

/// What waits for the person among `facts`, in project `project`: tool requests oldest first, then questions, the
/// steps that wait, and the proposals.
pub(super) fn of(facts: &DayFacts, project: ProjectId) -> Vec<PersonWait> {
  let mut waits = Vec::new();
  for approval in &facts.approvals {
    waits.push(wait(
      facts,
      project,
      (PersonWaitKind::Approval, approval.run),
      (Some(approval.action.clone()), Some(approval.requested_at)),
    ));
  }
  for asked in &facts.questions {
    let request = &asked.request;
    let step = facts
      .run(request.run())
      .and_then(|(_, run)| step_name(run, request.execution()));
    waits.push(wait(
      facts,
      project,
      (PersonWaitKind::Question, request.run()),
      (step, Some(request.asked_at())),
    ));
  }
  for task in &facts.tasks {
    let Some(run) = task.latest().filter(|run| !run.status().is_terminal()) else {
      continue;
    };
    let asked = |execution: StepExecutionId| {
      facts
        .questions
        .iter()
        .any(|asked| asked.request.execution() == execution)
    };
    let steps = run
      .awaiting_answer()
      .into_iter()
      .filter(|execution| !asked(*execution))
      .map(|execution| (PersonWaitKind::Message, execution))
      .chain(
        run
          .awaiting_plan()
          .into_iter()
          .map(|execution| (PersonWaitKind::Plan, execution)),
      );
    for (kind, execution) in steps {
      waits.push(wait(
        facts,
        project,
        (kind, run.id()),
        (step_name(run, execution), None),
      ));
    }
  }
  waits.extend(proposals(facts, project));
  waits
}

/// A wait of `kind` in run `run`, about `what`, since `since`, linking to its card on the run's task's Conversation.
fn wait(
  facts: &DayFacts,
  project: ProjectId,
  (kind, run): (PersonWaitKind, RunId),
  (what, since): (Option<String>, Option<orchester_core::time::Timestamp>),
) -> PersonWait {
  let found: Option<(&Task, &Run)> = facts.run(run);
  PersonWait {
    kind,
    task: found.map(|(task, _)| task.key().to_string()),
    number: found.map(|(_, run)| run.number().get()),
    what,
    since,
    link: found.map_or_else(
      || Link::Project(project).path(),
      |(task, _)| Link::Task(project, task.key().clone(), TaskPage::Conversation).path(),
    ),
  }
}

/// The Orchestrator's proposals that wait for a person, each linking to its conversation.
fn proposals(facts: &DayFacts, project: ProjectId) -> impl Iterator<Item = PersonWait> + '_ {
  facts.chats.iter().flat_map(move |(chat, entries)| {
    entries.iter().filter_map(move |entry| match &entry.kind {
      EntryKind::Action(action) if action.waits() => Some(PersonWait {
        kind: PersonWaitKind::Proposal,
        task: action.task.clone(),
        number: None,
        what: Some(action.tool.clone()),
        since: Some(entry.at),
        link: Link::Orchestrator(project, *chat).path(),
      }),
      _ => None,
    })
  })
}

/// The name of the step `execution` of `run` runs, while its pipeline has it.
fn step_name(run: &Run, execution: StepExecutionId) -> Option<String> {
  let step = run.execution(execution)?.step();
  StepName::in_run(run, step).map(|step| step.name)
}
