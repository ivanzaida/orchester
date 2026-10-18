//! Putting back what a fast-forward that stopped part way wrote. Git refuses up front when a path it
//! would write has uncommitted changes or an untracked or ignored file, so every path it did write
//! matched the base before, in the index and in the files. Git may stop after writing files only,
//! or after writing the index too (e.g. when it cannot lock the branch's ref); HEAD stays at the
//! base either way. So for the paths the merge changes that were clean before it began:
//!
//! - an index entry that holds the merge's version goes back to the base's (`git reset <base> --`), and one that holds
//!   neither is the person's and is not touched;
//! - a file that holds the merge's content, or is gone where the merge deletes it, goes back to the base's version; a
//!   file the base did not have is deleted, with the folders git made for it, which also lets a file the merge turned
//!   into a folder come back.
//!
//! Anything else, e.g. a file the person changed meanwhile, is left as it is. What should go back
//! but cannot is named ([`MergeError::LeftChanged`]).

use std::{
  collections::{HashMap, HashSet},
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
/// Paths are paths, not patterns, for the commands that take pathspecs.
const LITERAL: &str = "--literal-pathspecs";

/// A file's entry in a tree or the index.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
  mode: String,
  object: String,
}

impl Entry {
  fn is_link(&self) -> bool {
    self.mode == "120000"
  }

  fn is_submodule(&self) -> bool {
    self.mode == "160000"
  }
}

/// A path the fast-forward changes that matched the base before it began.
struct Clean {
  path: String,
  /// `None` where the base has no file.
  base: Option<Entry>,
  /// `None` where the merge deletes the file.
  merged: Option<Entry>,
}

/// The paths a fast-forward from the base to the merge changes and that were clean before, read
/// before it begins.
pub(super) struct Touched {
  base: String,
  clean: Vec<Clean>,
  /// Folders git makes for the merge's files where there was none, or a file, the deepest first.
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
      .filter(|entry| entry.base.is_some() || fs::symlink_metadata(root.join(&entry.path)).is_err())
      .collect();
    let mut created: Vec<PathBuf> = clean
      .iter()
      .filter(|entry| entry.merged.is_some())
      .flat_map(|entry| Path::new(&entry.path).ancestors().skip(1).map(Path::to_path_buf))
      .filter(|folder| !folder.as_os_str().is_empty() && !root.join(folder).is_dir())
      .collect::<HashSet<_>>()
      .into_iter()
      .collect();
    created.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
    Ok(Self {
      base: base.to_owned(),
      clean,
      created,
    })
  }

  /// Puts back what git wrote; the paths that could not be, as [`MergeError::LeftChanged`].
  pub(super) fn restore<R: CommandRunner>(&self, git: &Git<'_, R>) -> Result<(), MergeError> {
    let mut left = Left::default();
    self.restore_index(git, &mut left)?;
    let root = git.dir();
    let written = self.written(git, &left.paths)?;
    for entry in written.iter().filter(|entry| entry.base.is_none()) {
      let target = root.join(&entry.path);
      let removed = if entry.merged.as_ref().is_some_and(Entry::is_submodule) {
        fs::remove_dir(&target)
      } else {
        fs::remove_file(&target)
      };
      if let Err(error) = removed
        && error.kind() != io::ErrorKind::NotFound
      {
        left.add(&entry.path, error.to_string());
      }
    }
    for folder in &self.created {
      // Only a folder that is not empty refuses: it holds what was not put back, named here, or
      // what the person put there meanwhile. It stays, and losing the error loses nothing.
      let _ = fs::remove_dir(root.join(folder));
    }
    let mut back: Vec<&Clean> = written.into_iter().filter(|entry| entry.base.is_some()).collect();
    for entry in self.deleted() {
      let target = root.join(&entry.path);
      if left.paths.contains(entry.path.as_str()) {
        continue;
      }
      match fs::symlink_metadata(&target) {
        Err(_) => back.push(entry),
        // A file the merge turned into a folder that still holds what is not git's to delete.
        Ok(metadata) if metadata.is_dir() => left.add(&entry.path, format!("{} is a folder now", entry.path)),
        Ok(_) => {}
      }
    }
    for chunk in back.chunks(PATHS_PER_COMMAND) {
      let mut args = vec!["checkout-index", "--force", "--quiet", "--"];
      args.extend(chunk.iter().map(|entry| entry.path.as_str()));
      let output = git.output(&args)?;
      if !output.success() {
        for entry in chunk {
          left.add(&entry.path, message(&output));
        }
      }
    }
    left.into_result()
  }

  /// Puts the index entries git wrote back to the base's; one that holds neither the base's nor the
  /// merge's version is the person's and is left, named.
  fn restore_index<R: CommandRunner>(&self, git: &Git<'_, R>, left: &mut Left) -> Result<(), MergeError> {
    let mut staged = HashMap::new();
    let paths: Vec<&str> = self.clean.iter().map(|entry| entry.path.as_str()).collect();
    for chunk in paths.chunks(PATHS_PER_COMMAND) {
      let mut args = vec![LITERAL, "ls-files", "--stage", "-z", "--"];
      args.extend(chunk);
      let output = git.output(&args)?;
      if !output.success() {
        return Err(GitError::Failed(message(&output)).into());
      }
      staged.extend(parse_ls_files(&output.stdout)?);
    }
    let mut reset = Vec::new();
    for entry in &self.clean {
      let now = staged.get(&entry.path).cloned();
      if now == entry.base {
        continue;
      }
      if now == entry.merged {
        reset.push(entry.path.as_str());
      } else {
        left.add(
          &entry.path,
          format!("the index holds another version of {}", entry.path),
        );
      }
    }
    for chunk in reset.chunks(PATHS_PER_COMMAND) {
      let mut args = vec![LITERAL, "reset", "--quiet", self.base.as_str(), "--"];
      args.extend(chunk);
      let output = git.output(&args)?;
      if !output.success() {
        for path in chunk {
          left.add(path, message(&output));
        }
      }
    }
    Ok(())
  }

  /// The paths the merge deletes from the base.
  fn deleted(&self) -> impl Iterator<Item = &Clean> {
    self
      .clean
      .iter()
      .filter(|entry| entry.base.is_some() && entry.merged.is_none())
  }

  /// The clean paths, except `skipped`, whose file holds the merge's content now.
  fn written<R: CommandRunner>(&self, git: &Git<'_, R>, skipped: &HashSet<String>) -> Result<Vec<&Clean>, GitError> {
    let root = git.dir();
    let mut written = Vec::new();
    let mut hashed = Vec::new();
    for entry in &self.clean {
      let Some(merged) = &entry.merged else {
        continue;
      };
      let Ok(metadata) = fs::symlink_metadata(root.join(&entry.path)) else {
        continue;
      };
      if skipped.contains(&entry.path) {
        continue;
      }
      if merged.is_link() || merged.is_submodule() {
        // No edit of the person's can be told apart from git's here; it was clean before.
        written.push(entry);
      } else if !metadata.is_dir() {
        hashed.push((entry, merged.object.as_str()));
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

/// The paths that could not be put back, and the last reason why.
#[derive(Default)]
struct Left {
  paths: HashSet<String>,
  files: Vec<String>,
  detail: String,
}

impl Left {
  fn add(&mut self, path: &str, detail: String) {
    if self.paths.insert(path.to_owned()) {
      self.files.push(path.to_owned());
    }
    self.detail = detail;
  }

  fn into_result(self) -> Result<(), MergeError> {
    if self.files.is_empty() {
      Ok(())
    } else {
      Err(MergeError::LeftChanged {
        files: self.files,
        detail: self.detail,
      })
    }
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
    let [old_mode, new_mode, old, new, _] = parts[..] else {
      return Err(unrecognized(header));
    };
    let entry = |mode: &str, object: &str| {
      (object != NO_OBJECT).then(|| Entry {
        mode: mode.to_owned(),
        object: object.to_owned(),
      })
    };
    entries.push(Clean {
      path: path.to_owned(),
      base: entry(old_mode, old),
      merged: entry(new_mode, new),
    });
  }
  Ok(entries)
}

/// The entries `git ls-files --stage -z` lists, `<mode> <object> <stage>`, a tab and the path each, by path.
/// Only stage 0 is a version of the file; a fast-forward writes no conflict stages.
fn parse_ls_files(raw: &str) -> Result<HashMap<String, Entry>, GitError> {
  let mut entries = HashMap::new();
  for field in raw.split('\0').filter(|field| !field.is_empty()) {
    let unrecognized = || GitError::Unrecognized {
      expected: "an ls-files entry",
      detail: field.to_owned(),
    };
    let (header, path) = field.split_once('\t').ok_or_else(unrecognized)?;
    let parts: Vec<&str> = header.split(' ').collect();
    let [mode, object, stage] = parts[..] else {
      return Err(unrecognized());
    };
    if stage == "0" {
      entries.insert(
        path.to_owned(),
        Entry {
          mode: mode.to_owned(),
          object: object.to_owned(),
        },
      );
    }
  }
  Ok(entries)
}
