//! How a run that waits for a person's decision goes on, which the Orchestrator's agent asks for with `continue_run`
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 5): only one more retry (`allow_retry`) or a return to an
//! earlier agent step with a note (`return_to`), checked as the tool checks them and always proposed to the person,
//! who applies it through the same tool. Raising limits or budgets, resuming after the daily budget and accepting a
//! step stay the person's: the chat's call is refused.

use std::path::Path;

use orchester_core::{
  orchestrator::{
    StopReason,
    change::{Change, ContinueChoice, ProposedChange, StepName},
  },
  run::{Run, RunStatus},
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::run_subject;
use crate::{
  app_mcp::{self, ContinueRun, ToolError, continue_offered, find_run},
  messages::Message,
};

/// The decision `continue_run` with `arguments` takes on the run it names in the project at `root`, and the key of its
/// task; refused for a choice that is the person's, and as the tool refuses it. Blocks on SQLite.
pub(super) fn continue_change(
  root: &Path,
  arguments: &JsonObject,
) -> Result<(ProposedChange, Option<String>), ToolError> {
  let asked: ContinueRun = serde_json::from_value(Value::Object(arguments.clone()))
    .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  if !matches!(
    asked.choice,
    app_mcp::ContinueChoice::AllowRetry | app_mcp::ContinueChoice::ReturnTo { .. }
  ) {
    return Err(person_only());
  }
  let (run, target) = continue_offered(root, &asked)?;
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, &run.to_string())?;
  let (subject, task) = run_subject(&db, &run)?;
  let choice = match asked.choice {
    app_mcp::ContinueChoice::ReturnTo { step, note } => {
      let to = target.as_ref().and_then(|target| StepName::in_run(&run, target));
      let key = target.map_or(step, |target| target.to_string());
      ContinueChoice::ReturnTo { to, step: key, note }
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
/// longer waits for this decision. The tool checks again that the choice is offered where it continues the run.
pub(super) fn unchanged(root: &Path, (run, number): (&str, u32), proposed: u32) -> Result<(), ToolError> {
  let run = find_run(&ProjectDb::open(root)?, run)?;
  if attempt(&run) == proposed {
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
