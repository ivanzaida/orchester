//! The project's limits and today's token usage, for New task, Project settings › Limits and the
//! status bar. Blocks on SQLite; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  limits::ProjectLimits,
  store::{ActivityStore, SettingsStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectUsage {
  pub limits: ProjectLimits,
  /// Counted tokens of all runs since the day began.
  pub today: TokensUsed,
}

impl lurq::app::component::DevtoolsInspectable for ProjectUsage {}

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

/// Stores the project's limits; they apply to new tasks and runs.
pub fn save_limits_in(repo_root: &Path, limits: &ProjectLimits) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_project_limits(limits)
}
