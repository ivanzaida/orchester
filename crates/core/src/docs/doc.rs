//! A doc: its place in the tree, its title and whether it is archived. Its text lives in its versions.

use thiserror::Error;

use super::{DocSlug, DocTitle};
use crate::{
  id::{DocFolderId, DocId},
  time::Timestamp,
};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DocError {
  #[error("doc {0} is archived already")]
  Archived(DocId),
  #[error("doc {0} is not archived")]
  NotArchived(DocId),
  #[error("doc {0} cannot be archived before it was created")]
  ArchivedBeforeCreated(DocId),
}

/// One doc of a project. Archiving hides it from the tree and keeps every version; nothing about a doc is deleted
/// from the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doc {
  id: DocId,
  folder: Option<DocFolderId>,
  title: DocTitle,
  slug: DocSlug,
  position: u32,
  created_at: Timestamp,
  archived_at: Option<Timestamp>,
}

impl Doc {
  /// A doc in `folder` (`None`: the top level) at `position` among its siblings. `slug` comes from the title
  /// ([`DocSlug::from_title`]), made unique within the folder by the store.
  pub const fn new(
    id: DocId,
    folder: Option<DocFolderId>,
    title: DocTitle,
    slug: DocSlug,
    position: u32,
    created_at: Timestamp,
  ) -> Self {
    Self {
      id,
      folder,
      title,
      slug,
      position,
      created_at,
      archived_at: None,
    }
  }

  pub fn archive(&mut self, now: Timestamp) -> Result<(), DocError> {
    if self.archived_at.is_some() {
      return Err(DocError::Archived(self.id));
    }
    if now < self.created_at {
      return Err(DocError::ArchivedBeforeCreated(self.id));
    }
    self.archived_at = Some(now);
    Ok(())
  }

  /// Takes an archived doc back into the tree, with every version it had.
  pub fn restore(&mut self) -> Result<(), DocError> {
    if self.archived_at.take().is_none() {
      return Err(DocError::NotArchived(self.id));
    }
    Ok(())
  }

  pub const fn id(&self) -> DocId {
    self.id
  }

  /// The folder it is in; `None` at the top level.
  pub const fn folder(&self) -> Option<DocFolderId> {
    self.folder
  }

  pub const fn title(&self) -> &DocTitle {
    &self.title
  }

  pub const fn slug(&self) -> &DocSlug {
    &self.slug
  }

  pub const fn position(&self) -> u32 {
    self.position
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// When it was archived; `None` while it is in the tree.
  pub const fn archived_at(&self) -> Option<Timestamp> {
    self.archived_at
  }

  pub const fn is_archived(&self) -> bool {
    self.archived_at.is_some()
  }
}
