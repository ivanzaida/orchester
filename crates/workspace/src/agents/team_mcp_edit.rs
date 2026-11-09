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

use super::{
  team_mcp::{McpRuns, runs},
  team_mcp_conflict::{self, Newer},
};
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
  /// The number of that version: what a draft is edited from while a version of the person's own, which the team
  /// server has not numbered, is the latest here. `None` when `team` is.
  pub team_number: Option<VersionNumber>,
  /// The draft kept here, if there is one.
  pub draft: Option<McpDraft>,
  /// What the team published since the version the draft, or the version of the person's own that met it, was edited
  /// from; `None` while the team has nothing newer.
  pub newer: Option<Newer>,
}

/// A draft kept here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpDraft {
  pub fields: NewTeamMcpServer,
  /// The version it is edited from.
  pub based_on: VersionNumber,
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
        based_on: draft.based_on().unwrap_or_else(|| latest.number()),
        saved_at: draft.updated_at(),
      }),
      None => return Ok(None),
    },
    None => None,
  };
  let team = team_version(db, id)?;
  let newer = team_mcp_conflict::newer(db, id, team.as_ref(), draft.as_ref())?;
  let (team_number, team) = team.unzip();
  Ok(Some(McpEdit {
    number: latest.number(),
    published,
    team,
    team_number,
    draft,
    newer,
  }))
}

/// What an edit of a team MCP server starts from when only its latest version, `number` with `body`, is known here:
/// no draft and nothing newer; `None` for a server the form cannot hold ([`of`]).
pub(super) fn of_latest((number, body): (VersionNumber, &TeamMcpServer)) -> Option<McpEdit> {
  let published = fields_of(body)?;
  Some(McpEdit {
    number,
    team: Some(published.clone()),
    team_number: Some(number),
    published,
    draft: None,
    newer: None,
  })
}

/// The latest version of `id` that the team server numbered, with its fields; `None` when there is none, or the form
/// cannot hold it.
fn team_version(db: &ProjectDb, id: ResourceId) -> Result<Option<(VersionNumber, NewTeamMcpServer)>, StoreError> {
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
      .and_then(|version| fields_of(version.body()).map(|fields| (version.number(), fields))),
  )
}

/// Keeps `fields` as the draft of team MCP server `id` over `base`, the version the form is edited from; the draft goes
/// when they are that version's own, or the latest's. `base` is older than the latest when the team published since
/// the form was opened: the draft then says so, which is what the merge needs to tell who changed what. Refused unless
/// the person may change the team's resources. One store transaction reads the draft and the version and writes the
/// answer, so a save that races another never undoes what the other wrote. A `base` above the latest version can only
/// be the number of a version of the person's own that was taken back since the form was opened: the draft is edited
/// from the latest then.
pub fn save(root: &Path, id: ResourceId, fields: &NewTeamMcpServer, base: VersionNumber) -> Result<Saved, EditError> {
  let mut db = ProjectDb::open(root)?;
  let taken: Vec<_> = team_mcp_names(root)?
    .into_iter()
    .filter(|name| name.as_str() != fields.name)
    .collect();
  let body = team_mcp_server(fields, &taken).map_err(EditError::Invalid)?;
  // Versions are immutable, so the base can be read before the transaction that decides.
  let base_body = db
    .version::<TeamMcpServer>(id, base)?
    .map(|version| version.body().clone());
  let mut saved = Saved::Unchanged;
  db.settle_draft::<TeamMcpServer>(id, |kept, latest| {
    let base = base.min(latest.number());
    let own = if base == latest.number() {
      Some(latest.body())
    } else {
      base_body.as_ref()
    };
    if &body == latest.body() || own == Some(&body) {
      return Ok(if kept.is_some() {
        DraftChange::Discard
      } else {
        DraftChange::Keep
      });
    }
    // The same draft again keeps the time it was saved at.
    if let Some(kept) = kept
      && kept.body() == &body
      && kept.based_on() == Some(base)
    {
      saved = Saved::Draft(kept.updated_at());
      return Ok(DraftChange::Keep);
    }
    let now = Timestamp::now();
    saved = Saved::Draft(now);
    Ok(DraftChange::Replace(Draft::new(body.clone(), Some(base), now)))
  })?;
  Ok(saved)
}

/// What the edit of team MCP server `id` starts from; `None` for a server the form cannot hold ([`of`]).
pub fn read(root: &Path, id: ResourceId) -> Result<Option<McpEdit>, EditError> {
  Ok(of(&ProjectDb::open(root)?, id)?)
}

/// The fields of the latest version of team MCP server `id`, which the form shows again once its draft went; `None` for
/// a server the form cannot hold ([`of`]).
pub fn published(root: &Path, id: ResourceId) -> Result<Option<NewTeamMcpServer>, EditError> {
  Ok(read(root, id)?.map(|edit| edit.published))
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
