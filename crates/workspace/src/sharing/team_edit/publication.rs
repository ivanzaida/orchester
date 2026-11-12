//! A change's body in the team's form (`docs/design/team-configuration.md`), made on this computer before it is sent:
//! an agent profile's connection becomes the CLI kind it runs and its MCP servers the team's, and a body of a kind not
//! in the team's form yet must name no connection, MCP server or secret of this computer. What the team cannot take is
//! refused here, and nothing is sent.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, TeamTool, Tool},
  sharing::LocalEntities,
  store::{ConnectionStore, McpServerStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};
use serde::Serialize;

use super::{EditRefused, Local, Publication, Refusal};
use crate::{
  messages,
  sharing::{SharingError, sync::team_body},
  team_config::TeamConfig,
};

impl Publication {
  /// `body` of the resource `resource` of the project at `root`, of a kind whose body is not in the team's form yet, as
  /// it is: it must name nothing of this computer, and sends no references. Blocking: it reads the project's database.
  pub fn of_body(root: &Path, resource: ResourceId, body: &impl Serialize) -> Result<Self, EditRefused> {
    let db = ProjectDb::open(root).map_err(|error| store_failure(&error))?;
    let value = serde_json::to_value(body).map_err(|error| {
      store_failure(&StoreError::Corrupt {
        what: "resource body",
        detail: error.to_string(),
      })
    })?;
    let (connections, mcp_servers, secrets) = (
      db.connections().map_err(|error| store_failure(&error))?,
      db.mcp_servers().map_err(|error| store_failure(&error))?,
      db.secrets().map_err(|error| store_failure(&error))?,
    );
    let entities = LocalEntities {
      connections: &connections,
      mcp_servers: &mcp_servers,
      secrets: &secrets,
    };
    team_body::unchanged(resource, value, entities).map_err(refused)
  }

  /// `profile`, the agent profile `resource` of the project at `root`, in the team's form: its connection becomes the
  /// CLI kind it runs, and it may choose the team MCP servers of `config`. Blocking: it reads the project's database.
  pub fn of_profile(
    root: &Path,
    resource: ResourceId,
    profile: &AgentProfile,
    config: &TeamConfig,
  ) -> Result<Self, EditRefused> {
    let db = ProjectDb::open(root).map_err(|error| store_failure(&error))?;
    team_body::profile(&db, resource, profile, config).map_err(refused)
  }

  /// `tool`, the tool `resource` of the project at `root`, in the team's form: the secret it sends or connects with
  /// goes by the name it has in this project. Blocking: it reads the project's database.
  pub fn of_tool(root: &Path, resource: ResourceId, tool: &Tool) -> Result<Self, EditRefused> {
    let db = ProjectDb::open(root).map_err(|error| store_failure(&error))?;
    let secrets = team_body::named_secrets(&db).map_err(|error| store_failure(&error))?;
    team_body::tool(resource, tool, &secrets).map_err(refused)
  }

  /// `tool`, a team tool in the team's form, which names its secret by name and nothing of this computer: sent as it
  /// is.
  pub fn of_team_tool(tool: &TeamTool) -> Result<Self, EditRefused> {
    team_body::team_tool(tool).map_err(|error| store_failure(&error))
  }
}

/// What this computer found the team cannot take.
fn refused(error: SharingError) -> EditRefused {
  let local = match error {
    SharingError::LocalEntitiesNamed { .. } => Local::Entities,
    SharingError::PipelineNamesNonTeam { .. } => Local::NamesNonTeam,
    SharingError::WritableFoldersStay { .. } => Local::WritableFolders,
    SharingError::Store(error) => return store_failure(&error),
    error => return EditRefused::Failed(crate::sharing::team::said(&error, None)),
  };
  EditRefused::Refused(Refusal::Local(local))
}

fn store_failure(error: &StoreError) -> EditRefused {
  EditRefused::Failed(messages::store_error(error))
}
