//! What a run's branch took in from branches other than the one it merges into: commits that are neither on its own
//! first-parent line nor its target's (an agent that merged `main` into a module's run, say). Merging the run brings
//! them along, so the run's diff is that large; [`without_taken_in`] gives the point its agent's own work is read from.

use std::path::Path;

use super::{GitError, RunWork, git::Git};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// What a run's branch took in from other branches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenIn {
  /// How many such commits it holds.
  pub commits: usize,
  /// A branch they came from, when a local branch holds the newest of them: the project's `main` or `master` first.
  pub from: Option<BranchName>,
  /// The newest commit it merged in from elsewhere.
  pub tip: Revision,
}

/// What `work`'s branch, its own work starting at `start` and ending at `head`, took in from branches other than its
/// target, in the repository at `repo`; `None` when it took in nothing of the kind.
pub fn taken_in(
  runner: &impl CommandRunner,
  (program, repo): (&Path, &Path),
  (work, start, head): (&RunWork, &Revision, &Revision),
) -> Result<Option<TakenIn>, GitError> {
  let git = Git::new(runner, program, repo);
  let target = match &work.into {
    Some(into) => git
      .stdout(&[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("refs/heads/{into}^{{commit}}"),
      ])
      .ok(),
    None => None,
  };
  let mut range = vec![head.as_str().to_owned(), format!("^{start}")];
  range.extend(target.iter().map(|target| format!("^{target}")));
  let range: Vec<&str> = range.iter().map(String::as_str).collect();
  let count = |first_parent: bool| -> Result<usize, GitError> {
    let mut args = vec!["rev-list", "--count"];
    if first_parent {
      args.push("--first-parent");
    }
    args.extend(&range);
    let counted = git.stdout(&args)?;
    counted.parse().map_err(|_| GitError::Unrecognized {
      expected: "a count",
      detail: counted,
    })
  };
  let commits = count(false)?.saturating_sub(count(true)?);
  if commits == 0 {
    return Ok(None);
  }
  let Some(tip) = newest_merged_in(&git, &range)? else {
    return Ok(None);
  };
  let from = source_branch(&git, &tip, (&work.branch, work.into.as_ref()))?;
  Ok(Some(TakenIn { commits, from, tip }))
}

/// The tree of `start` merged with what the branch took in (`taken.tip`), as git merges them, each file they conflict
/// on as `start` has it, so the run's own work in it is never hidden (the file shows whole, what was taken in with
/// it): the point the run's own work is read from. Built in an index
/// of its own, which is removed after; the repository's index and checkout stay as they are. `None` when git printed
/// no tree.
pub fn without_taken_in(
  runner: &impl CommandRunner,
  (program, repo): (&Path, &Path),
  (start, taken): (&Revision, &TakenIn),
) -> Result<Option<Revision>, GitError> {
  let git = Git::new(runner, program, repo);
  let output = git.output(&[
    "merge-tree",
    "--write-tree",
    "--name-only",
    start.as_str(),
    taken.tip.as_str(),
  ])?;
  let mut lines = output.stdout.lines();
  let Some(tree) = lines.next().map(str::trim).and_then(|tree| Revision::new(tree).ok()) else {
    return Ok(None);
  };
  if output.success() {
    return Ok(Some(tree));
  }
  // Conflicts: their names follow the tree, up to an empty line.
  let conflicted: Vec<&str> = lines.map(str::trim).take_while(|path| !path.is_empty()).collect();
  settled(&git, (&tree, start), &conflicted).map(Some)
}

/// `tree` with each of `paths` as `start` has it (none where `start` has none), written in an index of its own.
fn settled<R: CommandRunner>(
  git: &Git<'_, R>,
  (tree, start): (&Revision, &Revision),
  paths: &[&str],
) -> Result<Revision, GitError> {
  let index = git.stdout(&["rev-parse", "--git-path", "orchester-taken-in.index"])?;
  let index = git.dir().join(index);
  let built = (|| {
    git.stdout_in_index(&index, &["read-tree", tree.as_str()])?;
    for path in paths {
      let entry = git.stdout(&["ls-tree", start.as_str(), "--", path])?;
      match entry
        .split_once('\t')
        .map(|(head, _)| head.split_whitespace().collect::<Vec<_>>())
      {
        Some(parts) if parts.len() == 3 => {
          let info = format!("{},{},{path}", parts[0], parts[2]);
          git.stdout_in_index(&index, &["update-index", "--add", "--cacheinfo", &info])?;
        }
        _ => {
          git.stdout_in_index(&index, &["update-index", "--force-remove", "--", path])?;
        }
      }
    }
    git.stdout_in_index(&index, &["write-tree"])
  })();
  // The index is this call's alone and is removed whatever came of it. One that could not be removed is a file in the
  // repository's git directory that nothing reads, and the next call starts it again with read-tree: losing that
  // error loses nothing.
  let _left = std::fs::remove_file(&index);
  let written = built?;
  Revision::new(&written).map_err(|_| GitError::Unrecognized {
    expected: "a tree",
    detail: written,
  })
}

/// The second parent of the newest merge on the first-parent line of `range`: the side it took in.
fn newest_merged_in<R: CommandRunner>(git: &Git<'_, R>, range: &[&str]) -> Result<Option<Revision>, GitError> {
  let mut args = vec!["rev-list", "--first-parent", "--merges", "--parents"];
  args.extend(range);
  let merges = git.stdout(&args)?;
  let Some(newest) = merges.lines().next() else {
    return Ok(None);
  };
  Ok(newest.split_whitespace().nth(2).and_then(|tip| Revision::new(tip).ok()))
}

/// A local branch that holds `tip`, other than the run's and its target: `main` or `master` when one does.
fn source_branch<R: CommandRunner>(
  git: &Git<'_, R>,
  tip: &Revision,
  (own, target): (&BranchName, Option<&BranchName>),
) -> Result<Option<BranchName>, GitError> {
  let listed = git.stdout(&[
    "for-each-ref",
    "--format=%(refname:short)",
    "--contains",
    tip.as_str(),
    "refs/heads",
  ])?;
  let names: Vec<&str> = listed
    .lines()
    .map(str::trim)
    .filter(|name| *name != own.as_str() && Some(*name) != target.map(BranchName::as_str))
    .collect();
  let chosen = ["main", "master"]
    .into_iter()
    .find(|wanted| names.contains(wanted))
    .or_else(|| names.first().copied());
  Ok(chosen.and_then(|name| BranchName::new(name).ok()))
}
