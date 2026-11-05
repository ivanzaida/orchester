//! Bringing a merge into the user's checkout: a fast-forward of its branch, index and files to the
//! merge, which git guards. Git checks every path it would write before it writes one and refuses,
//! changing nothing, when that would overwrite or remove a file with uncommitted changes, an
//! untracked one or an ignored one (`--no-overwrite-ignore`); `--no-autostash` keeps
//! `merge.autoStash` from stashing the person's work out of the way instead. Should git still stop
//! part way, e.g. at a file another program holds open, or stop answering and be stopped, the files
//! it wrote are put back ([`super::restore`]). What it would change is kept beside the checkout
//! while it runs ([`super::left`]), and stays there while files it wrote could not be put back,
//! whatever stopped the put-back.

use super::{MergeError, MergeStep, left::Record, restore::Touched};
use crate::{
  command::CommandRunner,
  id::RunId,
  integration::{
    GitError,
    git::{Git, message},
  },
};

/// Fast-forwards the checkout from `base`, its HEAD, to `commit`, a descendant of it, the merge of `run`; a module
/// delivery (no run) keeps no record of it.
pub(super) fn bring_in<R: CommandRunner>(
  git: &Git<'_, R>,
  run: Option<RunId>,
  (base, commit): (&str, &str),
) -> Result<(), MergeError> {
  let touched = Touched::read(git, (base, commit)).map_err(|error| MergeError::from(error).at(MergeStep::BringIn))?;
  let Some(run) = run else {
    return fast_forward(git, &touched, (base, commit));
  };
  let record = Record::save(git, run, &touched)?;
  let brought = fast_forward(git, &touched, (base, commit));
  let left_to_put_back = matches!(
    brought,
    Err(
      MergeError::LeftChanged { .. }
        | MergeError::Unanswered {
          step: MergeStep::PutBack,
          ..
        }
    )
  );
  if !left_to_put_back {
    // A record that stays names a base the checkout is no longer at, or one it holds again everywhere: the next merge
    // drops it. How the bring-in went is what to tell, so losing this error loses nothing.
    let _ = record.remove();
  }
  brought
}

fn fast_forward<R: CommandRunner>(
  git: &Git<'_, R>,
  touched: &Touched,
  (base, commit): (&str, &str),
) -> Result<(), MergeError> {
  let output = git.output(&[
    "merge",
    "--ff-only",
    "--no-autostash",
    "--no-edit",
    "--no-overwrite-ignore",
    commit,
  ]);
  let output = match output {
    Ok(output) => output,
    Err(GitError::Unanswered { command, after }) => {
      return stopped_answering(git, touched, (base, commit), (command, after));
    }
    Err(error) => return Err(error.into()),
  };
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
  touched
    .restore(git)
    .map_err(|error| not_put_back(git, touched, error.at(MergeStep::PutBack)))?;
  Err(MergeError::Refused(said))
}

/// The fast-forward stopped answering and was stopped with everything it started. Git moves the branch last: one at
/// the merge holds it, and only the `post-merge` hook did not finish; one still at the base gets back what git wrote.
fn stopped_answering<R: CommandRunner>(
  git: &Git<'_, R>,
  touched: &Touched,
  (base, commit): (&str, &str),
  (command, after): (String, std::time::Duration),
) -> Result<(), MergeError> {
  let unanswered = MergeError::Unanswered {
    step: MergeStep::BringIn,
    command,
    after,
  };
  let at = |error: MergeError| not_put_back(git, touched, error.at(MergeStep::PutBack));
  let head = git.commit("HEAD").map_err(|error| at(error.into()))?;
  if head.as_deref() == Some(commit) {
    return Ok(());
  }
  if head.as_deref() != Some(base) {
    return Err(unanswered);
  }
  touched.restore(git).map_err(at)?;
  Err(unanswered)
}

/// What a put-back that did not finish, with `error`, left: as it says when git stopped answering or files could not be
/// put back; otherwise the paths that do not hold the base now, as [`MergeError::LeftChanged`], or every path git may
/// have written when even that cannot be read. Only a checkout that holds the base everywhere again is told `error`.
fn not_put_back<R: CommandRunner>(git: &Git<'_, R>, touched: &Touched, error: MergeError) -> MergeError {
  if matches!(error, MergeError::LeftChanged { .. } | MergeError::Unanswered { .. }) {
    return error;
  }
  match touched.still_changed(git) {
    Ok(files) if files.is_empty() => error,
    Ok(files) => MergeError::LeftChanged {
      files,
      detail: error.to_string(),
    },
    Err(unread) => MergeError::LeftChanged {
      files: touched.paths(),
      detail: format!("{error}; which of these files still hold the merge's version is unknown: {unread}"),
    },
  }
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
