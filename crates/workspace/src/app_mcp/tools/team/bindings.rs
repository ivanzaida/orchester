//! Which local connection each agent profile of a team task's pipeline runs on, for its start's choices, as the
//! caller binds them: chosen by the team work's rule ([`crate::team_work::bind`]), the same as the Task page's Run.

use orchester_core::{connection::Connection, id::ResourceId, run::inputs::LocalBindings};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

use super::read_refused;
use crate::{
  app_mcp::error::{Missing, ToolError},
  messages::Message,
  sharing::sync::SystemClock,
  team_reads::TeamProject,
  team_work::{ChoiceError, bind as bind_choices},
};

/// The local connection a team profile runs on, chosen by the caller.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Binding {
  /// The team agent profile's id.
  pub profile: String,
  /// A local agent connection's id, or its name.
  pub connection: String,
}

/// The bindings of every agent profile of `pipeline`'s latest published version, on `connections`, read while the
/// project's subscription gives the reads their position, as the tools' other reads are (`super::read`).
pub(super) async fn bind(
  project: &TeamProject,
  pipeline: ResourceId,
  connections: &[Connection],
  given: &[Binding],
) -> Result<LocalBindings, ToolError> {
  let given: Vec<(String, String)> = given
    .iter()
    .map(|binding| (binding.profile.clone(), binding.connection.clone()))
    .collect();
  tokio::select! {
    ended = project.follow(&SystemClock) => Err(read_refused(&ended)),
    bound = bind_choices(project, pipeline, connections, &given) => bound.map_err(|error| choice_refused(&error)),
  }
}

/// Why a start's choices could not be made, as the tool refuses it.
pub(super) fn choice_refused(error: &ChoiceError) -> ToolError {
  match error {
    ChoiceError::BranchNotFound(branch) => ToolError::refused(
      "branch_not_found",
      vec![Message::new("appMcp.errors.branchNotFound").with("branch", branch.clone())],
    ),
    ChoiceError::Read(error) => read_refused(error),
    ChoiceError::NotPublished => ToolError::refused(
      "pipeline_not_published",
      vec![Message::new("appMcp.errors.teamNotPublished")],
    ),
    ChoiceError::Unreadable => {
      ToolError::refused("team_read_failed", vec![Message::new("appMcp.errors.teamReadFailed")])
    }
    ChoiceError::ConnectionNotFound(named) => ToolError::not_found(Missing::Connection, named.clone()),
    ChoiceError::BindingsRequired(profiles) => ToolError::refused(
      "bindings_required",
      vec![Message::new("appMcp.errors.bindingsRequired").with("profiles", profiles.join(", "))],
    ),
  }
}
