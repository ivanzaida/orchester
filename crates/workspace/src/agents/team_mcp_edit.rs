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
use orchester_store::{DraftChange, ProjectDb, StoreError};

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
  /// The version this computer uses, as the form holds it: while a version published here waits in the outbox, that
  /// one.
  pub published: NewTeamMcpServer,
  /// The latest version the team server numbered, as the form holds it: what an edit changes while a version of the
  /// person's own waits in the outbox, and what it would only revert to. `None` for a resource that has none yet.
  pub team: Option<NewTeamMcpServer>,
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
  let team = team_fields(db, id)?;
  Ok(Some(McpEdit {
    number: latest.number(),
    published,
    team,
    draft,
  }))
}

/// The fields of the latest version of `id` that the team server numbered; `None` when there is none, or the form
/// cannot hold it.
fn team_fields(db: &ProjectDb, id: ResourceId) -> Result<Option<NewTeamMcpServer>, StoreError> {
  let unsent: Vec<_> = db
    .waiting_publishes()?
    .into_iter()
    .filter(|publish| publish.resource == id)
    .map(|publish| publish.version)
    .collect();
  Ok(
    db.versions::<TeamMcpServer>(id)?
      .iter()
      .rfind(|version| !unsent.contains(&version.id()))
      .and_then(|version| fields_of(version.body())),
  )
}

/// Keeps `fields` as the draft of team MCP server `id` over the version it is edited from; the draft goes when they are
/// that version's own. Refused unless the person may change the team's resources. One store transaction reads the
/// draft and the version and writes the answer, so a save that races another never undoes what the other wrote.
pub fn save(root: &Path, id: ResourceId, fields: &NewTeamMcpServer) -> Result<Saved, EditError> {
  let mut db = ProjectDb::open(root)?;
  let taken: Vec<_> = team_mcp_names(root)?
    .into_iter()
    .filter(|name| name.as_str() != fields.name)
    .collect();
  let body = team_mcp_server(fields, &taken).map_err(EditError::Invalid)?;
  let mut saved = Saved::Unchanged;
  db.settle_draft::<TeamMcpServer>(id, |kept, latest| {
    if &body == latest.body() {
      return Ok(if kept.is_some() {
        DraftChange::Discard
      } else {
        DraftChange::Keep
      });
    }
    // The same draft again keeps the time it was saved at.
    if let Some(kept) = kept
      && kept.body() == &body
      && kept.based_on() == Some(latest.number())
    {
      saved = Saved::Draft(kept.updated_at());
      return Ok(DraftChange::Keep);
    }
    let now = Timestamp::now();
    saved = Saved::Draft(now);
    Ok(DraftChange::Replace(Draft::new(
      body.clone(),
      Some(latest.number()),
      now,
    )))
  })?;
  Ok(saved)
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
  db.settle_draft::<TeamMcpServer>(id, |kept, _| {
    Ok(if kept.is_some() {
      DraftChange::Discard
    } else {
      DraftChange::Keep
    })
  })?;
  Ok(())
}
