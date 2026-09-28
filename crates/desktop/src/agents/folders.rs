//! Extra writable folders as the profile editor adds and removes them: each typed folder is
//! checked by core against the repository and the home folder (`orchester_core::permission::writable`),
//! and a refusal is worded here.

use std::path::Path;

use orchester_core::permission::{
  FolderBounds, WritableFolder, WritableFolderError, WritableFolders, WritableFoldersError,
};

use crate::messages::Message;

/// Where folders typed in the editor are checked: the project's repository and the user's home
/// folder. Reads the file system.
pub fn bounds(repo_root: &Path) -> Result<FolderBounds, Message> {
  FolderBounds::new(repo_root, std::env::home_dir().as_deref()).map_err(|error| {
    tracing::error!(%error, "cannot read the repository to check a writable folder");
    Message::new("agents.permissions.folderInvalid.repository")
  })
}

/// `list` with the folder `typed` added, or why it cannot be. Reads the file system.
pub fn with_folder(list: &WritableFolders, typed: &str, repo_root: &Path) -> Result<WritableFolders, Message> {
  let folder = WritableFolder::new(typed, &bounds(repo_root)?).map_err(|error| folder_error(&error))?;
  let mut entries = list.entries().to_vec();
  entries.push(folder);
  WritableFolders::new(entries).map_err(|error| match error {
    WritableFoldersError::Duplicate(folder) => {
      Message::new("agents.permissions.folderInvalid.duplicate").with("folder", folder.to_string())
    }
    WritableFoldersError::TooMany => {
      Message::new("agents.permissions.folderInvalid.tooMany").with("max", WritableFolders::MAX_ENTRIES.to_string())
    }
  })
}

/// `list` without the folder `removed`.
pub fn without_folder(list: &WritableFolders, removed: &WritableFolder) -> WritableFolders {
  let rest = list.entries().iter().filter(|kept| *kept != removed).cloned().collect();
  WritableFolders::new(rest).expect("a shorter list keeps its entries unique")
}

/// Whether `list` already names the folder `path` (relative to `repo_root` when relative), compared
/// as written, without reading the file system.
pub fn lists(list: &WritableFolders, path: &str, repo_root: &Path) -> bool {
  let Ok(wanted) = WritableFolder::try_from(path.to_owned()) else {
    return false;
  };
  let wanted = wanted.lexical_path(repo_root);
  list
    .entries()
    .iter()
    .any(|folder| folder.lexical_path(repo_root) == wanted)
}

fn folder_error(error: &WritableFolderError) -> Message {
  let with_path = |key, path: &Path| Message::new(key).with("folder", path.display().to_string());
  match error {
    WritableFolderError::Blank => Message::new("agents.permissions.folderInvalid.blank"),
    WritableFolderError::TooLong => {
      Message::new("agents.permissions.folderInvalid.tooLong").with("max", WritableFolder::MAX_CHARS.to_string())
    }
    WritableFolderError::ControlCharacter => Message::new("agents.permissions.folderInvalid.character"),
    WritableFolderError::RepositoryRoot => Message::new("agents.permissions.folderInvalid.repositoryRoot"),
    WritableFolderError::ContainsRepository(path) => {
      with_path("agents.permissions.folderInvalid.containsRepository", path)
    }
    WritableFolderError::InCheckout(path) => with_path("agents.permissions.folderInvalid.inCheckout", path),
    WritableFolderError::OrchesterFolder(path) => with_path("agents.permissions.folderInvalid.orchester", path),
    WritableFolderError::Home => Message::new("agents.permissions.folderInvalid.home"),
    WritableFolderError::ContainsHome(path) => with_path("agents.permissions.folderInvalid.containsHome", path),
    WritableFolderError::FilesystemRoot => Message::new("agents.permissions.folderInvalid.root"),
    WritableFolderError::NotAFolder(path) => with_path("agents.permissions.folderInvalid.notAFolder", path),
    WritableFolderError::Missing(path) => with_path("agents.permissions.folderInvalid.missing", path),
    WritableFolderError::Unreadable { path, .. } => with_path("agents.permissions.folderInvalid.unreadable", path),
  }
}
