//! A message that runs a team task again (`send_message` to its run that ended): the team server's work start carries
//! no message, and the message is this computer's content anyway (conversations stay here), so it is kept here for the
//! task's next run, as a local run again keeps it, and the next run starts through the team server as `run_task`
//! starts it. The runner gives a message kept for a task's next run to that run's agents when they start, whichever
//! way the run was started. The start is checked before the message is kept ([`NextRun::checked`]), so a refusal keeps
//! nothing; a start that is refused or waits once the message is kept leaves it queued for the next run, with why.

use orchester_core::id::RunId;
use orchester_store::ProjectDb;

use super::{
  Team, find_run, of,
  run::{CheckedStart, check_start, start_checked},
};
use crate::{
  app_mcp::{
    Services,
    error::{Missing, ToolError},
    services::blocking,
  },
  tasks::recorded::server_task_of,
};

/// The next run of a team task whose latest run ended, checked as `run_task` checks a start.
pub(in crate::app_mcp::tools) struct NextRun {
  team: Team,
  checked: CheckedStart,
}

impl NextRun {
  /// The next run of the task of run `run` of project `project`, a team task's run, checked before anything is kept or
  /// asked for: refused as `run_task` refuses the task's start.
  pub(in crate::app_mcp::tools) async fn checked(
    services: &Services,
    (project, run): (&str, RunId),
  ) -> Result<Self, ToolError> {
    let team = of(services, project)
      .await?
      .ok_or_else(|| ToolError::not_found(Missing::Project, project))?;
    let root = team.open.repo_root.clone();
    let task = blocking(move || {
      let db = ProjectDb::open(&root)?;
      let task = find_run(&db, &run.to_string())?.task();
      Ok(server_task_of(&db, task)?.unwrap_or(task))
    })
    .await?;
    let checked = check_start(services, &team, &task.to_string()).await?;
    Ok(Self { team, checked })
  }

  /// Starts the run through the team server: the run started, or why it did not.
  pub(in crate::app_mcp::tools) async fn start(self, services: &Services) -> Result<RunId, ToolError> {
    start_checked(services, &self.team, self.checked).await
  }
}
