//! A team MCP server the team changed while an admin edited it (`Agents · MCP server · admin · draft` A18 `bk4i9` the
//! quiet line, A19 `DV8OI` the refusal and the comparison, rule 18 of `docs/design/team-configuration.md`): what the
//! team published since the version the draft was edited from ([`Newer`], with the field-by-field comparison of theirs
//! and mine against that base), and the rebase that carries the draft onto it, which keeps the fields only the person
//! changed, takes the team's for the rest and asks per field where both changed ([`rebase`]). Nothing is merged
//! unseen: a field both changed has no answer until the person picks one.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{Draft, TeamMcpServer, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{DraftChange, ProjectDb, PublishState, StoreError};

use super::{
  team_mcp::Published,
  team_mcp_edit::{EditError, McpDraft, fields_of},
  team_mcp_merge::{Comparison, McpField, Pick, Unresolved, compare, merge},
  team_sheets,
};
use crate::{
  app_mcp::setup::{Invalid, NewTeamMcpServer, team_mcp_names, team_mcp_server},
  messages::{Message, store_error},
};

/// What the team published since the version an edit was made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Newer {
  /// The team's latest version, *theirs*.
  pub number: VersionNumber,
  /// Who published it and when; `None` while this computer has not read it from the server.
  pub published: Option<Published>,
  /// The version the edit was made from.
  pub base_number: VersionNumber,
  /// Its fields, the base of the merge; `None` while this computer does not keep that version, when any difference is
  /// both's to settle.
  pub base: Option<NewTeamMcpServer>,
  /// The team's latest version as the form holds it.
  pub theirs: NewTeamMcpServer,
  /// The person's edit: their draft, or the version of their own that met the team's.
  pub mine: NewTeamMcpServer,
  /// The version of the person's own in the outbox that met the team's, when that is what `mine` is; the rebase takes
  /// it back as the draft first.
  pub from_outbox: Option<VersionId>,
  pub comparison: Comparison,
}

/// A version's number and fields: what a form is edited from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Based {
  pub number: VersionNumber,
  pub fields: NewTeamMcpServer,
}

/// Why a draft was not rebased.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebaseError {
  /// The database refused, as the words say.
  Store(Message),
  /// The merged fields do not make a team MCP server; nothing was changed.
  Invalid(Box<Invalid>),
  /// Fields both changed that were not picked.
  Unresolved(Vec<McpField>),
  /// The team published again, or the draft changed, since the comparison shown: it is read again first.
  Changed,
  /// The team has nothing newer than the draft's version, so there is nothing to rebase onto.
  NothingNewer,
}

impl RebaseError {
  /// Why, in words, for the one place that says it.
  pub fn message(self) -> Message {
    match self {
      Self::Store(message) => message,
      Self::Invalid(invalid) => invalid.message(),
      Self::Unresolved(_) => Message::new("sharing.edit.rebase.unresolved"),
      Self::Changed => Message::new("sharing.edit.rebase.changed"),
      Self::NothingNewer => Message::new("sharing.edit.rebase.nothingNewer"),
    }
  }
}

impl From<StoreError> for RebaseError {
  fn from(error: StoreError) -> Self {
    Self::Store(store_error(&error))
  }
}

impl From<EditError> for RebaseError {
  fn from(error: EditError) -> Self {
    match error {
      EditError::Store(message) => Self::Store(message),
      EditError::Invalid(invalid) => Self::Invalid(invalid),
    }
  }
}

/// What the team published since `draft` (or, when the first version of the resource in the outbox met a newer one,
/// since that version's base); `None` while the team has nothing newer, or the form cannot hold what it has.
/// `team` is the team's latest version and its fields.
pub(super) fn newer(
  db: &ProjectDb,
  id: ResourceId,
  team: Option<&(VersionNumber, NewTeamMcpServer)>,
  draft: Option<&McpDraft>,
) -> Result<Option<Newer>, StoreError> {
  let Some((number, theirs)) = team else {
    return Ok(None);
  };
  let first = db
    .waiting_publishes()?
    .into_iter()
    .find(|waiting| waiting.resource == id);
  let (base_number, mine, from_outbox) = match first {
    Some(waiting) => {
      let (PublishState::Conflict { .. }, Some(base)) = (&waiting.state, waiting.based_on) else {
        return Ok(None);
      };
      let Some(own) = db
        .version::<TeamMcpServer>(id, waiting.number)?
        .and_then(|own| fields_of(own.body()))
      else {
        return Ok(None);
      };
      (base, own, Some(waiting.version))
    }
    None => match draft {
      Some(draft) if draft.based_on < *number => (draft.based_on, draft.fields.clone(), None),
      _ => return Ok(None),
    },
  };
  let base = db
    .version::<TeamMcpServer>(id, base_number)?
    .and_then(|base| fields_of(base.body()));
  Ok(Some(Newer {
    number: *number,
    published: team_sheets::published::<TeamMcpServer>(db, id, *number)?,
    base_number,
    comparison: compare(base.as_ref(), theirs, &mine),
    base,
    theirs: theirs.clone(),
    mine,
    from_outbox,
  }))
}

/// The server `newer` and the person's draft make together with `picks` for the fields both changed.
pub fn merged(newer: &Newer, picks: &[(McpField, Pick)]) -> Result<NewTeamMcpServer, RebaseError> {
  merge(newer.base.as_ref(), &newer.theirs, &newer.mine, picks)
    .map_err(|Unresolved(fields)| RebaseError::Unresolved(fields))
}

/// *Rebase my draft on vN*: the draft of team MCP server `id` becomes the merge of the team's `expected` version and
/// the draft over the version the draft was edited from (the fields only the person changed, the team's for the rest,
/// `picks` for each both changed), kept as the draft edited from the team's version. A version of the person's own in
/// the outbox that met the team's (`outbox`) is taken back as the draft first, edited from its own base. Nothing
/// changes while a field both changed has no pick, the merge is not a server, or the team published again (or the draft
/// changed) since the comparison `expected` stands for. Blocking.
pub fn rebase(
  root: &Path,
  id: ResourceId,
  (outbox, expected): (Option<VersionId>, VersionNumber),
  picks: &[(McpField, Pick)],
) -> Result<Based, RebaseError> {
  let mut db = ProjectDb::open(root)?;
  let newer = of(&db, id)?;
  if newer.number != expected || newer.from_outbox != outbox {
    return Err(RebaseError::Changed);
  }
  let fields = merged(&newer, picks)?;
  let taken: Vec<_> = team_mcp_names(root)?
    .into_iter()
    .filter(|name| name.as_str() != fields.name)
    .collect();
  let body = team_mcp_server(&fields, &taken).map_err(RebaseError::Invalid)?;
  let mine = team_mcp_server(&newer.mine, &[]).map_err(RebaseError::Invalid)?;
  if let Some(version) = outbox {
    db.reopen_draft_on_its_base::<TeamMcpServer>(version, Timestamp::now())?;
  }
  let mut changed = false;
  db.settle_draft::<TeamMcpServer>(id, |kept, _| {
    if kept.map(Draft::body) != Some(&mine) {
      changed = true;
      return Ok(DraftChange::Keep);
    }
    Ok(DraftChange::Replace(Draft::new(
      body.clone(),
      Some(newer.number),
      Timestamp::now(),
    )))
  })?;
  if changed {
    return Err(RebaseError::Changed);
  }
  Ok(Based {
    number: newer.number,
    fields,
  })
}

/// What the team published since the person's edit of `id`, read from the store.
fn of(db: &ProjectDb, id: ResourceId) -> Result<Newer, RebaseError> {
  super::team_mcp_edit::of(db, id)?
    .and_then(|edit| edit.newer)
    .ok_or(RebaseError::NothingNewer)
}
