//! Placing the paths a part names, reads and writes. A part may run in more than one folder: a `cd`
//! might not run (`false && cd sub; rm -rf ../..`), or run in a pipeline or a subshell where it changes
//! nothing after it, so after a `cd` both folders are followed and every path is placed from each; the
//! strictest place wins. When a folder cannot be told, every path is Unknown.

use std::path::{Path, PathBuf};

use super::{CommandCategory, CommandScope, part::CommandPart, paths::Place, programs::operands};

/// The folders a part may run in; `None` when one cannot be told.
pub(super) type Folders = Option<Vec<PathBuf>>;

/// The most folders a command line is followed in; past them it cannot be told.
const MOST_FOLDERS: usize = 8;

/// Where a part runs.
#[derive(Clone, Copy)]
pub(super) struct Placing<'a> {
  pub(super) scope: &'a CommandScope,
  pub(super) folders: Option<&'a [PathBuf]>,
}

impl Placing<'_> {
  /// Each path `word` names, one per folder the part may run in; `None` when one cannot be told.
  fn locate(&self, word: &str) -> Option<Vec<(PathBuf, Place)>> {
    self
      .folders?
      .iter()
      .map(|folder| self.scope.locate(folder, word))
      .collect()
  }

  /// `word`, a file the part reads or a word that may name one: a credential location is noted, a
  /// folder outside the worktree kept.
  pub(super) fn read(&self, part: &mut CommandPart, word: &str) {
    if word.is_empty() || word == "-" {
      return;
    }
    let Some(located) = self.locate(word) else {
      part.at_least(CommandCategory::Unknown);
      return;
    };
    for (path, place) in located {
      if place == Place::Secret && part.secret_word.is_none() {
        part.secret_word = Some(word.to_owned());
      }
      note(part, &path, place);
    }
  }

  /// `target`, a file the part writes or, with `delete`, removes: a write outside the worktree is
  /// Outside write, a delete outside the temporary folder History / destructive.
  pub(super) fn write(&self, part: &mut CommandPart, target: &str, delete: bool) {
    if target == "-" || is_null_device(target) {
      return;
    }
    let Some(located) = self.locate(target) else {
      part.at_least(CommandCategory::Unknown);
      return;
    };
    for (path, place) in located {
      let category = match place {
        Place::Granted => CommandCategory::Worktree,
        Place::Temp => CommandCategory::OutsideWrite,
        Place::Outside | Place::Secret if delete => CommandCategory::HistoryDestructive,
        Place::Outside | Place::Secret => CommandCategory::OutsideWrite,
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

  /// `word` must lie in the worktree or a writable folder, else the part is Unknown.
  pub(super) fn inside(&self, part: &mut CommandPart, word: &str) {
    if word.is_empty() || is_null_device(word) {
      return;
    }
    let inside = self
      .locate(word)
      .is_some_and(|located| located.iter().all(|(_, place)| *place == Place::Granted));
    if !inside {
      part.at_least(CommandCategory::Unknown);
    }
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

fn is_null_device(target: &str) -> bool {
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

/// The folders later parts may run in after `cd args`: those before it and the one it goes to.
pub(super) fn change_directory(scope: &CommandScope, folders: &mut Folders, args: &[String]) {
  let target = operands(args).into_iter().next().unwrap_or_else(|| "~".to_owned());
  let reached: Option<Vec<PathBuf>> = match folders.as_deref() {
    Some(current) if target != "-" => current.iter().map(|folder| scope.path(folder, &target)).collect(),
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
