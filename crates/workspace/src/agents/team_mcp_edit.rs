//! The edit of a team MCP server in the hub's panel (`Agents · MCP server · admin · draft` A13 `WMtxm`): what its form
//! starts from (the version this computer uses and the draft kept here), and what the form's fields save as the
//! resource's draft, from the version it is edited from. Publishing the draft is
//! [`crate::sharing::Servers::publish_mcp_edit`]. The name is the resource's own and is not edited here: renaming never
//! changes the id, and is its own change.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, TeamMcpServer, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::team_mcp::{McpRuns, runs};
use crate::{
  app_mcp::setup::{Header, Invalid, NewTeamMcpServer, Runs, team_mcp_names, team_mcp_server},
  messages::{Message, store_error},
};

/// What an edit starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpEdit {
  /// The version it is edited from.
  pub number: VersionNumber,
  /// The version this computer uses, as the form holds it.
  pub published: NewTeamMcpServer,
  /// The draft kept here, if there is one.
  pub draft: Option<McpDraft>,
}

/// A draft kept here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpDraft {
  pub fields: NewTeamMcpServer,
  pub saved_at: Timestamp,
}

/// How a save of the form's fields ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
  /// The draft was saved, as of this time.
  Draft(Timestamp),
  /// The fields are the version's own, so no draft is kept.
  Unchanged,
}

/// Why the form's fields were not saved or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
  /// The database refused, as the words say.
  Store(Message),
  /// The fields do not make a team MCP server; nothing was saved.
  Invalid(Box<Invalid>),
}

impl From<StoreError> for EditError {
  fn from(error: StoreError) -> Self {
    Self::Store(store_error(&error))
  }
}

/// The fields of `body`; `None` when the form cannot hold all of it.
fn fields_of(body: &TeamMcpServer) -> Option<NewTeamMcpServer> {
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

/// What the edit of team MCP server `id` starts from; `None` when the server sends more than one secret header, which
/// the CLI is not given and the form cannot hold: editing it here would drop one.
pub(super) fn of(db: &ProjectDb, id: ResourceId) -> Result<Option<McpEdit>, StoreError> {
  let latest = db
    .latest_version::<TeamMcpServer>(id)?
    .ok_or(StoreError::NotPublished(id))?;
  let Some(published) = fields_of(latest.body()) else {
    return Ok(None);
  };
  // A draft is read here even after the person stopped being an admin: it stays, to copy or discard. One that says
  // what the version says is no work of anyone's (a resource made here keeps the one it was made from).
  let draft = match db.draft::<TeamMcpServer>(id)? {
    Some(draft) => match fields_of(draft.body()) {
      Some(fields) if fields == published => None,
      Some(fields) => Some(McpDraft {
        fields,
        saved_at: draft.updated_at(),
      }),
      None => return Ok(None),
    },
    None => None,
  };
  Ok(Some(McpEdit {
    number: latest.number(),
    published,
    draft,
  }))
}

/// Keeps `fields` as the draft of team MCP server `id` over the version it is edited from; the draft goes when they are
/// that version's own. Refused unless the person may change the team's resources.
pub fn save(root: &Path, id: ResourceId, fields: &NewTeamMcpServer) -> Result<Saved, EditError> {
  let mut db = ProjectDb::open(root)?;
  let latest = db
    .latest_version::<TeamMcpServer>(id)?
    .ok_or(StoreError::NotPublished(id))?;
  let taken: Vec<_> = team_mcp_names(root)?
    .into_iter()
    .filter(|name| name.as_str() != fields.name)
    .collect();
  let body = team_mcp_server(fields, &taken).map_err(EditError::Invalid)?;
  if &body == latest.body() {
    discard_in(&mut db, id)?;
    return Ok(Saved::Unchanged);
  }
  // The same draft again keeps the time it was saved at.
  if let Some(kept) = db.draft::<TeamMcpServer>(id)?
    && kept.body() == &body
    && kept.based_on() == Some(latest.number())
  {
    return Ok(Saved::Draft(kept.updated_at()));
  }
  let now = Timestamp::now();
  db.update_draft::<TeamMcpServer>(id, |_, _| Ok(Draft::new(body.clone(), Some(latest.number()), now)))?;
  Ok(Saved::Draft(now))
}

/// The fields of the latest version of team MCP server `id`, which the form shows again once its draft went; `None` for
/// a server the form cannot hold ([`of`]).
pub fn published(root: &Path, id: ResourceId) -> Result<Option<NewTeamMcpServer>, EditError> {
  let db = ProjectDb::open(root)?;
  Ok(of(&db, id)?.map(|edit| edit.published))
}

/// Throws away the draft of team MCP server `id`, so its latest version is the resource again.
pub fn discard(root: &Path, id: ResourceId) -> Result<(), EditError> {
  let mut db = ProjectDb::open(root)?;
  Ok(discard_in(&mut db, id)?)
}

fn discard_in(db: &mut ProjectDb, id: ResourceId) -> Result<(), StoreError> {
  let Some(draft) = db.draft::<TeamMcpServer>(id)? else {
    return Ok(());
  };
  db.discard_draft_matching::<TeamMcpServer>(id, draft.body())?;
  Ok(())
}
