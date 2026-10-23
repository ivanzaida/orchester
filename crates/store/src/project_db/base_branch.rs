//! The project's base branch (ORC-81, `orchester_core::integration::base`): `project_base_branch`, Project settings ›
//! General › Base branch, one row while a branch is set; without it new runs take the branch the checkout has checked
//! out. Kept on this computer only: a shared project does not sync it.

use orchester_core::{integration::base::BaseBranchSetting, run::BranchName};
use rusqlite::OptionalExtension;

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// Where the project's new runs start from.
  pub fn base_branch_setting(&self) -> Result<BaseBranchSetting, StoreError> {
    let branch: Option<String> = self
      .connection
      .query_row("SELECT branch FROM project_base_branch", [], |row| row.get(0))
      .optional()?;
    branch.map_or(Ok(BaseBranchSetting::CheckedOut), |branch| {
      BranchName::new(&branch)
        .map(BaseBranchSetting::Branch)
        .map_err(|error| corrupt("base branch setting", error))
    })
  }

  pub fn save_base_branch_setting(&mut self, setting: &BaseBranchSetting) -> Result<(), StoreError> {
    match setting {
      BaseBranchSetting::CheckedOut => self.connection.execute("DELETE FROM project_base_branch", [])?,
      BaseBranchSetting::Branch(branch) => self.connection.execute(
        "INSERT INTO project_base_branch (id, branch) VALUES (1, ?1) \
         ON CONFLICT (id) DO UPDATE SET branch = excluded.branch",
        [branch.as_str()],
      )?,
    };
    Ok(())
  }

  /// How many runs still deliver into the base branch they recorded: those that have not ended, and each task's
  /// latest run that succeeded and that Orchester has not merged. A change of the base branch leaves them on theirs.
  pub fn runs_keeping_their_base(&self) -> Result<usize, StoreError> {
    let count: i64 = self.connection.query_row(
      "SELECT COUNT(*) FROM runs AS run WHERE run.status NOT IN ('succeeded', 'failed', 'cancelled') \
       OR (run.status = 'succeeded' \
         AND NOT EXISTS (SELECT 1 FROM run_merges AS merged WHERE merged.run_id = run.id) \
         AND run.number = (SELECT MAX(other.number) FROM runs AS other WHERE other.task_id = run.task_id))",
      [],
      |row| row.get(0),
    )?;
    usize::try_from(count).map_err(|error| corrupt("run count", error))
  }
}
