//! `approval-modes`: one task per run approval mode the task page's status line names (`docs/design/approval-modes.md`,
//! 04 `H6kPd`, 04b `p6tXIO`, 04c `vZcoN`), each a Feature delivery run at work on Implement, in a project that allows
//! Autopilot: one whose agent started in Autopilot (the *Autopilot* chip with *Switch to Manual for this run*); one
//! the person switched to Manual for itself (*Manual · this run*, and the Conversation's row *Switched to Manual for
//! this run* by you, Builder's profile staying on Autopilot); and one whose agent's profile is Manual (*Manual ·
//! profile*). `approval-modes-project`: the Autopilot run alone, in a project that does not allow Autopilot (*Manual ·
//! project*).
//!
//! Allowing Autopilot and setting a profile to Autopilot are a person's alone in the app, so the seed records them.
//! Its runs are recorded, not executed, as the `cards` scenario's are: a runner that attaches ends them, so start the
//! QA copy without one; without a runner they read interrupted and keep their chip, as they have not ended.

use orchester_core::{
  permission::{
    AgentPermissions, ApprovalMode, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands,
    RepositoryAccess,
  },
  run::{AllowAutopilot, Answerer, Invocation, ManualSwitch},
  store::ActivityStore,
  time::Timestamp,
};

use super::cards::at_work;
use crate::kontur::{Kontur, SeedError};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];

/// The mode a task's run is left in.
#[derive(Clone, Copy)]
enum Mode {
  /// Its agent started in Autopilot.
  Autopilot,
  /// Its agent started in Autopilot, then the person switched the run to Manual for itself.
  SwitchedByYou,
  /// Its agent's profile is Manual.
  Profile,
}

/// A task's title, the mode its run is left in, and the state it is seeded as.
type Seeded = (&'static str, Mode, &'static str);

const TASKS: [Seeded; 3] = [
  ("Retry failed webhook deliveries", Mode::Autopilot, "run in Autopilot"),
  (
    "Paginate the contacts export",
    Mode::SwitchedByYou,
    "run switched to Manual by you",
  ),
  ("Trim the audit log", Mode::Profile, "run in Manual by its profile"),
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.db.save_allow_autopilot(AllowAutopilot::ON)?;
  runs(kontur, &TASKS)
}

/// The Autopilot run in a project that does not allow Autopilot: *Manual · project*.
pub fn project(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.db.save_allow_autopilot(AllowAutopilot::OFF)?;
  runs(
    kontur,
    &[(
      "Retry failed webhook deliveries",
      Mode::Autopilot,
      "run in Manual by the project",
    )],
  )
}

fn runs(kontur: &mut Kontur, tasks: &[Seeded]) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for (index, &(title, mode, state)) in tasks.iter().enumerate() {
    let minutes = i64::try_from(tasks.len() - index).expect("a short list") * 30;
    let (created, started) = (kontur.ago(minutes + 20), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), created)?;
    let (run, invocation) = at_work(kontur, &mut task, pipeline, started)?;
    kontur.seeded(&task, state);
    if matches!(mode, Mode::Profile) {
      continue;
    }
    launched_in_autopilot(kontur, &invocation)?;
    if matches!(mode, Mode::SwitchedByYou) {
      // Five minutes into the agent's turn.
      let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 5 * 60_000);
      kontur.db.switch_run_to_manual(
        run.id(),
        ManualSwitch {
          at,
          by: Answerer::Person,
        },
      )?;
    }
  }
  Ok(())
}

/// `invocation`'s agent started with a profile set to Autopilot.
fn launched_in_autopilot(kontur: &mut Kontur, invocation: &Invocation) -> Result<(), SeedError> {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  permissions.set_approval_mode(ApprovalMode::Autopilot);
  kontur.db.save_invocation_permissions(
    invocation.id,
    &LaunchPermissions::new(permissions, OutsideCommands::PermissionPrompt),
  )?;
  Ok(())
}
