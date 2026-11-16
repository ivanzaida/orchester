//! A run whose branch took in commits from branches other than its target (`orchester_core::integration::TakenIn`):
//! merging it brings them into the target with its agent's work, so no tool merges it. The person merges it from the
//! window, where the Changes tab says what it brings, or applies a proposal of the Orchester chat.

use super::super::runs::find_run;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::changes::{RunRange, run_taken_in},
};

/// Refuses `merge_run` of `run` in `project` while its branch took in commits from other branches.
pub(super) async fn refuse_taken_in(services: &Services, (project, run): (&str, &str)) -> Result<(), ToolError> {
  let services = services.clone();
  let (project, run) = (project.to_owned(), run.to_owned());
  blocking(move || {
    let open = services.project(&project)?;
    let db = orchester_store::ProjectDb::open(&open.repo_root)?;
    let found = find_run(&db, &run)?;
    let Some(taken) = run_taken_in(&open.repo_root, &services.programs, &RunRange::of(&found)) else {
      return Ok(());
    };
    let from = taken.from.map_or_else(String::new, |branch| branch.as_str().to_owned());
    let key = if from.is_empty() {
      "appMcp.errors.tookInOtherBranches"
    } else {
      "appMcp.errors.tookInBranch"
    };
    let why = Message::new(key)
      .with("commits", taken.commits.to_string())
      .with("branch", from);
    Err(ToolError::refused("took_in_other_branches", vec![why]))
  })
  .await
}
