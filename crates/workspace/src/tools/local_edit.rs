//! Editing a local command tool in its panel in the Agents hub (`Agents · v2` A29 `FrraS`, as a local project has it;
//! `docs/design/team-configuration.md` rule 11): the panel holds what is typed and stores nothing until *Save*, which
//! publishes it as the tool's next version at once. The panel has no drafts (decided by the person 2026-11-09): it
//! starts from the latest published version, or from the draft of a tool that has only a draft so far, and a draft
//! stored meanwhile (an MCP client's) is replaced by what it publishes.
//!
//! The rules are the command tool editor's ([`super::editor`]): the program is fixed outside the repository, and a
//! change that widens what agents may run asks first ([`Saved::Confirm`]), which only a person answers.

use std::path::Path;

use orchester_core::{id::ResourceId, name::Name, resource::Tool, store::ResourceStore};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{ProjectTools, ToolEntry},
  editor::{self, Edit, EditorError},
  widening::changes,
};
use crate::{
  connections::mcp::Allowed,
  messages::{self, Message},
};

/// What a save did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
  /// The tool's latest published version now: the one it published, or the one it had when nothing differed.
  Published(Tool),
  /// It widens what agents may run beyond what the person allowed: these are every reason it widens, which a person
  /// allows before it is saved again. Nothing is stored.
  Confirm(Vec<Message>),
}

/// Why a save stored nothing.
#[derive(Debug)]
pub enum SaveError {
  /// The tool is no longer one of the project's.
  Gone,
  /// The version the panel started from is no longer the tool's latest: another window or an MCP client published one.
  Changed,
  /// The command editor refuses it: a program that cannot be fixed, a tool that is no command.
  Refused(Message),
  Store(StoreError),
}

impl SaveError {
  /// What the panel says of it.
  pub fn message(&self) -> Message {
    match self {
      Self::Gone => Message::new("agents.localConnection.gone"),
      Self::Changed => Message::new("errors.draftChanged"),
      Self::Refused(message) => message.clone(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

impl From<StoreError> for SaveError {
  fn from(error: StoreError) -> Self {
    match error {
      StoreError::DraftChanged(_) => Self::Changed,
      StoreError::ResourceNotFound(_) => Self::Gone,
      other => Self::Store(other),
    }
  }
}

impl From<EditorError> for SaveError {
  fn from(error: EditorError) -> Self {
    match error {
      EditorError::Store(error) => error.into(),
      EditorError::Message(message) => Self::Refused(message),
    }
  }
}

/// Publishes `tool` as the next version of the local tool `id`, which the panel started from `base` (its latest
/// published version, or its draft while it has only one). `allowed` is what the person already allowed it to widen
/// over the latest published version: [`Saved::Confirm`] lists what it widens when that is not all of it, and nothing
/// is stored. All or nothing: the draft the save makes is thrown away when the publication is refused, unless the tool
/// had one before, which it replaces.
pub fn save_and_publish_in(
  root: &Path,
  id: ResourceId,
  (base, tool): (&Tool, &Tool),
  allowed: &[Message],
) -> Result<Saved, SaveError> {
  let mut db = ProjectDb::open(root)?;
  let before = entry(&db, id)?;
  let latest = db.latest_version::<Tool>(id)?.map(|version| version.body().clone());
  if latest.as_ref().unwrap_or(&before.tool) != base {
    return Err(SaveError::Changed);
  }
  let fixed = editor::fixed_tool(&db, root, tool)?;
  // Nothing to publish: a draft stored meanwhile is left as it is, as *Cancel* leaves it.
  if latest.as_ref() == Some(&fixed) {
    return Ok(Saved::Published(base.clone()));
  }
  let reasons = changes(latest.as_ref(), &fixed, "appMcp.projectTools.widens.firstVersion");
  if !reasons.iter().all(|reason| allowed.contains(reason)) {
    return Ok(Saved::Confirm(reasons));
  }
  let name = Name::new(&before.name).map_err(|_| SaveError::Gone)?;
  let save = Edit::Save {
    name,
    tool: Box::new(fixed.clone()),
    before: Some(before.clone()),
  };
  // The draft reaches no agent; what is published is checked against the latest version, under the store's lock.
  editor::apply(&mut db, root, save, &Allowed::Anything)?;
  let drafted = entry(&db, id)?;
  let published = editor::apply(&mut db, root, Edit::Publish(drafted), &Allowed::These(allowed.to_vec()));
  if let Err(error) = published {
    if !before.state.has_draft() {
      // The refusal is what the person is told; a draft that could not be thrown away is listed, and its panel shows
      // the version it publishes from.
      if let Err(error) = db.discard_draft_matching(id, &fixed) {
        tracing::error!(%error, "cannot discard the draft of a tool whose publication was refused");
      }
    }
    return Err(error.into());
  }
  Ok(Saved::Published(fixed))
}

fn entry(db: &ProjectDb, id: ResourceId) -> Result<ToolEntry, SaveError> {
  ProjectTools::load(db)?.tool(id).cloned().ok_or(SaveError::Gone)
}
