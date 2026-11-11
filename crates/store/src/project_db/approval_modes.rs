//! Approval modes (`orchester_core::run::EffectiveMode`): the project's *Approve mode*
//! (`orchester_core::run::ApproveMode`) and each run's switch to Manual, which the runner reads at each request with
//! its step's profile mode, with when and by whom it was switched for its Conversation.

use orchester_core::{
  id::RunId,
  permission::ApprovalMode,
  run::{AllowAutopilot, ApproveMode, EffectiveMode, ManualSwitch},
  store::ActivityStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  questions::{answerer, read_answerer},
};
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// The project's *Approve mode*; Manual until a person moves it. The runner reads it again at each request.
  pub fn approve_mode(&self) -> Result<ApproveMode, StoreError> {
    let stored: Option<(bool, bool)> = self
      .connection
      .query_row(
        "SELECT allow_autopilot, dangerous_auto_approve FROM project_approvals WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    Ok(match stored {
      Some((_, true)) => ApproveMode::DangerousAutoApprove,
      Some((true, false)) => ApproveMode::Autopilot,
      Some((false, false)) | None => ApproveMode::Manual,
    })
  }

  pub fn save_approve_mode(&mut self, mode: ApproveMode) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_approvals (id, allow_autopilot, dangerous_auto_approve) VALUES (1, ?1, ?2) \
       ON CONFLICT (id) DO UPDATE SET allow_autopilot = excluded.allow_autopilot, \
       dangerous_auto_approve = excluded.dangerous_auto_approve",
      [mode.allows_autopilot().is_on(), mode.approves_everything()],
    )?;
    Ok(())
  }

  /// Whether the project allows Autopilot: in its Autopilot and Dangerous auto-approve modes.
  pub fn allow_autopilot(&self) -> Result<AllowAutopilot, StoreError> {
    Ok(self.approve_mode()?.allows_autopilot())
  }

  /// The former *Allow Autopilot* switch's write ([`ApproveMode::with_autopilot`]): off is Manual, on Autopilot, and
  /// on leaves Dangerous auto-approve as it is.
  pub fn save_allow_autopilot(&mut self, setting: AllowAutopilot) -> Result<(), StoreError> {
    let mode = self.approve_mode()?.with_autopilot(setting.is_on());
    self.save_approve_mode(mode)
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

  /// Switches `run` to Manual for itself, for good: nothing switches a run back. The first switch is the one
  /// recorded; a later one changes nothing.
  pub fn switch_run_to_manual(&mut self, run: RunId, switch: ManualSwitch) -> Result<(), StoreError> {
    // SQLite reads every expression of the SET from the row as it was, so `manual = 0` is before this switch.
    let changed = self.connection.execute(
      "UPDATE runs SET manual = 1, \
       manual_at = CASE WHEN manual = 0 THEN ?2 ELSE manual_at END, \
       manual_by = CASE WHEN manual = 0 THEN ?3 ELSE manual_by END \
       WHERE id = ?1",
      params![run.to_string(), switch.at.unix_millis(), answerer(switch.by)],
    )?;
    if changed == 0 {
      return Err(corrupt("run", format!("{run} is not stored")));
    }
    Ok(())
  }

  /// When `run` was switched to Manual for itself and by whom; `None` while it was not, or when it was switched
  /// before that was recorded.
  pub fn run_manual_switch(&self, run: RunId) -> Result<Option<ManualSwitch>, StoreError> {
    let row: Option<(Option<i64>, Option<String>)> = self
      .connection
      .query_row(
        "SELECT manual_at, manual_by FROM runs WHERE id = ?1 AND manual = 1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((Some(at), by)) = row else {
      return Ok(None);
    };
    Ok(Some(ManualSwitch {
      at: Timestamp::from_unix_millis(at),
      by: read_answerer("manual switch author", by.as_deref())?,
    }))
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
