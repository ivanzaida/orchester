//! Approval modes (`docs/design/approval-modes.md`): a run's step is in Autopilot only when its agent profile says
//! Autopilot, its project allows Autopilot (its *Approve mode* is Autopilot) and the run was not switched to Manual;
//! otherwise it is in Manual, for the first of those that says so. Autopilot approves its agent's requests itself;
//! Manual runs the profile's allowed commands, then asks the reviewer, then the person. The project's *Approved for
//! every run* rules answer only in Manual.
//!
//! A project's *Approve mode* ([`ApproveMode`], the person's decision of 2026-11-11) is Manual, Autopilot or Dangerous
//! auto-approve. In Dangerous auto-approve every tool request of every agent of its runs is approved at once, before
//! the allowed list, the hold rules, the reviewer or anyone is asked. Moving it up widens what agents do without a
//! person, so only a person moves it up; moving it down is anyone's. A run drops to Manual for itself, from the person
//! or automation, and never goes back to Autopilot.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::Answerer;
use crate::{permission::ApprovalMode, time::Timestamp};

/// Whether a project allows Autopilot for its agents: what [`EffectiveMode::of`] reads of its [`ApproveMode`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AllowAutopilot(bool);

impl AllowAutopilot {
  pub const OFF: Self = Self(false);
  pub const ON: Self = Self(true);

  pub const fn is_on(self) -> bool {
    self.0
  }
}

/// A project's *Approve mode* (Project settings › Runs › Approvals), from the safest up; Manual until a person moves
/// it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApproveMode {
  /// Autopilot is not allowed: every agent runs as Manual, whatever its profile says.
  #[default]
  Manual,
  /// Agents whose profile is set to Autopilot approve their own requests; the others stay Manual.
  Autopilot,
  /// Every tool request of every agent is approved at once: no allowed list, hold, reviewer or person.
  DangerousAutoApprove,
}

/// Someone other than a person asked to move a project's Approve mode up.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("only a person moves a project's Approve mode up to {0:?}")]
pub struct ApproveModePersonOnly(pub ApproveMode);

impl ApproveMode {
  /// Whether Autopilot is allowed: in Autopilot, and in Dangerous auto-approve, which approves before any mode is
  /// consulted.
  pub const fn allows_autopilot(self) -> AllowAutopilot {
    match self {
      Self::Manual => AllowAutopilot::OFF,
      Self::Autopilot | Self::DangerousAutoApprove => AllowAutopilot::ON,
    }
  }

  /// Whether every tool request is approved at once.
  pub const fn approves_everything(self) -> bool {
    matches!(self, Self::DangerousAutoApprove)
  }

  /// The mode once `by` asked for `to`: down, or what it is already, is always taken; up is a person's alone.
  pub fn changed(self, to: Self, by: Answerer) -> Result<Self, ApproveModePersonOnly> {
    if to <= self || by == Answerer::Person {
      Ok(to)
    } else {
      Err(ApproveModePersonOnly(to))
    }
  }

  /// The mode the former *Allow Autopilot* switch asks for: off is Manual; on is Autopilot, and changes nothing in
  /// Dangerous auto-approve, which allows it already.
  pub const fn with_autopilot(self, on: bool) -> Self {
    match (on, self) {
      (false, _) => Self::Manual,
      (true, Self::DangerousAutoApprove) => Self::DangerousAutoApprove,
      (true, _) => Self::Autopilot,
    }
  }
}

/// Why a run's step is in Manual, as its mode chip names it (*Manual · profile*, *· project*, *· this run*).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualBecause {
  /// The step's agent profile is set to Manual.
  Profile,
  /// The project does not allow Autopilot.
  Project,
  /// The run was switched to Manual for itself.
  Run,
}

/// When a run was switched to Manual for itself, and who switched it: its Conversation's row *Switched to Manual for
/// this run* says so. Only the first switch counts, as nothing switches a run back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManualSwitch {
  pub at: Timestamp,
  pub by: Answerer,
}

/// The mode a run's step answers its agent's requests in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "mode", content = "because", rename_all = "snake_case")]
pub enum EffectiveMode {
  Autopilot,
  Manual(ManualBecause),
}

impl EffectiveMode {
  /// The mode of a step whose profile asks for `profile`, in a project whose setting is `project`, in a run that was
  /// `switched_to_manual` or not.
  pub const fn of(profile: ApprovalMode, project: AllowAutopilot, switched_to_manual: bool) -> Self {
    match (profile, project.is_on(), switched_to_manual) {
      (ApprovalMode::Manual, ..) => Self::Manual(ManualBecause::Profile),
      (ApprovalMode::Autopilot, false, _) => Self::Manual(ManualBecause::Project),
      (ApprovalMode::Autopilot, true, true) => Self::Manual(ManualBecause::Run),
      (ApprovalMode::Autopilot, true, false) => Self::Autopilot,
    }
  }

  pub const fn is_autopilot(self) -> bool {
    matches!(self, Self::Autopilot)
  }
}

/// The mode of a step whose profile and project keep their defaults: Manual, by the profile.
impl Default for EffectiveMode {
  fn default() -> Self {
    Self::of(ApprovalMode::Manual, AllowAutopilot::OFF, false)
  }
}
