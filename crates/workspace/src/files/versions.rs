//! The versions the code viewer's switch offers (`docs/design/code-viewer.md`, decision 2): the project's
//! checkout, the worktree of the run the page was opened from while it exists, the base branch while the
//! checkout has a branch, and the commit a link named; for each what names it (its branch, commit or run)
//! and what the file is there.

use std::path::Path;

use orchester_core::{
  command::CommandRunner,
  id::RunId,
  integration::checked_out_branch,
  project::TaskKey,
  revision::Revision,
  run::BranchName,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{
  FilePath, FileView, FilesError, Version,
  read::read_file,
  source::{Source, git_error, git_output, source},
};
use crate::tasks::changes::GIT;

/// One version the page can show, with what names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionChoice {
  pub version: Version,
  /// The checkout's branch, or the base branch; `None` on a detached checkout.
  pub branch: Option<BranchName>,
  /// The commit the base branch or a named commit is at.
  pub commit: Option<Revision>,
  /// What the file is in this version; `None` without a file.
  pub file: Option<FileFacts>,
}

/// The versions that exist, and the run the page was opened from, named whether or not its worktree still
/// exists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Versions {
  pub choices: Vec<VersionChoice>,
  pub run: Option<RunFacts>,
}

/// A run as the switch names it: `Run #12 · ORC-24`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
  pub number: u32,
  pub task: TaskKey,
}

/// What a file is in one version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFacts {
  Absent,
  Text {
    lines: usize,
    size: u64,
  },
  Binary {
    size: u64,
  },
  TooLarge {
    size: u64,
  },
  /// It is there but cannot be read, or is refused.
  Unreadable,
}

/// The versions that exist for the page that shows `asked`, opened from `run` when it was, with what
/// `path` is in each.
pub fn versions(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  (asked, run): (&Version, Option<RunId>),
  path: Option<&FilePath>,
) -> Result<Versions, FilesError> {
  let mut wanted = vec![Version::Checkout];
  wanted.extend(run.map(Version::Run));
  wanted.push(Version::Base);
  if let Version::Revision(_) = asked {
    wanted.push(asked.clone());
  }
  let mut choices = Vec::new();
  for version in wanted {
    let found = match source(runner, repo_root, &version) {
      Ok(found) => found,
      Err(FilesError::VersionGone(_)) => continue,
      Err(error) => return Err(error),
    };
    choices.push(choice(runner, repo_root, (version, &found), path)?);
  }
  // The switch still offers the worktree without the run's name: a database that cannot be read must not
  // hide the files.
  let run = run.and_then(|run| {
    run_facts(repo_root, run).unwrap_or_else(|error| {
      tracing::warn!(%error, %run, "cannot name the run whose worktree the code viewer shows");
      None
    })
  });
  Ok(Versions { choices, run })
}

fn choice(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  (version, found): (Version, &Source),
  path: Option<&FilePath>,
) -> Result<VersionChoice, FilesError> {
  let (branch, commit) = match found {
    Source::Folder(_) if version == Version::Checkout => (checkout_branch(runner, repo_root)?, None),
    Source::Folder(_) => (None, None),
    Source::Commit { repo, commit, branch } => (branch.clone(), Some(commit_of(runner, repo, commit)?)),
  };
  let file = path
    .map(|path| facts(read_file(runner, repo_root, &version, path)))
    .transpose()?;
  Ok(VersionChoice {
    version,
    branch,
    commit,
    file,
  })
}

/// What `read` found of a file; a failure to reach the version or git at all stays an error.
fn facts(read: Result<FileView, FilesError>) -> Result<FileFacts, FilesError> {
  match read {
    Ok(FileView::Text(text)) => Ok(FileFacts::Text {
      lines: text.lines().len(),
      size: text.size(),
    }),
    Ok(FileView::Binary { size }) => Ok(FileFacts::Binary { size }),
    Ok(FileView::TooLarge { size }) => Ok(FileFacts::TooLarge { size }),
    Err(FilesError::Missing(_)) => Ok(FileFacts::Absent),
    Err(FilesError::Path(_) | FilesError::Folder(_) | FilesError::Unreadable { .. }) => Ok(FileFacts::Unreadable),
    Err(error) => Err(error),
  }
}

fn checkout_branch(runner: &dyn CommandRunner, repo_root: &Path) -> Result<Option<BranchName>, FilesError> {
  checked_out_branch(&runner, Path::new(GIT), repo_root).map_err(git_error)
}

/// The commit `named` is at.
fn commit_of(runner: &dyn CommandRunner, repo: &Path, named: &str) -> Result<Revision, FilesError> {
  let output = git_output(runner, repo, &["rev-parse", "--verify", named])?;
  Revision::new(output.trim()).map_err(|error| FilesError::Git(error.to_string()))
}

fn store(error: impl std::fmt::Display) -> FilesError {
  FilesError::Store(error.to_string())
}

/// The run's number and its task's key, from the project's database; `None` for a run it does not hold.
fn run_facts(repo_root: &Path, run: RunId) -> Result<Option<RunFacts>, FilesError> {
  let db = ProjectDb::open(repo_root).map_err(store)?;
  let Some(found) = db.run(run).map_err(store)? else {
    return Ok(None);
  };
  let Some(task) = db.task(found.task()).map_err(store)? else {
    return Ok(None);
  };
  Ok(Some(RunFacts {
    number: found.number().get(),
    task: task.key().clone(),
  }))
}
