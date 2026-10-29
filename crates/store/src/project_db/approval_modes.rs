//! Approval modes (`orchester_core::run::EffectiveMode`): the project's *Allow Autopilot for agents in this project*
//! and each run's switch to Manual, which the runner reads at each request with its step's profile mode.

use orchester_core::{
  id::RunId,
  permission::ApprovalMode,
  run::{AllowAutopilot, EffectiveMode},
  store::ActivityStore,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// *Allow Autopilot for agents in this project*; off until a person turns it on.
  pub fn allow_autopilot(&self) -> Result<AllowAutopilot, StoreError> {
    let on: Option<bool> = self
      .connection
      .query_row(
        "SELECT allow_autopilot FROM project_approvals WHERE id = 1",
        [],
        |row| row.get(0),
      )
      .optional()?;
    Ok(if on.unwrap_or(false) {
      AllowAutopilot::ON
    } else {
      AllowAutopilot::OFF
    })
  }

  pub fn save_allow_autopilot(&mut self, setting: AllowAutopilot) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_approvals (id, allow_autopilot) VALUES (1, ?1) \
       ON CONFLICT (id) DO UPDATE SET allow_autopilot = excluded.allow_autopilot",
      [setting.is_on()],
    )?;
    Ok(())
  }

  /// Whether `run` was switched to Manual for itself.
  pub fn run_switched_to_manual(&self, run: RunId) -> Result<bool, StoreError> {
    self
      .connection
      .query_row("SELECT manual FROM runs WHERE id = ?1", [run.to_string()], |row| {
        row.get(0)
      })
      .optional()?
      .ok_or_else(|| corrupt("run", format!("{run} is not stored")))
  }

  /// Switches `run` to Manual for itself, for good: nothing switches a run back.
  pub fn switch_run_to_manual(&mut self, run: RunId) -> Result<(), StoreError> {
    let changed = self
      .connection
      .execute("UPDATE runs SET manual = 1 WHERE id = ?1", params![run.to_string()])?;
    if changed == 0 {
      return Err(corrupt("run", format!("{run} is not stored")));
    }
    Ok(())
  }

  /// The mode a step of `run` whose profile asks for `profile` answers its agent's requests in now.
  pub fn approval_mode(&self, run: RunId, profile: ApprovalMode) -> Result<EffectiveMode, StoreError> {
    Ok(EffectiveMode::of(
      profile,
      self.allow_autopilot()?,
      self.run_switched_to_manual(run)?,
    ))
  }

  /// The mode `run` answers its agents' requests in now, by the profile its latest agent started with; Manual, by the
  /// profile, before any agent started.
  pub fn run_approval_mode(&self, run: RunId) -> Result<EffectiveMode, StoreError> {
    let profile = self
      .invocations(run)?
      .iter()
      .rev()
      .find_map(|invocation| invocation.permissions.as_ref())
      .map_or(ApprovalMode::Manual, |launch| launch.permissions().approval_mode());
    self.approval_mode(run, profile)
  }
}
