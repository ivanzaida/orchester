//! What `get_delivery` reads of a team project (`docs/plan/orchestrator-autonomy/OA-36-…`): every task and module
//! its team server holds, as core's tasks and modules, from which the delivery figures are built with this computer's
//! runs of those tasks (`crate::delivery::Delivery::read_team`).

use orchester_api::v2::TaskFilters;
use orchester_core::{id::ModuleId, module::Module, task::Task};

use super::{Team, modules, read_refused, tasks};
use crate::{
  app_mcp::{Services, error::ToolError},
  messages::Message,
  sharing::{SharingError, project::convert},
  team_reads::listed::task_of,
};

/// The project's tasks and modules as `team`'s server holds them, and the module `module` names among them.
pub(in crate::app_mcp::tools) async fn read(
  services: &Services,
  team: &Team,
  module: Option<&str>,
) -> Result<(Vec<Task>, Vec<Module>, Option<ModuleId>), ToolError> {
  let project = team.reads(services).await?;
  let named = match module {
    Some(named) => Some(modules::module_named(&project, named).await?),
    None => None,
  };
  let listed = tasks::all_tasks(&project, TaskFilters::default())
    .await
    .map_err(|error| read_refused(&error))?;
  // The pipeline a task names takes no part in its delivery figures.
  let tasks = listed
    .iter()
    .map(|read| task_of(read, None))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| unreadable(&error))?;
  let modules = modules::all_modules(&project)
    .await?
    .iter()
    .map(|summary| convert::cached_module(&summary.module.module).map(|cached| cached.module))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| unreadable(&error))?;
  Ok((tasks, modules, named))
}

/// The server answered with a task or a module this version cannot read.
fn unreadable(error: &SharingError) -> ToolError {
  tracing::warn!(%error, "a team project's delivery figures could not be read");
  ToolError::refused("team_read_failed", vec![Message::new("appMcp.errors.teamReadFailed")])
}
