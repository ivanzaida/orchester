//! Placing the paths a part names, reads and writes. A part may run in more than one folder: a `cd`
//! might not run (`false && cd sub; rm -rf ../..`), or run in a pipeline or a subshell where it changes
//! nothing after it, so after a `cd` both folders are followed and every path is placed from each; the
//! strictest place wins. When a folder cannot be told, every path is Unknown.

use std::path::{Path, PathBuf};

use super::{CommandCategory, CommandScope, part::CommandPart, paths::Place, toolchains, variables::is_assignment};

/// The folders a part may run in; `None` when one cannot be told.
pub(super) type Folders = Option<Vec<PathBuf>>;

/// What the parts before a part leave it: the folders it may run in, and the paths they write, create,
/// copy or move to, as written. The file system is read before the line runs, so what an earlier part
/// makes is not there yet: a write under it is placed by its words only, which a link brought there
/// would not follow.
#[derive(Clone, Debug)]
pub(super) struct Line {
  pub(super) folders: Folders,
  pub(super) made: Vec<PathBuf>,
}

impl Line {
  pub(super) const fn new(folders: Folders) -> Self {
    Self {
      folders,
      made: Vec::new(),
    }
  }

  pub(super) fn placing<'a>(&'a self, scope: &'a CommandScope) -> Placing<'a> {
    Placing {
      scope,
      folders: self.folders.as_deref(),
      made: &self.made,
    }
  }

  /// Notes the paths `targets` (as a part names them) as made by the line.
  pub(super) fn made(&mut self, scope: &CommandScope, targets: &[String]) {
    let Some(folders) = &self.folders else {
      return;
    };
    for target in targets
      .iter()
      .filter(|target| *target != "-" && !is_null_device(target))
    {
      self
        .made
        .extend(folders.iter().filter_map(|folder| scope.written(folder, target)));
    }
  }
}

/// The most folders a command line is followed in; past them it cannot be told.
const MOST_FOLDERS: usize = 8;

/// Where a part runs.
#[derive(Clone, Copy)]
pub(super) struct Placing<'a> {
  pub(super) scope: &'a CommandScope,
  pub(super) folders: Option<&'a [PathBuf]>,
  made: &'a [PathBuf],
}

impl<'a> Placing<'a> {
  /// Each path `word` names, one per folder the part may run in; `None` when one cannot be told.
  fn locate(&self, word: &str) -> Option<Vec<(PathBuf, Place)>> {
    let located: Option<Vec<Vec<(PathBuf, Place)>>> = self
      .folders?
      .iter()
      .map(|folder| self.scope.locate(folder, word))
      .collect();
    located.map(|located| located.into_iter().flatten().collect())
  }

  /// `word`, a file the part reads or a word that may name one: a credential location is noted, a
  /// folder outside the worktree kept. A plain reader reads the dependency sources of a toolchain as it reads
  /// the worktree ([`toolchains::reads_sources`]); any other part there reaches outside. A word that cannot be
  /// placed could name a credential location: the part no longer only reads ([`CommandPart::only_reads`]).
  pub(super) fn read(&self, part: &mut CommandPart, word: &str) {
    if word.is_empty() || word == "-" {
      return;
    }
    let Some(located) = self.located(part, word) else {
      part.at_least(CommandCategory::Unknown);
      part.only_reads = false;
      return;
    };
    for (path, place) in located {
      if place == Place::Secret && part.secret_word.is_none() {
        part.secret_word = Some(word.to_owned());
      }
      if !(place == Place::Sources && toolchains::reads_sources(part)) {
        note(part, &path, place);
      }
    }
  }

  /// [`Self::locate`], noting what `word` names among what `part` touches ([`CommandPart::touches`]).
  fn located(&self, part: &mut CommandPart, word: &str) -> Option<Vec<(PathBuf, Place)>> {
    let located = self.locate(word);
    match &located {
      Some(located) => part.touches.extend(located.iter().map(|(path, _)| path.clone())),
      None => part.unplaced = true,
    }
    located
  }

  /// Notes what the words of a part Orchester did not read may name ([`CommandPart::touches`]): the folder it runs
  /// in, and each word after its program as [`named`] reads it, an assignment's value for one before the program.
  pub(super) fn touch_words(&self, part: &mut CommandPart) {
    let words = part.words.clone();
    self.touch(part, ".");
    let mut program = false;
    for word in &words {
      if !program && let Some((_, value)) = word.split_once('=').filter(|_| is_assignment(word)) {
        self.touch(part, value);
      } else if !program {
        program = true;
      } else {
        for named in named(std::slice::from_ref(word)) {
          self.touch(part, named);
        }
      }
    }
  }

  fn touch(&self, part: &mut CommandPart, word: &str) {
    if !(word.is_empty() || word == "-" || is_null_device(word)) {
      self.located(part, word);
    }
  }

  /// `target`, a file the part writes or, with `delete`, removes: a write outside the worktree is
  /// Outside write, a delete outside the temporary folder History / destructive.
  pub(super) fn write(&self, part: &mut CommandPart, target: &str, delete: bool) {
    if target == "-" || is_null_device(target) {
      return;
    }
    let Some(located) = self.located(part, target).filter(|_| !self.under_made(target)) else {
      part.at_least(CommandCategory::Unknown);
      return;
    };
    for (path, place) in located {
      let category = match place {
        // What no edit may change is no worktree write; a person answers each such write.
        Place::Granted if self.scope.write_refused(&path, delete) => CommandCategory::Unknown,
        Place::Granted => CommandCategory::Worktree,
        Place::Temp => CommandCategory::OutsideWrite,
        Place::Outside | Place::Sources | Place::Secret if delete => CommandCategory::HistoryDestructive,
        Place::Outside | Place::Sources | Place::Secret => CommandCategory::OutsideWrite,
      };
      let written = Some(target.to_owned());
      match category {
        CommandCategory::HistoryDestructive => part.outside_delete = part.outside_delete.take().or(written),
        CommandCategory::OutsideWrite => part.outside_write = part.outside_write.take().or(written),
        _ => {}
      }
      if place == Place::Secret && part.secret_word.is_none() {
        part.secret_word = Some(target.to_owned());
      }
      part.at_least(category);
      note(part, &path, place);
    }
  }

  /// Whether `target` lies under a path an earlier part of the line makes, in a folder that is not there
  /// now as a plain folder: what that part brings there cannot be told before the line runs.
  fn under_made(&self, target: &str) -> bool {
    let Some(folders) = self.folders else {
      return true;
    };
    folders.iter().any(|folder| {
      self.scope.written(folder, target).is_none_or(|path| {
        self.made.iter().any(|made| path != *made && path.starts_with(made))
          && !path.parent().is_some_and(|parent| self.scope.plain_folders(parent))
      })
    })
  }

  /// This placing from `folders` instead (where `git -C <folder>` reads its arguments).
  pub(super) const fn placed_in<'b>(&self, folders: Option<&'b [PathBuf]>) -> Placing<'b>
  where
    'a: 'b,
  {
    Placing {
      scope: self.scope,
      folders,
      made: self.made,
    }
  }

  /// Whether `word` names a regular file in the worktree or a writable folder, no folder on its way and
  /// not the file a link or junction, where an edit may write (no `.git`, no `.orchester/`).
  pub(super) fn plain_file(&self, word: &str) -> bool {
    self.plain(word, |path| {
      std::fs::symlink_metadata(path).is_ok_and(|found| found.is_file() && !found.file_type().is_symlink())
    })
  }

  /// Whether `word` names a repository nested in the worktree or a writable folder: a plain folder below
  /// it that holds its own `.git` folder (not a file that points elsewhere, not a link), where an edit may
  /// write.
  pub(super) fn nested_repository(&self, word: &str) -> bool {
    self.plain(word, |path| {
      std::fs::symlink_metadata(path).is_ok_and(|found| found.is_dir() && !found.file_type().is_symlink())
        && !self.scope.is_root(path)
        && std::fs::symlink_metadata(path.join(".git"))
          .is_ok_and(|found| found.is_dir() && !found.file_type().is_symlink())
    })
  }

  /// Whether `word`, from every folder the part may run in, is a path in the worktree (not a writable
  /// folder, which may hold the user's own repositories) whose folders are plain folders, where an edit may
  /// write, and that `is` says it is.
  fn plain(&self, word: &str, is: impl Fn(&Path) -> bool) -> bool {
    !word.contains(['*', '?', '['])
      && self.folders.is_some_and(|folders| {
        folders.iter().all(|folder| {
          self.scope.written(folder, word).is_some_and(|path| {
            self.scope.in_worktree(&path)
              && path.parent().is_some_and(|parent| self.scope.plain_folders(parent))
              && !self.scope.write_refused(&path, false)
              && is(&path)
          })
        })
      })
  }

  /// Whether `cp` or `mv` of `source` copies or moves a folder or a link (or what cannot be told), whose
  /// tree can bring links to the target.
  pub(super) fn moves_tree(&self, source: &str) -> bool {
    let Some(folders) = self.folders else {
      return true;
    };
    source.contains(['*', '?', '['])
      || folders.iter().any(|folder| {
        self.scope.written(folder, source).is_none_or(|path| {
          std::fs::symlink_metadata(path).is_ok_and(|found| found.is_dir() || found.file_type().is_symlink())
        })
      })
  }

  /// `word` must lie in the worktree or a writable folder, else the part is Unknown; whether it does.
  pub(super) fn inside(&self, part: &mut CommandPart, word: &str) -> bool {
    if word.is_empty() || is_null_device(word) {
      return true;
    }
    let inside = self
      .located(part, word)
      .is_some_and(|located| located.iter().all(|(_, place)| *place == Place::Granted));
    if !inside {
      part.at_least(CommandCategory::Unknown);
    }
    inside
  }
}

fn note(part: &mut CommandPart, path: &Path, place: Place) {
  part.reads_secrets |= place == Place::Secret;
  if place != Place::Granted {
    let folder = if path.is_dir() {
      path.to_path_buf()
    } else {
      path.parent().unwrap_or(path).to_path_buf()
    };
    part.reaches(folder);
  }
}

pub(super) fn is_null_device(target: &str) -> bool {
  matches!(target, "/dev/null" | "$null") || target.eq_ignore_ascii_case("nul")
}

/// The words of a part's arguments that may name a file: each argument, the value of an option written
/// `--name=value` or `-Name:value`, and the value attached to a short option (`-F~/x`).
pub(super) fn named(args: &[String]) -> Vec<&str> {
  args
    .iter()
    .filter_map(|arg| match arg.strip_prefix('-') {
      None => Some(arg.as_str()),
      Some(option) => option
        .split_once('=')
        .or_else(|| option.split_once(':'))
        .map(|(_, value)| value)
        .or_else(|| {
          option
            .get(1..)
            .filter(|value| !option.starts_with('-') && !value.is_empty())
        }),
    })
    .collect()
}

/// The folders later parts may run in after `cd args`: those before it and the one it goes to. Only
/// `cd` alone (the home folder) and `cd <folder>` are followed; an option (`-P`, `-L`, `--`), the
/// previous folder (`-`), or a second operand (zsh's `cd old new` replaces `old` in the current folder's
/// path) leaves where later parts run unknown.
pub(super) fn change_directory(scope: &CommandScope, folders: &mut Folders, args: &[String]) {
  let target = match args {
    [] => Some("~"),
    [target] if !target.starts_with('-') => Some(target.as_str()),
    _ => None,
  };
  let reached: Option<Vec<PathBuf>> = match (folders.as_deref(), target) {
    (Some(current), Some(target)) => current.iter().map(|folder| scope.folder(folder, target)).collect(),
    _ => None,
  };
  *folders = match (folders.take(), reached) {
    (Some(mut current), Some(reached)) => {
      for folder in reached {
        if !current.contains(&folder) {
          current.push(folder);
        }
      }
      (current.len() <= MOST_FOLDERS).then_some(current)
    }
    _ => None,
  };
}
