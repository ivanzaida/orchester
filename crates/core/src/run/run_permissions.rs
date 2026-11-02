//! A run's permissions menu (ORC-319, `15f · Permissions on a run · parts and rules` `CimC4`): how one run answers its
//! agents' requests, changed for that run alone; the agent profile and the project never change from it. Narrowing —
//! *Manual for this run*, stopping *Approve everything in this run*, revoking a rule of Approve similar — is anyone's;
//! widening — *Approve everything in this run* — is a person's alone, as it is on an approval card: automation's asking
//! for it is refused, and the run's Conversation records the refusal as it records each change.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Answerer, ApprovalRuleId, EffectiveMode, RuleScope};
use crate::time::Timestamp;

/// A change asked of how one run answers its agents' requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunPermissionChange {
  /// Manual for this run, for good: nothing switches the run back to Autopilot.
  ManualForRun,
  /// *Approve everything in this run*: the run answers its worktree commands and MCP calls itself.
  ApproveEverything,
  /// Stop approving everything: the run answers as its mode says again.
  StopApprovingEverything,
  /// Revoke a rule of Approve similar: its requests ask again from the next one.
  Revoke(ApprovalRuleId),
}

/// Someone other than a person asked to widen a run.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("only a person widens how a run answers its requests")]
pub struct WideningPersonOnly;

impl RunPermissionChange {
  /// Whether the change lets the run do more without a person.
  pub const fn widens(self) -> bool {
    matches!(self, Self::ApproveEverything)
  }

  /// The change, once `by` asked for it: a narrowing is anyone's, a widening a person's alone.
  pub const fn asked_by(self, by: Answerer) -> Result<Self, WideningPersonOnly> {
    match (self.widens(), by) {
      (true, Answerer::Automation) => Err(WideningPersonOnly),
      _ => Ok(self),
    }
  }
}

/// What a run's Conversation records of a change from its permissions menu, besides its switch to Manual, which the
/// run keeps itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PermissionEventKind {
  /// The run began approving everything; the mode it answered in before.
  Everything { was: EffectiveMode },
  /// The run stopped approving everything, back to its mode.
  Stopped { back_to: EffectiveMode },
  /// A rule of Approve similar was revoked: its requests ask again.
  Revoked { scope: RuleScope },
  /// Automation asked the run to approve everything, and was refused.
  Refused,
}

/// A change from a run's permissions menu: when, by whom and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionEvent {
  pub at: Timestamp,
  pub by: Answerer,
  pub kind: PermissionEventKind,
}
