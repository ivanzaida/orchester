//! `complete_run`: completes a run that passed every step and committed nothing (ORC-93), as its task Summary's
//! *Complete task* does for a run whose result is its report or work done outside the repository, and *Complete
//! anyway* for one whose pipeline expects changes, through the same completion: its report kept as the run's *Report*
//! and its task Done. Completing anyway decides for the person, so it needs their consent as any decision on a run that
//! waits for them; every completion through the tool is recorded as automation's.

use orchester_core::{run::CompletedBy, time::Timestamp};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::{runs::find_run, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  decisions::DecisionKind,
  messages::Message,
  tasks::{completion::complete_in, outside::Outside},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CompleteRun {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// Complete the run although its pipeline expects changes, as *Complete anyway* does.
  #[serde(default)]
  pub anyway: bool,
}

pub async fn complete_run(services: &Services, args: CompleteRun) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    if args.anyway {
      DecisionKind::Stuck.require(&services.workspace)?;
    }
    let open = services.project(&args.project)?;
    let run = find_run(&ProjectDb::open(&open.repo_root)?, &args.run)?.id();
    let completion = complete_in(
      (&open.repo_root, &services.programs),
      run,
      (CompletedBy::Automation, args.anyway),
      Timestamp::now(),
    )
    .map_err(refusal)?;
    services.changes.record(open.project.id(), Outside::Tasks);
    Ok(json!({
      "completion": {
        "run": run.to_string(),
        "by": completion.by.as_str(),
        "completed_at": wire::time(completion.completed_at),
      }
    }))
  })
  .await
}

/// The refusal of a completion `complete_in` refused with `message`, by a stable code.
fn refusal(message: Message) -> ToolError {
  let code = match message.key {
    "runs.complete.failure.expectsChanges" => "expects_changes",
    "runs.complete.failure.hasCommits" => "has_commits",
    "runs.complete.failure.alreadyCompleted" => "already_completed",
    "runs.complete.failure.notCompletable" => "run_not_completable",
    _ => "completion_failed",
  };
  ToolError::refused(code, vec![message])
}
