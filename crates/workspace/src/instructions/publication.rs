//! Publication requires every variable a rule uses to have a value at each of its injection events
//! (05.A `m2WjL`); which preview the person picked does not matter.
use std::collections::BTreeSet;

use orchester_core::resource::{InjectionEvent, PromptText, Variable};

use super::preview::PreviewCatalog;

/// A variable of a rule without a value at one of the rule's injection events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unavailable {
  pub variable: Variable,
  /// The first of the rule's events at which it has none.
  pub event: InjectionEvent,
}

/// The variables of `text` that have no value at one of `events`, in order of first use.
pub fn missing(text: &PromptText, events: &BTreeSet<InjectionEvent>) -> Vec<Unavailable> {
  text
    .variables()
    .iter()
    .filter_map(|&variable| {
      events
        .iter()
        .find(|event| !variable.has_value_at(**event))
        .map(|&event| Unavailable { variable, event })
    })
    .collect()
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
