//! The card a navigation points at on the page it opens (`Needs you · where each item leads` `icGHG`): the
//! Needs you panel opens a task's Summary at the decision that waits, which the page brings into view and
//! rings for a moment. The pointer is taken once, by the card it names; any other navigation drops it, so a
//! card decided elsewhere in the meantime leaves nothing to ring later.

use orchester_core::{id::ProjectId, project::TaskKey};

/// A decision card of a task's Summary a navigation points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointed {
  pub project: ProjectId,
  pub task: TaskKey,
  pub card: PointedCard,
}

impl lurq::app::component::DevtoolsInspectable for Pointed {}

/// Which decision of the Summary: the plan under review (`JY0xW`), raising the limit the run stopped at
/// (`krC4n`), or the decision after a step's retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointedCard {
  Plan,
  RaiseLimit,
  Retries,
}
