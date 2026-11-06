//! What a subscription that ended by itself leaves in its conversation (ORC-764): the line that says why it ended
//! (*Every task of Mobile release is Done — watching ended*) and, with *Let the Orchestrator answer agents* on, the one
//! Orchestrator turn it starts by itself to write the summary of its module or task. That turn changes nothing, as a
//! daily report's does.

use serde::{Deserialize, Serialize};

use super::{EndReason, Subscription, WatchTarget};
use crate::time::Timestamp;

/// A subscription that ended by itself, as its conversation keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchSummary {
  /// The module or the task it watched.
  pub target: WatchTarget,
  /// The module's name, or the task's key as people read it (`KON-41`).
  pub name: String,
  /// The task's title; `None` for a module.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  pub reason: EndReason,
  /// The person's order for it; `None` when the project's default order applied.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<String>,
  /// Since when it led its target.
  pub since: Timestamp,
  /// The module's page or the task's Summary.
  pub link: String,
  /// It starts one Orchestrator turn by itself, which writes the summary: *Let the Orchestrator answer agents* was on
  /// and the Orchestrator had what it runs on when it was kept.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub automatic: bool,
}

impl WatchSummary {
  /// The key the summary of `subscription` is kept by: one per subscription, so it ends with one line and one turn
  /// at most, however often its end is looked for.
  pub fn key_of(subscription: &Subscription) -> String {
    let target = match subscription.target() {
      WatchTarget::Task(task) => format!("task:{task}"),
      WatchTarget::Module(module) => format!("module:{module}"),
      WatchTarget::WithoutModule => "without_module".to_owned(),
      WatchTarget::Project => "project".to_owned(),
    };
    format!(
      "watch:{}:{target}:{}",
      subscription.chat(),
      subscription.since().unix_millis()
    )
  }
}
