//! A person's changes to memory records through the workspace, away from the UI thread.

use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{id::MemoryId, memory::MemoryScope};
use orchester_workspace::{
  background,
  memory::{
    change::{Change, change_in, create_in, delete_in},
    draft::RecordDraft,
  },
  messages::Message,
  secrets::Vault,
};

/// A change the person asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
  /// Change record `.0`, shown at revision `.1`.
  Change(MemoryId, u32, Change),
  /// A new record of the scope, saying what the draft says, pinned or not.
  Create(MemoryScope, RecordDraft, bool),
  /// Delete record `.0`, shown at revision `.1`.
  Delete(MemoryId, u32),
}

impl Write {
  /// The record the change is of; `None` for a new one.
  pub const fn record(&self) -> Option<MemoryId> {
    match self {
      Self::Change(id, ..) | Self::Delete(id, _) => Some(*id),
      Self::Create(..) => None,
    }
  }
}

#[derive(Clone)]
pub struct Start {
  pub repo: PathBuf,
  pub vault: Vault,
  pub write: Write,
}

/// What a finished change was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done(pub Write);

impl DevtoolsInspectable for Done {}

/// Why a change was not kept, and whether the records the screen showed are out of date.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
  pub message: Message,
  pub stale: bool,
}

impl DevtoolsInspectable for Failed {}

impl From<Message> for Failed {
  fn from(message: Message) -> Self {
    Self { message, stale: false }
  }
}

pub type Action = FutureAction<Start, Done, Failed>;

pub fn action(ctx: &mut Ctx) -> Action {
  ctx.future_action(move |start: Start| async move {
    let write = start.write.clone();
    background::run(move || {
      let result = match start.write {
        Write::Change(id, revision, change) => change_in(&start.repo, start.vault.get(), (id, revision), change),
        Write::Create(scope, draft, pinned) => {
          create_in(&start.repo, start.vault.get(), scope, (&draft, pinned)).map(|_| ())
        }
        Write::Delete(id, revision) => delete_in(&start.repo, (id, revision)),
      };
      result.map_err(|failure| Failed {
        message: failure.message(),
        stale: failure.stale(),
      })
    })
    .await
    .map_err(Failed::from)
    .and_then(|result| result)
    .map(|()| Done(write))
  })
}
