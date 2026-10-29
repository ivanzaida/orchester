//! Approval modes (`docs/design/approval-modes.md`): a run's step is in Autopilot only when its agent profile says
//! Autopilot, its project allows Autopilot (*Allow Autopilot for agents in this project*) and the run was not switched
//! to Manual; otherwise it is in Manual, for the first of those that says so. Autopilot approves its agent's requests
//! itself; Manual runs the profile's allowed commands, then asks the reviewer, then the person. The project's
//! *Approved for every run* rules answer only in Manual.
//!
//! Allowing Autopilot widens what agents do without a person, so only a person turns it on; turning it off is anyone's.
//! A run drops to Manual for itself, from the person or automation, and never goes back to Autopilot.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::Answerer;
use crate::permission::ApprovalMode;

/// The project's *Allow Autopilot for agents in this project*; off until a person turns it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AllowAutopilot(bool);

/// Someone other than a person asked to allow Autopilot.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("only a person allows Autopilot in a project")]
pub struct AutopilotPersonOnly;

impl AllowAutopilot {
  pub const OFF: Self = Self(false);
  pub const ON: Self = Self(true);

  pub const fn is_on(self) -> bool {
    self.0
  }

  /// The setting once `by` asked for it `on`: turning it off is always taken, and asking for what it is already
  /// changes nothing; turning it on is a person's alone.
  pub const fn changed(self, on: bool, by: Answerer) -> Result<Self, AutopilotPersonOnly> {
    match (on, by) {
      (false, _) => Ok(Self::OFF),
      (true, _) if self.0 => Ok(self),
      (true, Answerer::Person) => Ok(Self::ON),
      (true, Answerer::Automation) => Err(AutopilotPersonOnly),
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
