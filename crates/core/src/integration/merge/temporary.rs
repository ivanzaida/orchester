//! The merge commit, made in a temporary worktree so that neither the merge nor what stops it
//! reaches the user's checkout, whose uncommitted work stays out of it. The worktree is
//! `<git common dir>/orchester-merge-<run id>`: inside the repository's own git folder, so on the
//! same disk as the checkout and outside its working tree and `git status`, and named by the run,
//! so one a killed merge left is the next merge of that run's to remove. It is detached at the
//! checkout's commit and filled with `git reset --hard`, which unlike a checkout runs no
//! `post-checkout` hook, and removed again on every path; no ref stays.
//!
//! The merge there runs the hooks a merge commit runs (`pre-merge-commit`, `prepare-commit-msg`,
//! `commit-msg`, `post-merge`) from the hooks folder the checkout uses (`core.hooksPath`, relative
//! ones resolved against the checkout, as git resolves them there), with the same identity:
//! worktrees share the configuration. Unlike a merge in the checkout, the hooks' working folder is
//! the temporary worktree, which holds the tracked files only: nothing ignored, untracked or
//! uncommitted of the checkout's, e.g. no installed tools. `post-merge` runs there, and once more in
//! the checkout when the merge is brought in.

use std::{fs, path::Path};

use super::{MergeError, MergeStep, TemporaryLeft};
use crate::{
  command::CommandRunner,
  id::RunId,
  integration::{
    GitError,
    git::{Git, message},
  },
};

/// The commit made on the base: a merge commit titled `title`, or one squashed commit of the run's work with its
/// `title` and `body` (a Deliver step's *Squash*).
#[derive(Clone, Copy, Debug)]
pub(super) enum Made<'a> {
  Merge { title: &'a str },
  Squash { title: &'a str, body: &'a str },
}

/// Merges `head` into `base` with the commit `made`, the merge of `run`: the commit, and the temporary worktree
/// when it could not be removed afterwards. A git that stops answering is stopped with everything it started, and the
/// temporary worktree is removed after it as after any merge that stopped.
pub(super) fn merge_commit<R: CommandRunner>(
  git: &Git<'_, R>,
  run: RunId,
  (base, head): (&str, &str),
  made: Made<'_>,
) -> Result<(String, Option<TemporaryLeft>), MergeError> {
  let at_worktree = |error: GitError| MergeError::from(error).at(MergeStep::TemporaryWorktree);
  let hooks = git
    .stdout(&["rev-parse", "--path-format=absolute", "--git-path", "hooks"])
    .map_err(at_worktree)?;
  let path = temporary_path(git, run).map_err(at_worktree)?;
  remove(git, &path).map_err(GitError::Failed)?;
  let merged = merge_in(git, (&path, &hooks), (base, head), made);
  // A merge that stopped says why; a folder that stays after it is the next merge's to remove.
  let left = remove(git, &path).err().map(|detail| TemporaryLeft {
    path: path.clone(),
    detail,
  });
  Ok((merged?, left))
}

/// Removes the temporary worktree a merge of `run` that the app or the runner ended left; the folder, when it stays.
pub(super) fn remove_left<R: CommandRunner>(git: &Git<'_, R>, run: RunId) -> Result<Option<TemporaryLeft>, MergeError> {
  let path = temporary_path(git, run).map_err(|error| MergeError::from(error).at(MergeStep::TemporaryWorktree))?;
  Ok(remove(git, &path).err().map(|detail| TemporaryLeft { path, detail }))
}

/// `<git common dir>/orchester-merge-<run id>`.
fn temporary_path<R: CommandRunner>(git: &Git<'_, R>, run: RunId) -> Result<String, GitError> {
  let common = git.stdout(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
  Ok(format!("{common}/orchester-merge-{run}"))
}

fn merge_in<R: CommandRunner>(
  git: &Git<'_, R>,
  (path, hooks): (&str, &str),
  (base, head): (&str, &str),
  made: Made<'_>,
) -> Result<String, MergeError> {
  let temporary = filled(git, path, base).map_err(|error| MergeError::from(error).at(MergeStep::TemporaryWorktree))?;
  let at_merge = |error: GitError| MergeError::from(error).at(MergeStep::Merge);
  let hooks = format!("core.hooksPath={hooks}");
  let settings = ["-c", hooks.as_str(), "-c", "submodule.recurse=false"];
  let merge = ["merge", "--no-autostash", "--no-overwrite-ignore"];
  let output = match made {
    Made::Merge { title } => {
      temporary.output(&[&settings[..], &merge, &["--no-ff", "--no-edit", "-m", title, head]].concat())
    }
    Made::Squash { .. } => temporary.output(&[&settings[..], &merge, &["--squash", head]].concat()),
  }
  .map_err(at_merge)?;
  if !output.success() {
    return Err(stopped(&temporary, message(&output)).at(MergeStep::Merge));
  }
  if let Made::Squash { title, body } = made {
    let output = temporary
      .output(&[&settings[..], &["commit", "--quiet", "-m", title, "-m", body]].concat())
      .map_err(at_merge)?;
    if !output.success() {
      return Err(MergeError::Refused(message(&output)));
    }
  }
  temporary.stdout(&["rev-parse", "--verify", "HEAD"]).map_err(at_merge)
}

/// The temporary worktree at `path`, detached at `base` and filled with its files.
fn filled<'a, R: CommandRunner>(git: &Git<'a, R>, path: &'a str, base: &str) -> Result<Git<'a, R>, GitError> {
  // Twice `--force` takes over the place of one a killed merge left registered with its folder
  // gone, locked as `worktree add` keeps it while it runs.
  git.stdout(&[
    "worktree",
    "add",
    "--quiet",
    "--force",
    "--force",
    "--detach",
    "--no-checkout",
    path,
    base,
  ])?;
  let temporary = git.at(Path::new(path));
  is_temporary(&temporary, path)?;
  // Submodules are not set up in the temporary worktree: neither command may try to update them.
  temporary.stdout(&["-c", "submodule.recurse=false", "reset", "--hard", "--quiet"])?;
  Ok(temporary)
}

/// Refuses to write unless git in `temporary` works in the worktree at `path`, not in a repository
/// above it.
fn is_temporary<R: CommandRunner>(temporary: &Git<'_, R>, path: &str) -> Result<(), GitError> {
  let top = temporary.stdout(&["rev-parse", "--show-toplevel"])?;
  let same = match (Path::new(&top).canonicalize(), Path::new(path).canonicalize()) {
    (Ok(top), Ok(path)) => top == path,
    _ => false,
  };
  if same {
    Ok(())
  } else {
    Err(GitError::Unrecognized {
      expected: "the temporary worktree",
      detail: top,
    })
  }
}

/// Why the merge in `temporary` stopped: the files that conflict, else git's message `said`.
fn stopped<R: CommandRunner>(temporary: &Git<'_, R>, said: String) -> MergeError {
  match temporary.stdout(&["diff", "--name-only", "--diff-filter=U", "-z"]) {
    Ok(files) if !files.is_empty() => MergeError::Conflict {
      files: files
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(str::to_owned)
        .collect(),
    },
    Ok(_) => MergeError::Refused(said),
    Err(error) => error.into(),
  }
}

/// Removes the temporary worktree at `path` with what git keeps of it, when its folder is there;
/// why it stays, when it does. A folder git refuses to remove, or no longer knows as a worktree, is
/// Orchester's by its name and place, so it goes as a folder, as a run's forgotten worktree does
/// ([`crate::integration::forgotten`]).
fn remove<R: CommandRunner>(git: &Git<'_, R>, path: &str) -> Result<(), String> {
  if !Path::new(path).exists() {
    return Ok(());
  }
  let said = match git.output(&["worktree", "remove", "--force", "--force", path]) {
    Ok(output) if output.success() => return Ok(()),
    Ok(output) => message(&output),
    Err(error) => error.to_string(),
  };
  fs::remove_dir_all(path).map_err(|error| format!("{said} ({error})"))?;
  // Git still lists one whose folder went: `worktree remove` forgets it too.
  let listed = git
    .stdout(&["worktree", "list", "--porcelain", "-z"])
    .map_err(|error| error.to_string())?;
  let registered = crate::integration::parse_worktree_list(&listed)
    .iter()
    .any(|worktree| worktree.path == path);
  if registered {
    git
      .stdout(&["worktree", "remove", "--force", "--force", path])
      .map_err(|error| error.to_string())?;
  }
  Ok(())
}
