//! Publication from an editor preview requires every referenced value in that context.
use std::path::Path;

use orchester_core::resource::{Instruction, Variable};

use super::{
  data::InstructionEntry,
  preview::{PreviewCatalog, PreviewChoice, Resolved},
};
use crate::messages::{self, Message};

/// Values required by this rule, never inferred from the other preview sources.
pub fn missing(body: &Instruction, resolved: Option<&Resolved<'_>>) -> Vec<Variable> {
  body
    .text()
    .variables()
    .iter()
    .copied()
    .filter(|variable| resolved.is_none_or(|context| context.value(*variable).is_none()))
    .collect()
}

/// Re-read context immediately before publishing; UI availability is only an affordance.
pub fn ensure_in(root: &Path, entry: &InstructionEntry, choice: &PreviewChoice) -> Result<(), Message> {
  if entry.body.text().variables().is_empty() {
    return Ok(());
  }
  let catalog = PreviewCatalog::load_in(root).map_err(|error| messages::store_error(&error))?;
  let resolved = catalog.resolve(choice);
  if let Some(variable) = missing(&entry.body, resolved.as_ref().ok()).first() {
    return Err(Message::new("instructions.errors.variableUnavailable").with("variable", variable.name().to_owned()));
  }
  Ok(())
}
/// An active run keeps the instruction version pinned when it started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedRun {
  pub task: String,
  pub number: orchester_core::run::RunNumber,
  pub version: u32,
}
pub fn retained_runs(catalog: &PreviewCatalog, instruction: orchester_core::id::ResourceId) -> Vec<RetainedRun> {
  catalog
    .tasks()
    .iter()
    .flat_map(|task| {
      task.runs.iter().filter_map(|run| {
        if run.status().is_terminal() {
          return None;
        }
        let pinned = run
          .snapshot()
          .instructions()
          .iter()
          .find(|pinned| pinned.resource == instruction)?;
        Some(RetainedRun {
          task: task.task.key().to_string(),
          number: run.number(),
          version: pinned.number.get(),
        })
      })
    })
    .collect()
}
