//! The edit of a team MCP server in the hub's panel (`docs/design/team-configuration-live.md`, rule 6): what its form
//! starts from (the latest version read this session) and Save, which publishes the form's fields as the next version
//! on the team server at once. No draft is kept: Cancel puts back the version's own fields. The name is the resource's
//! own and is not edited here: renaming never changes the id, and is its own change.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{TeamMcpServer, VersionNumber},
};

use super::team_mcp::{McpRuns, runs};
use crate::{
  app_mcp::setup::{Header, Invalid, NewTeamMcpServer, Runs, team_mcp_names, team_mcp_server},
  sharing::{
    Servers,
    team_edit::{EditRefused, Publication},
  },
  team_config::TeamConfig,
};

/// What an edit starts from: the latest version, as the form holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpEdit {
  pub number: VersionNumber,
  pub published: NewTeamMcpServer,
}

/// Why the form's fields were not published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
  /// The fields do not make a team MCP server; nothing was sent.
  Invalid(Box<Invalid>),
  /// The team server did not take them, or could not be asked.
  Refused(EditRefused),
}

/// The fields of `body`; `None` when the form cannot hold all of it.
pub(super) fn fields_of(body: &TeamMcpServer) -> Option<NewTeamMcpServer> {
  let runs = match runs(body.launch()) {
    McpRuns::Program {
      program,
      args,
      folder,
      variables,
    } => Runs::Program {
      program,
      arguments: args,
      folder: folder.unwrap_or_default(),
      variables: variables.iter().map(ToString::to_string).collect(),
    },
    McpRuns::Remote { url, headers } => {
      let mut headers = headers.into_iter();
      let header = headers.next().map(|header| Header {
        name: header.name,
        secret: header.secret.to_string(),
        prefix: header.prefix.unwrap_or_default(),
      });
      if headers.next().is_some() {
        return None;
      }
      Runs::Remote { url, header }
    }
  };
  Some(NewTeamMcpServer {
    name: body.name().as_str().to_owned(),
    runs,
    exempt_tools: body
      .exempt_tools()
      .tools()
      .iter()
      .map(|tool| tool.as_str().to_owned())
      .collect(),
  })
}

/// What an edit of a team MCP server starts from, its latest version `number` with `body`; `None` when the server sends
/// more than one secret header, which the CLI is not given and the form cannot hold: editing it here would drop one.
pub(super) fn of_latest((number, body): (VersionNumber, &TeamMcpServer)) -> Option<McpEdit> {
  Some(McpEdit {
    number,
    published: fields_of(body)?,
  })
}

/// What an edit of team MCP server `id` starts from as `config` holds it now; `None` for a server `config` does not
/// hold, one with no version this computer reads, and one the form cannot hold ([`of_latest`]).
pub fn latest(config: &TeamConfig, id: ResourceId) -> Option<McpEdit> {
  let resource = config.live(id)?;
  let body = resource.body::<TeamMcpServer>()?.ok()?;
  of_latest((resource.number()?, &body))
}

/// Publishes `fields` as the next version of team MCP server `id` of the project at `root`, whose configuration this
/// session read is `config`, made on version `base`; the number the team server gave it.
pub async fn save(
  servers: &Servers,
  (root, config): (&Path, &TeamConfig),
  (id, base): (ResourceId, VersionNumber),
  fields: &NewTeamMcpServer,
) -> Result<VersionNumber, EditError> {
  let own = config.name(id).map(str::to_owned);
  let taken: Vec<_> = team_mcp_names(config)
    .into_iter()
    .filter(|name| Some(name.as_str()) != own.as_deref())
    .collect();
  let body = team_mcp_server(fields, &taken).map_err(EditError::Invalid)?;
  let publication = Publication {
    body: serde_json::to_value(&body).expect("a team MCP server serializes"),
    references: Vec::new(),
  };
  servers
    .publish_team(root, id, (Some(base), publication))
    .await
    .map_err(EditError::Refused)
}
