//! Project settings › Limits, one row; a project that never changed them uses the defaults. The
//! environment additions of Project settings › Environment are in [`super::environment`], the
//! repository agent files setting of General in [`super::agent_files`], the code index setting in
//! [`super::code_index`].

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  code_index::CodeIndexSetting,
  limits::{AgentSlots, ProjectLimits},
  resource::CommandEnvironment,
  store::SettingsStore,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{budget_value, corrupt, duration_value, millis_column, tokens_column},
};

impl SettingsStore for ProjectDb {
  type Error = StoreError;

  fn project_limits(&self) -> Result<ProjectLimits, StoreError> {
    let row: Option<(i64, i64, i64, i64, i64)> = self
      .connection
      .query_row(
        "SELECT max_run_budget, default_run_budget, daily_budget, default_time_limit_ms, agent_slots \
         FROM project_limits",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
      )
      .optional()?;
    let Some((max, default, daily, time_limit, slots)) = row else {
      return Ok(ProjectLimits::DEFAULT);
    };
    let slots = u64::try_from(slots)
      .map_err(|error| corrupt("agent slots", error))
      .and_then(|slots| AgentSlots::new(slots).map_err(|error| corrupt("agent slots", error)))?;
    let limits = ProjectLimits::new(
      budget_value("maximum run budget", max)?,
      budget_value("default run budget", default)?,
      budget_value("daily project budget", daily)?,
      duration_value("default time limit", time_limit)?,
    )
    .map_err(|error| corrupt("project limits", error))?;
    Ok(limits.with_agent_slots(slots))
  }

  fn save_project_limits(&mut self, limits: &ProjectLimits) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_limits (id, max_run_budget, default_run_budget, daily_budget, default_time_limit_ms, \
       agent_slots) VALUES (1, ?1, ?2, ?3, ?4, ?5) ON CONFLICT (id) DO UPDATE SET \
       max_run_budget = excluded.max_run_budget, default_run_budget = excluded.default_run_budget, \
       daily_budget = excluded.daily_budget, default_time_limit_ms = excluded.default_time_limit_ms, \
       agent_slots = excluded.agent_slots",
      params![
        tokens_column("maximum run budget", limits.max_run_budget().get())?,
        tokens_column("default run budget", limits.default_run_budget().get())?,
        tokens_column("daily project budget", limits.daily_budget().get())?,
        millis_column("default time limit", limits.default_time_limit())?,
        limits.agent_slots().get().get(),
      ],
    )?;
    Ok(())
  }

  fn project_environment(&self) -> Result<CommandEnvironment, StoreError> {
    super::environment::load(&self.connection)
  }

  fn save_project_environment(&mut self, environment: &CommandEnvironment) -> Result<(), StoreError> {
    super::environment::save(&mut self.connection, environment)
  }

  fn repository_agent_files(&self) -> Result<RepositoryAgentFiles, StoreError> {
    super::agent_files::load(&self.connection)
  }

  fn save_repository_agent_files(&mut self, setting: RepositoryAgentFiles) -> Result<(), StoreError> {
    super::agent_files::save(&self.connection, setting)
  }

  fn code_index(&self) -> Result<CodeIndexSetting, StoreError> {
    super::code_index::load_setting(&self.connection)
  }

  fn save_code_index(&mut self, setting: CodeIndexSetting) -> Result<(), StoreError> {
    super::code_index::save_setting(&self.connection, setting)
  }
}
