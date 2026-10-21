//! Saving an edited task that someone changed meanwhile (design 10.41 and its decisions 17, 18;
//! `docs/architecture/server.md`, decision 26): the store refuses a change made on an older
//! revision. When the two changes touch different content fields, the person's change is put on
//! the stored task and sent again without asking; the organization merges by itself. Only content
//! fields both changed to different values come back to the person, whose text is kept until they
//! choose.

use std::sync::Arc;

use orchester_core::{
  id::TaskId,
  sharing::TaskRevision,
  task::{
    Task, TaskDetails, TaskUpdate,
    content::{Choice, ContentField, ContentMerge},
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};

use super::{
  data::SaveTarget,
  writer::{TaskWriter, WriteError},
};
use crate::sharing::SharingError;

/// How many times a change is put on a task that keeps changing before Orchester stops.
const ATTEMPTS: usize = 3;

/// A change of a task that met a newer version, waiting for the person's choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditConflict {
  pub merge: ContentMerge,
  /// The task as it is stored now.
  pub current: Arc<Task>,
  /// Who saved the newer version and when, as the server names them; unknown for a local task.
  pub by: Option<Arc<str>>,
  pub at: Option<Timestamp>,
  /// The organization the form loaded and as the person left it.
  pub organization: (TaskOrganization, TaskOrganization),
}

impl EditConflict {
  /// The details saved with the person's `choose` for each conflicting field.
  pub fn resolved(&self, choose: impl Fn(ContentField) -> Choice) -> TaskDetails {
    self.merge.merged(choose)
  }
}

/// How saving an edit ended.
#[derive(Debug)]
pub enum EditSaved {
  /// Saved: the stored task and the details it holds, with the other person's changes kept when
  /// they were merged in.
  Saved {
    task: Task,
    details: TaskDetails,
  },
  Conflict(EditConflict),
}

/// The edit the person made, read at a revision.
pub struct Edit<'a> {
  pub task: TaskId,
  pub read: TaskRevision,
  /// The details as the form read them, and as the person changed them.
  pub base: &'a TaskDetails,
  pub details: TaskDetails,
  pub organization: (&'a TaskOrganization, &'a TaskOrganization),
}

/// Saves `edit`, merging it into a task changed meanwhile when no content field conflicts.
pub fn save_edit(writer: &mut TaskWriter, edit: Edit<'_>, now: Timestamp) -> Result<EditSaved, WriteError> {
  save_edit_by(writer, edit, (ChangeActor::Person, now))
}

/// [`save_edit`], the organization changes recorded as `actor`'s.
pub fn save_edit_by(
  writer: &mut TaskWriter,
  edit: Edit<'_>,
  (actor, now): (ChangeActor, Timestamp),
) -> Result<EditSaved, WriteError> {
  let Edit {
    task,
    mut read,
    base,
    mut details,
    organization,
  } = edit;
  let mut base = base.clone();
  for _ in 0..ATTEMPTS {
    let target = SaveTarget::Task(task, read);
    let current = match writer.save_organized_by(target, details.clone(), organization, (actor, now))? {
      TaskUpdate::Saved(task) => return Ok(EditSaved::Saved { task, details }),
      TaskUpdate::Conflict { current } => current,
    };
    let merge = ContentMerge::new(base, details, current.details().clone());
    if !merge.is_clean() {
      let info = writer.db().shared_task_info(task)?;
      return Ok(EditSaved::Conflict(EditConflict {
        merge,
        by: info.as_ref().and_then(|info| info.updated_by.as_deref()).map(Arc::from),
        at: info.and_then(|info| info.updated_at),
        current: Arc::new(current),
        organization: (organization.0.clone(), organization.1.clone()),
      }));
    }
    details = merge.merged(|_| Choice::Mine);
    base = current.details().clone();
    read = current.revision();
  }
  Err(kept_changing(writer))
}

fn kept_changing(writer: &TaskWriter) -> WriteError {
  let server = match writer {
    TaskWriter::Local(_) => None,
    TaskWriter::Shared(store) => Some(store.server().entry().address.clone()),
  };
  WriteError::sharing(SharingError::KeptChanging, server)
}
