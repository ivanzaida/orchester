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

use crate::runner::Runners;

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

/// The project's limits as stored.
pub fn limits_in(repo_root: &Path) -> Result<ProjectLimits, StoreError> {
  ProjectDb::open(repo_root)?.project_limits()
}

/// Stores the project's limits, as Project settings › Limits and Orchester's MCP server change them.
/// The budgets and the time limit apply to new tasks and runs; changed agent slots apply at once,
/// handed to the project's runner once they are stored.
pub fn save_limits(runners: &Runners, repo_root: &Path, limits: &ProjectLimits) -> Result<(), StoreError> {
  runners.store_agent_slots(repo_root, || {
    let mut db = ProjectDb::open(repo_root)?;
    let before = db.project_limits()?.agent_slots();
    db.save_project_limits(limits)?;
    Ok((limits.agent_slots() != before).then_some(limits.agent_slots()))
  })
}
