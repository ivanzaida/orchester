//! What an action of the Orchestrator chat acted on besides a task, so its line names it in people's words: the
//! module a module's tool made, changed, accepted or delivered, or that a change of a task closed it.

use serde::{Deserialize, Serialize};

use crate::id::ModuleId;

/// What an action acted on besides a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionAbout {
  /// The module of `create_module`, `update_module`, `start_acceptance` or `deliver_module`: its name, its id when
  /// the tool told it, and whether the change closed it.
  Module {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<ModuleId>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    closed: bool,
  },
  /// A change of a task that closed it: canceled, or marked done.
  TaskClosed { canceled: bool },
}
