//! *Create pull request* on a task's Ready card (`docs/design/orchester-workspace.md`, "GitHub", rules 3 to 7):
//! whether the task's latest succeeded run can have one ([`readiness_in`]), and creating it ([`create_in`]),
//! which pushes the run's branch to the link's remote with git's own credentials, opens the pull request from
//! that branch into the one the run started from through `gh` (a draft unless the person asks otherwise), then
//! syncs the pull requests, so the Pull requests list links it to the task and run as it does any branch a run
//! made. Every function blocks on SQLite, git or `gh`; the UI calls them off the UI thread, and creating runs on
//! the project's shared operations (`integration_jobs`).

use std::path::Path;

use orchester_core::{
  connection::{GitHubHost, GitHubLogin, GitHubState, check_github},
  github::GhCli,
  id::RunId,
  pull_request::{
    CreateFailure, NewPullRequest, OpenFailure, OpenedPullRequest, PullRequestState, Push, PushFailure, SyncScope,
    open_pull_request, task_link, task_pull_request,
  },
  repository::{RemoteName, RepositoryError, RepositoryLink, read_git_dir_remotes},
  run::{BranchName, Run, RunStatus},
  store::{ConnectionStore, PullRequestStore, RepositoryStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{InboxError, github_connection, sync_scope_in};
use crate::{
  messages::{self, Message},
  tasks::detail::run_store_error,
};

/// The `git` program, looked up on the `PATH` like the runner does.
const GIT: &str = "git";

/// Why *Create pull request* cannot be used for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
  /// The folder has a GitHub remote but the project is not linked to it, or `gh` of the link's connection
  /// cannot be used: not found, not signed in, not answering as the GitHub CLI.
  NoConnection,
  /// The folder has no `origin` remote on GitHub.
  NoGitHubRemote,
  /// `gh` acts as `active`, but the project expects `expected`. Orchester never switches accounts.
  WrongAccount { active: GitHubLogin, expected: GitHubLogin },
  /// `gh` is signed in as `account` on `host` without the `repo` scope.
  MissingScope { account: GitHubLogin, host: GitHubHost },
  /// The run did not record the branch it started from, which the pull request would go into.
  NoBaseBranch,
}

impl Unavailable {
  /// What `state`, a check of `gh` on `host`, means for creating a pull request; `None` when it can be created.
  fn of(state: GitHubState, host: &GitHubHost) -> Option<Self> {
    match state {
      GitHubState::SignedIn { .. } => None,
      GitHubState::WrongAccount { active, expected } => Some(Self::WrongAccount { active, expected }),
      GitHubState::MissingScope { account } => Some(Self::MissingScope {
        account,
        host: host.clone(),
      }),
      _ => Some(Self::NoConnection),
    }
  }
}

/// Whether a run can have a pull request created now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
  Ready,
  /// The run's branch has open pull request `number`, as the last sync read it: Review PR is offered instead.
  Open {
    number: u64,
  },
  Unavailable(Unavailable),
}

/// Why it could not be read whether a pull request can be created.
#[derive(Debug, Error)]
pub enum ReadinessError {
  #[error(transparent)]
  Inbox(#[from] InboxError),
  #[error(transparent)]
  Remotes(#[from] RepositoryError),
}

impl From<StoreError> for ReadinessError {
  fn from(error: StoreError) -> Self {
    Self::Inbox(InboxError::Store(error))
  }
}

impl ReadinessError {
  pub fn message(&self) -> Message {
    match self {
      Self::Inbox(error) => error.message(),
      Self::Remotes(_) => Message::new("errors.githubUnavailable"),
    }
  }
}

/// Whether run `run` of the project at `repo_root` can have a pull request created: the folder's GitHub remote,
/// the link, the pull requests the last sync stored, the branch the run started from, and a fresh check of the
/// link's `gh` with `programs`.
pub fn readiness_in(repo_root: &Path, programs: &Programs, run: RunId) -> Result<Readiness, ReadinessError> {
  let db = ProjectDb::open(repo_root)?;
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(StoreError::RunNotFound(run))?;
  let Some(link) = db.repository_link()? else {
    return Ok(Readiness::Unavailable(unlinked(repo_root, programs)?));
  };
  if let Some(number) = open_for(&db, &link, run.branch())? {
    return Ok(Readiness::Open { number });
  }
  if run.base_branch().is_none() {
    return Ok(Readiness::Unavailable(Unavailable::NoBaseBranch));
  }
  let connection = github_connection(&db, link.connection())?;
  let check = check_github(
    connection.executable(),
    connection.host(),
    &programs.runner(),
    Timestamp::now(),
  );
  let state = GitHubState::of(Some(&check.result), Some(link.expected_login()));
  Ok(Unavailable::of(state, link.repo().host()).map_or(Readiness::Ready, Readiness::Unavailable))
}

/// Why an unlinked project cannot create one: its folder has a GitHub remote the project is not linked to, or none.
/// Git reads them from the repository's metadata without running in the checkout, which the task page's reads of a
/// local integration hold at the same time.
fn unlinked(repo_root: &Path, programs: &Programs) -> Result<Unavailable, RepositoryError> {
  let remotes = read_git_dir_remotes(&programs.runner(), Path::new(GIT), &repo_root.join(".git"))?;
  let on_github = remotes
    .iter()
    .any(|remote| remote.name().as_str() == RemoteName::ORIGIN && remote.github_repo().is_some());
  Ok(if on_github {
    Unavailable::NoConnection
  } else {
    Unavailable::NoGitHubRemote
  })
}

/// The open pull request of `branch` the last sync of `link`'s repository stored, if any.
fn open_for(db: &ProjectDb, link: &RepositoryLink, branch: &BranchName) -> Result<Option<u64>, StoreError> {
  Ok(db.pull_requests(link.repo())?.and_then(|synced| {
    synced
      .pull_requests
      .iter()
      .find(|pull_request| {
        pull_request.head() == branch
          && !pull_request.is_cross_repository()
          && matches!(pull_request.state(), PullRequestState::Open | PullRequestState::Draft)
      })
      .map(|pull_request| pull_request.number())
  }))
}

/// A pull request GitHub opened for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
  pub pull_request: OpenedPullRequest,
  pub draft: bool,
}

/// Why no pull request was created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateProblem {
  Unavailable(Unavailable),
  /// The run's branch has an open pull request already: `number`, when known.
  AlreadyOpen {
    number: Option<u64>,
  },
  /// The run is not its task's latest run, or did not succeed.
  NotReady,
  /// Git did not push the branch to `remote` on `host`; nothing was asked of GitHub.
  Push {
    failure: PushFailure,
    remote: RemoteName,
    host: GitHubHost,
  },
  /// The branch has no commits the base lacks.
  NoCommits,
  /// The branch was pushed, but GitHub opened no pull request.
  GitHub(RepositoryError),
  /// The project database could not be read or written.
  Store(Message),
}

impl From<StoreError> for CreateProblem {
  fn from(error: StoreError) -> Self {
    tracing::error!(%error, "cannot read or record a pull request's creation");
    Self::Store(messages::store_error(&error))
  }
}

/// Creates a pull request from run `run`'s branch of the project at `repo_root` with `programs`, a draft when
/// `draft`, then syncs the pull requests.
pub fn create_in(repo_root: &Path, programs: &Programs, (run, draft): (RunId, bool)) -> Result<Created, CreateProblem> {
  let outcome = open(repo_root, programs, (run, draft));
  if matches!(&outcome, Ok(_) | Err(CreateProblem::AlreadyOpen { .. })) {
    // The pull request stands either way; the Pull requests page says why a sync failed.
    if let Err(error) = sync_scope_in(repo_root, programs, SyncScope::OpenAndClosed) {
      tracing::warn!(%error, "cannot sync the pull requests after creating one");
    }
  }
  outcome
}

fn open(repo_root: &Path, programs: &Programs, (run, draft): (RunId, bool)) -> Result<Created, CreateProblem> {
  let mut db = ProjectDb::open(repo_root)?;
  let run = latest_succeeded(&db, run)?;
  let Some(base) = run.base_branch().cloned() else {
    return Err(CreateProblem::Unavailable(Unavailable::NoBaseBranch));
  };
  let Some(link) = db.repository_link()? else {
    let unavailable = unlinked(repo_root, programs).map_err(CreateProblem::GitHub)?;
    return Err(CreateProblem::Unavailable(unavailable));
  };
  if let Some(number) = open_for(&db, &link, run.branch())? {
    return Err(CreateProblem::AlreadyOpen { number: Some(number) });
  }
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  let (title, body) = task_pull_request(task.key(), task.details(), &task_link(db.project().id(), task.key()));
  let new = NewPullRequest {
    base,
    head: run.branch().clone(),
    title,
    body,
    draft,
  };
  let mut connection =
    github_connection(&db, link.connection()).map_err(|error| CreateProblem::Store(error.message()))?;
  let push = Push {
    runner: programs.git_work(),
    git: Path::new(GIT),
    repo_root,
  };
  let attempt = open_pull_request(
    &GhCli::new(programs.runner(), &connection),
    push,
    (&link, &new),
    Timestamp::now(),
  );
  connection.record_check(attempt.check);
  db.save_connection(&connection)?;
  attempt
    .outcome
    .map(|pull_request| Created { pull_request, draft })
    .map_err(|failure| problem(failure, &link))
}

/// `run` as stored, if it succeeded and is its task's latest run.
fn latest_succeeded(db: &ProjectDb, run: RunId) -> Result<Run, CreateProblem> {
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(StoreError::RunNotFound(run))?;
  let latest = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    .filter_map(|listed| {
      listed.as_ref().map_or_else(
        |bad| bad.number.map(|number| number.get()),
        |run| Some(run.number().get()),
      )
    })
    .max();
  if run.status() != &RunStatus::Succeeded || latest != Some(run.number().get()) {
    return Err(CreateProblem::NotReady);
  }
  Ok(run)
}

fn problem(failure: OpenFailure, link: &RepositoryLink) -> CreateProblem {
  let host = link.repo().host();
  match failure {
    OpenFailure::Account(state) => {
      CreateProblem::Unavailable(Unavailable::of(state, host).unwrap_or(Unavailable::NoConnection))
    }
    OpenFailure::Push(failure) => CreateProblem::Push {
      failure,
      remote: link.remote().clone(),
      host: host.clone(),
    },
    OpenFailure::Create(CreateFailure::AlreadyExists { number }) => CreateProblem::AlreadyOpen { number },
    OpenFailure::Create(CreateFailure::NoCommits) => CreateProblem::NoCommits,
    OpenFailure::Create(CreateFailure::Repository(error)) => CreateProblem::GitHub(error),
  }
}
