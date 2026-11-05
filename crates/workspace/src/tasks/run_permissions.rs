//! A run's permissions chip and its menu (ORC-319, `15f · Permissions on a run · parts and rules` `CimC4`): what the
//! chip says of how the run answers its requests now and what the profile allows, and what each of the menu's options
//! does for the run. The chip and the menu change this run only; the agent profile and the project never change from
//! them.
//!
//! - *Manual for this run* narrows: from Autopilot it switches the run to Manual for good, and stops approving
//!   everything with it; in Manual it stops approving everything. It is current while the run is in Manual and does not
//!   approve everything.
//! - *Autopilot* is current while the run is in it and does not approve everything; while it approves everything,
//!   choosing Autopilot stops that, back to Autopilot. It is offered only while the profile says Autopilot, the project
//!   allows it and the run was not switched to Manual; otherwise it is unavailable with the first reason that applies,
//!   or left out while the run approves everything already: then nothing is wider, and the menu's foot says why.
//! - *Approve everything in this run* widens: a person's alone
//!   ([`orchester_core::run::RunPermissionChange::asked_by`]).

use orchester_core::{
  permission::ApprovalMode,
  run::{EffectiveMode, ManualBecause, RunPermissionChange},
};

use super::run_approvals::RunApprovals;

/// What the chip says of how the run answers its requests now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipMode {
  /// Manual, by the profile.
  Manual,
  Autopilot,
  /// It approves everything, whatever its mode.
  Everything,
  /// Manual, the run switched to it for itself.
  ManualRun,
  /// Manual, as the project does not allow Autopilot.
  ManualProject,
}

/// Why *Autopilot* cannot be chosen for the run, the first that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAutopilot {
  /// The run was switched to Manual, which it stays until it ends.
  Switched,
  /// The profile says Manual.
  ProfileManual,
  /// The project does not allow Autopilot.
  ProjectOff,
}

/// What one of the menu's options does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
  /// It is how the run answers now: choosing it changes nothing.
  Current,
  /// Choosing it makes this change.
  Change(RunPermissionChange),
  /// It cannot be chosen, for this reason.
  Unavailable(NotAutopilot),
  /// The menu leaves it out.
  Hidden,
}

/// The chip and its menu for one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionsMenu {
  pub chip: ChipMode,
  /// What the profile the run's latest agent started with says.
  pub profile: ApprovalMode,
  pub manual: Offer,
  pub autopilot: Offer,
  pub everything: Offer,
  /// While nothing is wider than how the run answers now: why Autopilot is not offered.
  pub nothing_wider: Option<NotAutopilot>,
}

impl PermissionsMenu {
  /// The chip and the menu of a run that keeps `approvals`.
  pub const fn of(approvals: &RunApprovals) -> Self {
    let mode = approvals.mode;
    let everything = approvals.approves_everything();
    let not_autopilot = match mode {
      EffectiveMode::Autopilot => None,
      EffectiveMode::Manual(ManualBecause::Run) => Some(NotAutopilot::Switched),
      EffectiveMode::Manual(ManualBecause::Profile) => Some(NotAutopilot::ProfileManual),
      EffectiveMode::Manual(ManualBecause::Project) => Some(NotAutopilot::ProjectOff),
    };
    let chip = match (everything, mode) {
      (true, _) => ChipMode::Everything,
      (false, EffectiveMode::Autopilot) => ChipMode::Autopilot,
      (false, EffectiveMode::Manual(ManualBecause::Profile)) => ChipMode::Manual,
      (false, EffectiveMode::Manual(ManualBecause::Run)) => ChipMode::ManualRun,
      (false, EffectiveMode::Manual(ManualBecause::Project)) => ChipMode::ManualProject,
    };
    let profile = match mode {
      EffectiveMode::Manual(ManualBecause::Profile) => ApprovalMode::Manual,
      _ => ApprovalMode::Autopilot,
    };
    let manual = match (everything, mode.is_autopilot()) {
      (_, true) => Offer::Change(RunPermissionChange::ManualForRun),
      (true, false) => Offer::Change(RunPermissionChange::StopApprovingEverything),
      (false, false) => Offer::Current,
    };
    let autopilot = match (everything, not_autopilot) {
      (false, None) => Offer::Current,
      (true, None) => Offer::Change(RunPermissionChange::StopApprovingEverything),
      (false, Some(why)) => Offer::Unavailable(why),
      (true, Some(_)) => Offer::Hidden,
    };
    let everything_offer = if everything {
      Offer::Current
    } else {
      Offer::Change(RunPermissionChange::ApproveEverything)
    };
    Self {
      chip,
      profile,
      manual,
      autopilot,
      everything: everything_offer,
      nothing_wider: if everything { not_autopilot } else { None },
    }
  }
}
