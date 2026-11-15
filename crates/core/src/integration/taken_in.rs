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

/// The tree of `start` merged with `tip`, what the branch took in, as git would merge them: the point the run's own
/// work is read from. `None` when they conflict, as then no such point holds.
pub fn without_taken_in(
  runner: &impl CommandRunner,
  (program, repo): (&Path, &Path),
  (start, tip): (&Revision, &Revision),
) -> Result<Option<Revision>, GitError> {
  let git = Git::new(runner, program, repo);
  let output = git.output(&["merge-tree", "--write-tree", start.as_str(), tip.as_str()])?;
  if !output.success() {
    return Ok(None);
  }
  let tree = output.stdout.lines().next().unwrap_or_default().trim().to_owned();
  Revision::new(&tree).map(Some).map_err(|_| GitError::Unrecognized {
    expected: "a tree",
    detail: tree,
  })
}

/// The second parent of the newest merge on the first-parent line of `range` whose merged-in side is outside it.
fn newest_merged_in<R: CommandRunner>(git: &Git<'_, R>, range: &[&str]) -> Result<Option<Revision>, GitError> {
  let mut args = vec!["rev-list", "--first-parent", "--merges", "--parents"];
  args.extend(range);
  let merges = git.stdout(&args)?;
  let newest = merges
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(2).map(str::to_owned));
  Ok(newest.and_then(|parent| Revision::new(&parent).ok()))
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
