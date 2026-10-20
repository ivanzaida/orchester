//! A person's changes to memory records (`docs/design/orchester-workspace.md`, "How records change"): approving a
//! proposal (with the person's edits), rejecting it, editing a record in place, pinning, deprecating, restoring,
//! deleting and writing a new one. Each change names the revision of the record the screen showed; a record changed
//! since is refused, so a stale screen never overwrites a newer decision. What a person writes is checked against
//! the values of the project's secrets before it is kept. Every function blocks on SQLite and the keychain; the UI
//! calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  id::MemoryId,
  memory::{
    ContainsSecret, Memory, MemoryChanges, MemoryError, MemoryFilter, MemoryScope, MemoryStore, ScopeFilter,
    ScreenUnavailable, ScreenedContent, SecretScreen, approval,
  },
  secret::SecretVault,
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::draft::{DraftProblem, RecordDraft};
use crate::messages::{self, Message};

/// Why a change was not kept. Nothing was stored.
#[derive(Debug)]
pub enum MemoryFailure {
  Draft(DraftProblem),
  ContainsSecret(ContainsSecret),
  Unscreened(ScreenUnavailable),
  /// The record changed after the screen read it, or a decision on it was taken meanwhile.
  ChangedMeanwhile,
  /// The record no longer exists.
  Gone,
  /// A deprecated record is not restored while another active record of its scope has its key.
  KeyInUse(String),
  Store(StoreError),
}

impl From<StoreError> for MemoryFailure {
  fn from(error: StoreError) -> Self {
    match error {
      StoreError::MemoryConflict(_) | StoreError::MemoryNotActive(_) => Self::ChangedMeanwhile,
      StoreError::MemoryNotFound(_) => Self::Gone,
      error => Self::Store(error),
    }
  }
}

impl From<MemoryError> for MemoryFailure {
  fn from(error: MemoryError) -> Self {
    match error {
      MemoryError::Unknown { .. } => Self::Gone,
      _ => Self::ChangedMeanwhile,
    }
  }
}

impl MemoryFailure {
  pub fn message(&self) -> Message {
    match self {
      Self::Draft(problem) => problem.message(),
      Self::ContainsSecret(found) => Message::new("memory.errors.secret").with("secret", found.secret.to_string()),
      Self::Unscreened(unavailable) => {
        Message::new("memory.errors.keychain").with("secret", unavailable.secret.to_string())
      }
      Self::ChangedMeanwhile => Message::new("memory.errors.changedMeanwhile"),
      Self::Gone => Message::new("memory.errors.gone"),
      Self::KeyInUse(key) => Message::new("memory.errors.restoreKeyInUse").with("key", key.clone()),
      Self::Store(StoreError::DuplicateMemoryKey(_)) => Message::new("memory.errors.keyTaken"),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// A change of one record, as the person asked for it on what the screen showed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
  /// Approve a proposal, saving the person's edits with it when they changed what it says.
  Approve(Option<RecordDraft>),
  Reject,
  /// Edit what an open record says and whether it is pinned.
  Edit {
    draft: RecordDraft,
    pinned: bool,
  },
  Pin(bool),
  Deprecate,
  Restore,
}

/// Makes `change` of record `id`, which the screen showed at `revision`, in the project at `repo_root`; `vault`
/// holds the project's secret values that edits are checked against.
pub fn change_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  (id, revision): (MemoryId, u32),
  change: Change,
) -> Result<(), MemoryFailure> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut memory = current(&db, id, revision)?;
  let now = Timestamp::now();
  match change {
    Change::Approve(draft) => {
      let edited = match draft {
        Some(draft) if draft.differs_from(&memory) => Some(screened(&db, vault, &draft)?),
        _ => None,
      };
      return approve(&mut db, &memory, edited, now);
    }
    Change::Reject => memory.reject(now)?,
    Change::Edit { draft, pinned } => {
      if draft.differs_from(&memory) {
        memory.edit(screened(&db, vault, &draft)?, now)?;
      }
      if pinned != memory.is_pinned() {
        memory.set_pinned(pinned, now)?;
      }
    }
    Change::Pin(pinned) => memory.set_pinned(pinned, now)?,
    Change::Deprecate => memory.deprecate(now)?,
    Change::Restore => {
      memory.restore(now)?;
      // A key names one active record of its scope: another holding it now keeps this one deprecated.
      return match db.save_memory(&mut memory) {
        Err(StoreError::DuplicateMemoryKey(_)) => Err(MemoryFailure::KeyInUse(
          memory
            .content()
            .key()
            .map_or_else(String::new, |key| key.as_str().to_owned()),
        )),
        saved => Ok(saved?),
      };
    }
  }
  db.save_memory(&mut memory)?;
  Ok(())
}

/// Approves `proposal`, edited to `edited` first if the person changed it, with what follows from it in its scope
/// ([`approval`]), in one transaction that reads the scope's records again.
fn approve(
  db: &mut ProjectDb,
  proposal: &Memory,
  edited: Option<ScreenedContent>,
  now: Timestamp,
) -> Result<(), MemoryFailure> {
  let (id, revision) = (proposal.id(), proposal.revision());
  let filter = MemoryFilter {
    scope: scope_filter(proposal.scope()),
    status: None,
    pinned_only: false,
  };
  db.change_memories(&filter, |mut records| -> Result<MemoryChanges, MemoryFailure> {
    let read = records
      .iter_mut()
      .find(|record| record.id() == id)
      .ok_or(MemoryFailure::Gone)?;
    if read.revision() != revision {
      return Err(MemoryFailure::ChangedMeanwhile);
    }
    if let Some(content) = edited {
      read.edit(content, now)?;
    }
    Ok(approval(id, records, now)?)
  })?
}

/// Writes a new record of `scope` saying what `draft` says, active at once as a person's record is, pinned when
/// `pinned`; its id.
pub fn create_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  scope: MemoryScope,
  (draft, pinned): (&RecordDraft, bool),
) -> Result<MemoryId, MemoryFailure> {
  let mut db = ProjectDb::open(repo_root)?;
  let now = Timestamp::now();
  let mut memory = Memory::by_person(scope, screened(&db, vault, draft)?, now);
  if pinned {
    memory.set_pinned(true, now)?;
  }
  db.add_memories(std::slice::from_ref(&memory))?;
  Ok(memory.id())
}

/// Deletes record `id`, which the screen showed at `revision`, with its history. Only a person deletes records.
pub fn delete_in(repo_root: &Path, (id, revision): (MemoryId, u32)) -> Result<(), MemoryFailure> {
  let mut db = ProjectDb::open(repo_root)?;
  current(&db, id, revision)?;
  db.delete_memory(id)?;
  Ok(())
}

/// Record `id` as stored, while it is still at `revision`.
fn current(db: &ProjectDb, id: MemoryId, revision: u32) -> Result<Memory, MemoryFailure> {
  let memory = db.memory(id)?.ok_or(MemoryFailure::Gone)?;
  if memory.revision() != revision {
    return Err(MemoryFailure::ChangedMeanwhile);
  }
  Ok(memory)
}

/// `draft`'s content, once it holds no value of the project's secrets.
fn screened(db: &ProjectDb, vault: &dyn SecretVault, draft: &RecordDraft) -> Result<ScreenedContent, MemoryFailure> {
  let content = draft.content().map_err(MemoryFailure::Draft)?;
  let secrets = db.secrets()?;
  SecretScreen::load(db.project().id(), &secrets, vault)
    .map_err(MemoryFailure::Unscreened)?
    .check(content)
    .map_err(MemoryFailure::ContainsSecret)
}

const fn scope_filter(scope: MemoryScope) -> ScopeFilter {
  match scope {
    MemoryScope::Task(task) => ScopeFilter::Task(task),
    MemoryScope::Project => ScopeFilter::Project,
  }
}
