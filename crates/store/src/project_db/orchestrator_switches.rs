//! `orchestrator_switches`, one row: what the Orchestrator chat may do itself in this project (Project settings ›
//! Orchestrator). Each switch is `None` until it is written for the project.

use rusqlite::params;

use super::ProjectDb;
use crate::StoreError;

/// A switch over what the Orchestrator chat does itself in a project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorSwitch {
  /// *Let the Orchestrator publish drafts*.
  PublishDrafts,
  /// *Let the Orchestrator answer agents*.
  AnswerAgents,
  /// *Let the Orchestrator merge runs*.
  MergeRuns,
  /// *Let the Orchestrator merge finished runs on its own*.
  MergeFinished,
  /// *Let the Orchestrator continue interrupted runs*.
  ContinueInterrupted,
  /// *Let the Orchestrator write with git*.
  WriteGit,
}

impl OrchestratorSwitch {
  pub const ALL: [Self; 6] = [
    Self::PublishDrafts,
    Self::AnswerAgents,
    Self::MergeRuns,
    Self::MergeFinished,
    Self::ContinueInterrupted,
    Self::WriteGit,
  ];

  const fn column(self) -> &'static str {
    match self {
      Self::PublishDrafts => "publishes_drafts",
      Self::AnswerAgents => "answers_agents",
      Self::MergeRuns => "merges_runs",
      Self::MergeFinished => "merges_finished",
      Self::ContinueInterrupted => "continues_interrupted",
      Self::WriteGit => "writes_git",
    }
  }
}

impl ProjectDb {
  /// Whether the project lets the Orchestrator do what `switch` names itself; `None` until it is written for the
  /// project.
  pub fn orchestrator_switch(&self, switch: OrchestratorSwitch) -> Result<Option<bool>, StoreError> {
    Ok(self.connection.query_row(
      &format!("SELECT {} FROM orchestrator_switches WHERE id = 1", switch.column()),
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_orchestrator_switch(&mut self, switch: OrchestratorSwitch, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      &format!("UPDATE orchestrator_switches SET {} = ?1 WHERE id = 1", switch.column()),
      params![allowed],
    )?;
    Ok(())
  }

  /// The project's `switch`; one never written for the project first takes the application-wide value it moved from
  /// (ORC-594), once: `application_wide` is read only then, and a project's own value always stays.
  pub fn copy_orchestrator_switch(
    &mut self,
    switch: OrchestratorSwitch,
    application_wide: impl FnOnce() -> Result<bool, StoreError>,
  ) -> Result<bool, StoreError> {
    if let Some(allowed) = self.orchestrator_switch(switch)? {
      return Ok(allowed);
    }
    let column = switch.column();
    self.connection.execute(
      &format!("UPDATE orchestrator_switches SET {column} = ?1 WHERE id = 1 AND {column} IS NULL"),
      params![application_wide()?],
    )?;
    // Another copy or a person's save may have come first: what the project holds now is its value.
    Ok(self.orchestrator_switch(switch)?.unwrap_or(false))
  }
}
