//! Bringing a merge into the user's checkout: a fast-forward of its branch, index and files to the
//! merge, which git guards. Git checks every path it would write before it writes one and refuses,
//! changing nothing, when that would overwrite or remove a file with uncommitted changes, an
//! untracked one or an ignored one (`--no-overwrite-ignore`); `--no-autostash` keeps
//! `merge.autoStash` from stashing the person's work out of the way instead. Should git still stop
//! part way, e.g. at a file another program holds open, the files it wrote are put back
//! ([`super::restore`]).

use super::{MergeError, restore::Touched};
use crate::{
  command::CommandRunner,
  integration::{
    GitError,
    git::{Git, message},
  },
};

/// Fast-forwards the checkout from `base`, its HEAD, to `commit`, a descendant of it.
pub(super) fn bring_in<R: CommandRunner>(git: &Git<'_, R>, (base, commit): (&str, &str)) -> Result<(), MergeError> {
  let touched = Touched::read(git, (base, commit))?;
  let output = git.output(&[
    "merge",
    "--ff-only",
    "--no-autostash",
    "--no-edit",
    "--no-overwrite-ignore",
    commit,
  ])?;
  if output.success() {
    return Ok(());
  }
  let files = in_the_way(&output.stderr);
  if !files.is_empty() {
    return Err(MergeError::UncommittedInTheWay { files });
  }
  let said = message(&output);
  if git.commit("HEAD")?.as_deref() != Some(base) {
    // The branch moved after all, by git or by someone else: what the files hold now is not this
    // merge's to put back.
    return Err(GitError::Failed(said).into());
  }
  touched.restore(git)?;
  Err(MergeError::Refused(said))
}

/// The paths git names when it refuses to update the checkout because that would lose what is in
/// them: one per line after a tab, under a line that ends like "would be overwritten by merge:",
/// "would be removed by merge:" or "would lose untracked files in them:" (a folder). Git offers
/// these lists in no other form; its messages are in English ([`crate::diff::git::git_command`]).
fn in_the_way(stderr: &str) -> Vec<String> {
  let mut files = Vec::new();
  let mut listing = false;
  for line in stderr.lines() {
    match line.strip_prefix('\t') {
      Some(file) if listing => files.push(file.to_owned()),
      Some(_) => {}
      None => listing = line.ends_with(" by merge:") || line.ends_with(" would lose untracked files in them:"),
    }
  }
  files
}
