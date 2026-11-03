//! The project's limits and today's token usage, for New task, Project settings › Runs › Limits and the
//! status bar. Blocks on SQLite; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  limits::ProjectLimits,
  store::{ActivityStore, SettingsStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};

use crate::runner::Runners;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectUsage {
  pub limits: ProjectLimits,
  /// Counted tokens of all runs since the day began.
  pub today: TokensUsed,
}

impl ProjectUsage {
  /// Reads the limits and the usage since `day_start`, local midnight.
  pub fn load(repo_root: &Path, day_start: Timestamp) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    Ok(Self {
      limits: db.project_limits()?,
      today: db.tokens_used_since(day_start)?,
    })
  }
}

/// The project's limits as stored.
pub fn limits_in(repo_root: &Path) -> Result<ProjectLimits, StoreError> {
  ProjectDb::open(repo_root)?.project_limits()
}

/// Stores the project's limits, as Project settings › Runs › Limits and Orchester's MCP server change them.
/// The budgets and the time limit apply to new tasks and runs; changed agent slots apply at once,
/// handed to the project's runner once they are stored.
pub fn save_limits(runners: &Runners, repo_root: &Path, limits: &ProjectLimits) -> Result<(), StoreError> {
  update_limits(runners, repo_root, |_| Some(*limits)).map(|_| ())
}

/// Stores what `change` makes of the stored limits, read and written in one transaction so no other
/// writer changes them in between, as [`save_limits`] stores them. The limits saved; `None` when
/// `change` saved nothing.
pub fn update_limits(
  runners: &Runners,
  repo_root: &Path,
  change: impl FnOnce(ProjectLimits) -> Option<ProjectLimits>,
) -> Result<Option<ProjectLimits>, StoreError> {
  let mut saved = None;
  runners.store_agent_slots(repo_root, || {
    let Some((before, after)) = ProjectDb::open(repo_root)?.update_project_limits(change)? else {
      return Ok(None);
    };
    saved = Some(after);
    Ok((after.agent_slots() != before.agent_slots()).then_some(after.agent_slots()))
  })?;
  Ok(saved)
}
