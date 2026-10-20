//! How a run that waits for a person's decision goes on, which the Orchestrator's agent asks for with `continue_run`
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 5): only one more retry (`allow_retry`), a return to an
//! earlier agent step with a note (`return_to`), or, for a run whose merge stopped at conflicts, *Ask an agent to
//! resolve* (`resolve_conflicts`), checked as the tool checks them and always proposed to the person, who applies it
//! through the same tool. Raising limits or budgets, resuming after the daily budget and accepting a step stay the
//! person's: the chat's call is refused.

use std::path::Path;

use orchester_core::{
  orchestrator::{
    StopReason,
    change::{Change, ContinueChoice, ProposedChange, StepName},
  },
  run::{Run, RunStatus},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::run_subject;
use crate::{
  app_mcp::{self, ContinueOffered, ContinueRun, ToolError, continue_offered, find_run},
  messages::Message,
  tasks::integration_jobs::{IntegrationJobs, JobState},
};

/// The decision `continue_run` with `arguments` takes on the run it names in the project at `root`, and the key of its
/// task; refused for a choice that is the person's, and as the tool refuses it. `jobs` are the project's merges.
/// Blocks on SQLite and git.
pub(super) fn continue_change(
  (root, programs, jobs): (&Path, &Programs, &IntegrationJobs),
  arguments: &JsonObject,
) -> Result<(ProposedChange, Option<String>), ToolError> {
  let asked: ContinueRun = serde_json::from_value(Value::Object(arguments.clone()))
    .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  if !matches!(
    asked.choice,
    app_mcp::ContinueChoice::AllowRetry
      | app_mcp::ContinueChoice::ReturnTo { .. }
      | app_mcp::ContinueChoice::ResolveConflicts { .. }
  ) {
    return Err(person_only());
  }
  let ContinueOffered { run, target, conflicts } = continue_offered(root, &asked, (programs, jobs))?;
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, &run.to_string())?;
  let (subject, task) = run_subject(&db, &run)?;
  let to = target.as_ref().and_then(|target| StepName::in_run(&run, target));
  let choice = match asked.choice {
    app_mcp::ContinueChoice::ReturnTo { step, note } => {
      let key = target.map_or(step, |target| target.to_string());
      ContinueChoice::ReturnTo { to, step: key, note }
    }
    app_mcp::ContinueChoice::ResolveConflicts { note, .. } => {
      // The tool found the writing step and the conflicts, or refused.
      let (Some(target), Some(conflicts)) = (target, conflicts) else {
        return Err(ToolError::InvalidArguments(
          "resolve_conflicts: nothing to resolve".to_owned(),
        ));
      };
      ContinueChoice::ResolveConflicts {
        step: target.to_string(),
        to,
        base: conflicts.base.as_str().to_owned(),
        files: conflicts.files,
        note: note.map(|note| note.trim().to_owned()).filter(|note| !note.is_empty()),
      }
    }
    app_mcp::ContinueChoice::AllowRetry => ContinueChoice::AllowRetry,
    app_mcp::ContinueChoice::RaiseLimits { .. }
    | app_mcp::ContinueChoice::RaiseStepBudget { .. }
    | app_mcp::ContinueChoice::Resume
    | app_mcp::ContinueChoice::Accept => return Err(person_only()),
  };
  let change = Change::Continue {
    run: run.id(),
    number: run.number().get(),
    attempt: attempt(&run),
    step: stopped_step(&run),
    why: StopReason::of_run(&run),
    choice,
  };
  Ok((ProposedChange { subject, change }, task))
}

/// A choice the chat may not propose: it is the person's.
fn person_only() -> ToolError {
  ToolError::refused(
    "person_only",
    vec![Message::new("orchestrator.tools.continuePersonOnly")],
  )
}

/// Refused unless run `#number` (`run`) is still in the attempt `proposed` it stopped in: one that went on since no
/// longer waits for this decision. A resolution of conflicts (`choice`) is refused as well once the run's last merge,
/// as `jobs` hold it, no longer stopped at the files it was proposed for: resolved in the editor, merged since or
/// returned from the task page. The tool checks again that the choice is offered where it continues the run.
pub(super) fn unchanged(
  (root, jobs): (&Path, &IntegrationJobs),
  (run, number): (&str, u32),
  (proposed, choice): (u32, &ContinueChoice),
) -> Result<(), ToolError> {
  let run = find_run(&ProjectDb::open(root)?, run)?;
  let still_conflicts = match choice {
    ContinueChoice::ResolveConflicts { files, .. } => {
      let job = jobs.of(run.id()).state();
      matches!(job.merge, JobState::Failed(_)) && job.conflicts == *files
    }
    ContinueChoice::AllowRetry | ContinueChoice::ReturnTo { .. } => true,
  };
  if attempt(&run) == proposed && still_conflicts {
    return Ok(());
  }
  Err(ToolError::refused(
    "changed_since_proposed",
    vec![Message::new("orchestrator.proposal.runMovedOn").with("run", number.to_string())],
  ))
}

/// Adds the choice of a decision `choice` to the tool's `arguments`, as `continue_run` reads it.
pub(super) fn words(choice: &ContinueChoice, arguments: &mut JsonObject) {
  let choice = match choice {
    ContinueChoice::AllowRetry => json!({ "kind": "allow_retry" }),
    ContinueChoice::ReturnTo { step, note, .. } => json!({ "kind": "return_to", "step": step, "note": note }),
    ContinueChoice::ResolveConflicts { step, note, .. } => {
      json!({ "kind": "resolve_conflicts", "step": step, "note": note })
    }
  };
  arguments.insert("choice".to_owned(), choice);
}

/// The number of `run`'s current attempt; 0 before its first.
fn attempt(run: &Run) -> u32 {
  run.current_attempt().map_or(0, |attempt| attempt.number().get())
}

/// The step `run` stopped at and waits for a decision on, while its pipeline has it.
fn stopped_step(run: &Run) -> Option<StepName> {
  match run.status() {
    RunStatus::WaitingForPerson(stuck) => StepName::in_run(run, stuck.step()),
    _ => None,
  }
}
