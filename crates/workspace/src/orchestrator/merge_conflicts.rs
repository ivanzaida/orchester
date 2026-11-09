//! A merge of a task's run that stopped at conflicts is an event of its task (ORC-768, ORC-755's
//! [`TaskEventKind::MergeConflict`]): the follower ([`super::waits`]) keeps it as a wait of the run, in the
//! conversation that leads the task (ORC-761), once per merge attempt. A merge from the task page, `merge_run` (an MCP
//! client's, or the Orchestrator's own) and the merge queue's landing record an attempt that ended at conflicts; the
//! merge queue's sync that left the run at conflicts for a person is its own attempt, by when the run left. Only the
//! newest attempt of a run that is still not merged is news, and none from before the chat followed its project's runs
//! ([`ProjectDb::run_ends_since`]). The Orchestrator's own merge that conflicted starts a turn like any other
//! (ORC-909): a conflict `merge_run` met while a turn about its run still thinks waits for that turn to end before it
//! is kept, so the turn that merged is not met by a second one about the same call.

use std::collections::HashSet;

use orchester_core::{
  id::{ProjectId, RunId},
  integration::attempt::{AttemptOutcome, MergeAsker},
  merge_queue::{Left, QueueState},
  orchestrator::{AgentWait, TaskEvent, TaskEventKind, WaitKind},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  links::{Link, TaskPage},
  tasks::detail::run_store_error,
};

/// The merges of the project's runs that stopped at conflicts and are news now, each by the key its event is kept by.
pub(super) fn current_conflicts(db: &ProjectDb, project: ProjectId) -> Result<Vec<(String, AgentWait)>, StoreError> {
  let since = db.run_ends_since()?;
  let thinking: HashSet<RunId> = db.waits_thinking()?.into_iter().map(|kept| kept.wait.run).collect();
  let mut conflicts = Vec::new();
  for recorded in db.unlanded_merge_attempts()? {
    let attempt = &recorded.attempt;
    let conflicted = matches!(attempt.outcome(), AttemptOutcome::Conflicts { .. });
    let own_under_way = attempt.asked_by() == MergeAsker::Automation && thinking.contains(&attempt.run());
    if conflicted && attempt.at() >= since && !own_under_way {
      let key = format!("event:conflict:{}:{}", attempt.run(), recorded.sequence);
      conflicts.push((key, attempt.run(), attempt.at()));
    }
  }
  for entry in db.merge_queue()? {
    if matches!(entry.state, QueueState::Left(Left::Conflicts { .. })) && entry.since >= since {
      let key = format!("event:conflict:queue:{}:{}", entry.run, entry.since.unix_millis());
      conflicts.push((key, entry.run, entry.since));
    }
  }
  let mut waits = Vec::new();
  for (key, run, at) in conflicts {
    if let Some(wait) = conflict_wait(db, project, (run, at))? {
      waits.push((key, wait));
    }
  }
  Ok(waits)
}

/// The event of run `run`'s merge that stopped at conflicts at `at`, linking to its task's Summary, where the merge
/// card tells what conflicted; `None` once the run or its task is gone.
fn conflict_wait(
  db: &ProjectDb,
  project: ProjectId,
  (run, at): (RunId, Timestamp),
) -> Result<Option<AgentWait>, StoreError> {
  let Some(run) = db.run(run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let Some(task) = db.task(run.task())? else {
    return Ok(None);
  };
  Ok(Some(AgentWait {
    kind: WaitKind::Event(TaskEvent {
      kind: TaskEventKind::MergeConflict,
      at,
    }),
    task: task.key().to_string(),
    title: task.details().title.as_str().to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
    automatic: false,
    continuing: None,
    merging: None,
  }))
}
