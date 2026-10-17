//! Putting back what a fast-forward that stopped part way wrote. Git refuses up front when a path it
//! would write has uncommitted changes or an untracked or ignored file, so every path it did write
//! matched the base before; and a stopped fast-forward leaves HEAD and the index at the base. So
//! the paths the merge changes that were clean before it began, and now hold the merge's content
//! (or are gone, where the merge deletes them), are git's writing: they go back to the index's
//! version, which is the base's, and a file the base did not have is deleted with the folders made
//! for it. Anything else, e.g. a file the person changed meanwhile, is left as it is.

use std::{
  collections::HashSet,
  fs, io,
  path::{Path, PathBuf},
};

use super::MergeError;
use crate::{
  command::CommandRunner,
  integration::{
    GitError,
    git::{Git, message},
  },
};

/// Paths handed to one git command, well within the command-line limit of Windows.
const PATHS_PER_COMMAND: usize = 64;
/// The object name git gives a side that has no file.
const NO_OBJECT: &str = "0000000000000000000000000000000000000000";

/// A path the fast-forward changes that matched the base before it began.
struct Clean {
  path: String,
  /// The base has the file: putting it back writes the index's version.
  in_base: bool,
  /// The merge's content, by mode: `None` where the merge deletes it.
  merged: Option<Merged>,
}

enum Merged {
  /// A file whose content git can hash, with the merge's object name.
  File(String),
  /// A symbolic link: put back whenever it is there, since no edit of the person's can be told apart.
  Link,
  /// A submodule, of which a fast-forward writes at most an empty folder.
  Submodule,
}

/// The paths a fast-forward from the base to the merge changes and that were clean before, read
/// before it begins.
pub(super) struct Touched {
  clean: Vec<Clean>,
  /// Folders missing before that git creates for new files, the deepest first.
  created: Vec<PathBuf>,
}

impl Touched {
  pub(super) fn read<R: CommandRunner>(git: &Git<'_, R>, (base, commit): (&str, &str)) -> Result<Self, GitError> {
    let status = git.output(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    if !status.success() {
      return Err(GitError::Failed(message(&status)));
    }
    let dirty = changed_paths(&status.stdout);
    let diff = git.output(&["diff-tree", "-r", "-z", "--no-renames", "--no-commit-id", base, commit])?;
    if !diff.success() {
      return Err(GitError::Failed(message(&diff)));
    }
    let root = git.dir();
    let clean: Vec<Clean> = parse_diff_tree(&diff.stdout)?
      .into_iter()
      .filter(|entry| !dirty.contains(entry.path.as_str()))
      .filter(|entry| entry.in_base || fs::symlink_metadata(root.join(&entry.path)).is_err())
      .collect();
    let mut created: Vec<PathBuf> = clean
      .iter()
      .filter(|entry| !entry.in_base)
      .flat_map(|entry| Path::new(&entry.path).ancestors().skip(1).map(Path::to_path_buf))
      .filter(|folder| !folder.as_os_str().is_empty() && !root.join(folder).exists())
      .collect::<HashSet<_>>()
      .into_iter()
      .collect();
    created.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
    Ok(Self { clean, created })
  }

  /// Puts back what git wrote; the files that could not be, as [`MergeError::LeftChanged`].
  pub(super) fn restore<R: CommandRunner>(&self, git: &Git<'_, R>) -> Result<(), MergeError> {
    let written = self.written(git)?;
    let root = git.dir();
    let mut left = Vec::new();
    let mut detail = String::new();
    let (in_base, added): (Vec<&Clean>, Vec<&Clean>) = written.into_iter().partition(|entry| entry.in_base);
    for chunk in in_base.chunks(PATHS_PER_COMMAND) {
      let mut args = vec!["checkout-index", "--force", "--quiet", "--"];
      args.extend(chunk.iter().map(|entry| entry.path.as_str()));
      let output = git.output(&args)?;
      if !output.success() {
        left.extend(chunk.iter().map(|entry| entry.path.clone()));
        detail = message(&output);
      }
    }
    for entry in added {
      let target = root.join(&entry.path);
      let removed = if matches!(entry.merged, Some(Merged::Submodule)) {
        fs::remove_dir(&target)
      } else {
        fs::remove_file(&target)
      };
      if let Err(error) = removed
        && error.kind() != io::ErrorKind::NotFound
      {
        left.push(entry.path.clone());
        detail = error.to_string();
      }
    }
    for folder in &self.created {
      // Only a folder that is not empty refuses: it holds what was not put back, named above, or
      // what the person put there meanwhile. It stays, and losing the error loses nothing.
      let _ = fs::remove_dir(root.join(folder));
    }
    if left.is_empty() {
      Ok(())
    } else {
      Err(MergeError::LeftChanged { files: left, detail })
    }
  }

  /// The clean paths git wrote: they hold the merge's content now, or are gone where it deletes them.
  fn written<R: CommandRunner>(&self, git: &Git<'_, R>) -> Result<Vec<&Clean>, GitError> {
    let root = git.dir();
    let mut written = Vec::new();
    let mut hashed = Vec::new();
    for entry in &self.clean {
      let present = fs::symlink_metadata(root.join(&entry.path)).is_ok();
      match &entry.merged {
        None if !present => written.push(entry),
        None => {}
        Some(Merged::File(object)) if present => hashed.push((entry, object.as_str())),
        Some(Merged::File(_)) => {}
        Some(Merged::Link | Merged::Submodule) if present => written.push(entry),
        Some(Merged::Link | Merged::Submodule) => {}
      }
    }
    for chunk in hashed.chunks(PATHS_PER_COMMAND) {
      let mut args = vec!["hash-object", "--"];
      args.extend(chunk.iter().map(|(entry, _)| entry.path.as_str()));
      let objects = git.stdout(&args)?;
      for ((entry, merged), object) in chunk.iter().zip(objects.lines()) {
        if object.trim() == *merged {
          written.push(entry);
        }
      }
    }
    Ok(written)
  }
}

/// Every path `git status --porcelain=v1 -z` names: `XY <path>` per entry, a rename or copy
/// followed by its original path, which is changed too.
fn changed_paths(status: &str) -> HashSet<&str> {
  let mut paths = HashSet::new();
  let mut entries = status.split('\0').filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let (Some(code), Some(path)) = (entry.get(..2), entry.get(3..)) else {
      continue;
    };
    paths.insert(path);
    if code.contains(['R', 'C'])
      && let Some(original) = entries.next()
    {
      paths.insert(original);
    }
  }
  paths
}

/// The entries of `git diff-tree -r -z --no-renames`: `:<old mode> <new mode> <old> <new> <status>`
/// then the path, each ended by NUL.
fn parse_diff_tree(raw: &str) -> Result<Vec<Clean>, GitError> {
  let unrecognized = |detail: &str| GitError::Unrecognized {
    expected: "a diff-tree entry",
    detail: detail.to_owned(),
  };
  let mut entries = Vec::new();
  let mut fields = raw.split('\0').filter(|field| !field.is_empty());
  while let Some(header) = fields.next() {
    let path = fields.next().ok_or_else(|| unrecognized(header))?;
    let parts: Vec<&str> = header.trim_start_matches(':').split(' ').collect();
    let [_, new_mode, old, new, _] = parts[..] else {
      return Err(unrecognized(header));
    };
    let merged = (new != NO_OBJECT).then(|| match new_mode {
      "120000" => Merged::Link,
      "160000" => Merged::Submodule,
      _ => Merged::File(new.to_owned()),
    });
    entries.push(Clean {
      path: path.to_owned(),
      in_base: old != NO_OBJECT,
      merged,
    });
  }
  Ok(entries)
}
