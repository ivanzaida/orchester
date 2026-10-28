//! Folders of the Docs tree and the rule that keeps it a tree.

use thiserror::Error;

use crate::{id::DocFolderId, name::Name};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FolderError {
  #[error("folder {0} cannot be its own parent")]
  IntoItself(DocFolderId),
  #[error("folder {folder} cannot move into {parent}, a folder inside it")]
  IntoOwnSubfolder { folder: DocFolderId, parent: DocFolderId },
  #[error("folder {0} is not in the tree")]
  Unknown(DocFolderId),
  /// The tree given already has a cycle: rows no move made.
  #[error("the folders given do not form a tree")]
  NotATree,
}

/// A folder of the Docs tree: at the top level or inside another folder, at a position among its siblings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocFolder {
  id: DocFolderId,
  parent: Option<DocFolderId>,
  name: Name,
  position: u32,
}

impl DocFolder {
  pub fn new(id: DocFolderId, parent: Option<DocFolderId>, name: Name, position: u32) -> Result<Self, FolderError> {
    if parent == Some(id) {
      return Err(FolderError::IntoItself(id));
    }
    Ok(Self {
      id,
      parent,
      name,
      position,
    })
  }

  /// Moves this folder under `parent` (`None`: the top level) at `position`, once [`check_folder_move`] allows it
  /// against `tree`, the project's folders.
  pub fn move_to(&mut self, parent: Option<DocFolderId>, position: u32, tree: &[Self]) -> Result<(), FolderError> {
    check_folder_move(tree, self.id, parent)?;
    self.parent = parent;
    self.position = position;
    Ok(())
  }

  pub fn rename(&mut self, name: Name) {
    self.name = name;
  }

  pub const fn id(&self) -> DocFolderId {
    self.id
  }

  /// The folder it is in; `None` at the top level.
  pub const fn parent(&self) -> Option<DocFolderId> {
    self.parent
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }

  pub const fn position(&self) -> u32 {
    self.position
  }
}

/// Whether `moved`, a folder of `tree`, may move under `parent`: never into itself or a folder inside it, which
/// would make a cycle, and only into a folder of the tree.
pub fn check_folder_move(
  tree: &[DocFolder],
  moved: DocFolderId,
  parent: Option<DocFolderId>,
) -> Result<(), FolderError> {
  let parent_of = |id: DocFolderId| {
    tree
      .iter()
      .find(|folder| folder.id == id)
      .map(DocFolder::parent)
      .ok_or(FolderError::Unknown(id))
  };
  parent_of(moved)?;
  let Some(parent) = parent else {
    return Ok(());
  };
  if parent == moved {
    return Err(FolderError::IntoItself(moved));
  }
  let mut above = Some(parent);
  for _ in 0..=tree.len() {
    let Some(folder) = above else {
      return Ok(());
    };
    if folder == moved {
      return Err(FolderError::IntoOwnSubfolder { folder: moved, parent });
    }
    above = parent_of(folder)?;
  }
  Err(FolderError::NotATree)
}
