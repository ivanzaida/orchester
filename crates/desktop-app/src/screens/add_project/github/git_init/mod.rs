//! *Create a git repository here* (`Project setup · Create a git repository here` `OdlXb`): the GitHub block of a
//! folder that is not a git repository offers to make it one in place — `git init` and a first commit of the
//! folder's files or an empty one — with the progress, git's own words when it fails and Try again, which repeats
//! only what did not happen. A repository without a commit offers the first commit alone, and git that is not
//! installed says how to install it. The work is `orchester_workspace::repository::init`'s, run off the UI thread.

mod install;
mod view;
mod words;

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_workspace::{
  background,
  messages::Message,
  repository::init::{
    FirstCommit, InitError, InitStep, Initialized, LocalRepository, Survey, SurveyError, create_repository,
    make_first_commit, survey,
  },
};

pub(super) use self::view::{block, created_line};
use super::super::{Actions, Detected, Form};
use crate::services::Services;

/// What git sees of the chosen folder, when it holds Create back and the block offers to fix it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Shown {
  /// git is not installed (`KsWqe`).
  Missing,
  /// The folder is not a git repository (`vu5Ux`).
  NotRepository,
  /// A repository without a commit (`k20Zq`).
  NoCommits { branch: Option<String> },
}

impl Shown {
  pub(super) fn of(local: Option<&LocalRepository>) -> Option<Self> {
    match local? {
      LocalRepository::GitMissing => Some(Self::Missing),
      LocalRepository::NotRepository => Some(Self::NotRepository),
      LocalRepository::NoCommits { branch } => Some(Self::NoCommits { branch: branch.clone() }),
      LocalRepository::Commits { .. } | LocalRepository::Unread => None,
    }
  }
}

/// The repository or first commit to make, and in which folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Request {
  folder: PathBuf,
  first: FirstCommit,
}

/// What a first commit of a folder would hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Surveyed(PathBuf, Result<Survey, SurveyError>);

impl DevtoolsInspectable for Surveyed {}

/// A repository made, still without a commit: what was asked, and its branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Made(Request, String);

impl DevtoolsInspectable for Made {}

/// A first commit made in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Committed(PathBuf, Initialized);

impl DevtoolsInspectable for Committed {}

/// Why the repository or its first commit was not made in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Failed(PathBuf, InitError);

impl DevtoolsInspectable for Failed {}

/// What the person chose the first commit to hold; `None` until they choose, which takes the folder's files
/// when it has any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Choice(Option<FirstCommit>);

impl DevtoolsInspectable for Choice {}

/// How the last attempt to make a repository or its first commit ended, and in which folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::add_project) struct Attempt(Option<(PathBuf, Result<Initialized, InitError>)>);

impl DevtoolsInspectable for Attempt {}

impl Attempt {
  /// How the attempt in `folder` ended, if the last one was there.
  fn in_folder(&self, folder: &Path) -> Option<&Result<Initialized, InitError>> {
    self
      .0
      .as_ref()
      .filter(|(attempted, _)| attempted == folder)
      .map(|(_, result)| result)
  }
}

type SurveyAction = FutureAction<PathBuf, Surveyed, Message>;
type InitAction = FutureAction<Request, Made, Failed>;
type CommitAction = FutureAction<Request, Committed, Failed>;

/// The page's git work, asked for after its other futures in every render.
#[derive(Clone)]
pub(in crate::screens::add_project) struct GitActions {
  survey: SurveyAction,
  init: InitAction,
  commit: CommitAction,
}

impl GitActions {
  pub(in crate::screens::add_project) fn of(ctx: &mut Ctx) -> Self {
    let programs = Services::of(ctx).programs;
    let (quick, init, commit) = (programs.clone(), programs.clone(), programs);
    Self {
      survey: ctx.future_action(move |folder: PathBuf| {
        let programs = quick.clone();
        async move {
          background::run(move || {
            let surveyed = survey(programs.runner(), &folder);
            Surveyed(folder, surveyed)
          })
          .await
        }
      }),
      init: ctx.future_action(move |request: Request| {
        let (programs, folder) = (init.clone(), request.folder.clone());
        let work = move || create_repository(programs.git_work(), &request.folder).map(|branch| Made(request, branch));
        settle(folder, InitStep::Init, work)
      }),
      commit: ctx.future_action(move |request: Request| {
        let (programs, folder) = (commit.clone(), request.folder.clone());
        let work = move || {
          make_first_commit(programs.git_work(), &request.folder, request.first)
            .map(|made| Committed(request.folder, made))
        };
        settle(folder, InitStep::Commit, work)
      }),
    }
  }

  /// Whether git is making a repository or a first commit now.
  pub(in crate::screens::add_project) fn busy(&self) -> bool {
    self.init.state().get().is_pending() || self.commit.state().get().is_pending()
  }

  /// The finished survey of `folder`; `None` while it runs, before it ran, and for another folder.
  fn survey_of(&self, folder: &Path) -> Option<Result<Survey, SurveyError>> {
    self
      .survey
      .state()
      .get()
      .fulfilled_data()
      .filter(|Surveyed(surveyed, _)| surveyed == folder)
      .map(|Surveyed(_, result)| result.clone())
  }

  /// What git is doing in `folder` now.
  fn progress(&self, folder: &Path) -> Option<Progress> {
    if self.init.state().get().is_pending() {
      return Some(Progress::Initializing);
    }
    if !self.commit.state().get().is_pending() {
      return None;
    }
    let init = self.init.state().get();
    let made = init
      .fulfilled_data()
      .filter(|Made(request, _)| request.folder == folder)
      .map(|Made(_, branch)| branch.clone());
    Some(Progress::Committing { made })
  }
}

/// `work` off the UI thread, its failure kept with `folder`; work that did not finish failed at `step`.
async fn settle<T: Send + 'static>(
  folder: PathBuf,
  step: InitStep,
  work: impl FnOnce() -> Result<T, InitError> + Send + 'static,
) -> Result<T, Failed> {
  let kept = folder.clone();
  match background::run(work).await {
    Ok(result) => result.map_err(|error| Failed(kept, error)),
    Err(_) => Err(Failed(folder, InitError::Stopped(step))),
  }
}

/// What git is doing while the person waits (`Progress` `Lj60r`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Progress {
  Initializing,
  /// Making the first commit; `made` is the branch of the repository made just before it, if one was.
  Committing {
    made: Option<String>,
  },
}

/// Follows the page's git work: a folder detected without a repository or without a commit is surveyed; a made
/// repository gets its first commit; a first commit that ended, either way, reads the folder again.
pub(in crate::screens::add_project) fn follow(ctx: &mut Ctx, form: &Form, actions: &Actions) {
  let survey = actions.git.survey.clone();
  ctx.watch(&actions.detect.state(), move |state| {
    let Some(Detected(detected, local)) = state.fulfilled_data() else {
      return;
    };
    if matches!(
      local,
      LocalRepository::NotRepository | LocalRepository::NoCommits { .. }
    ) {
      survey.run(detected.folder().to_path_buf());
    }
  });
  let (commit, attempt) = (actions.git.commit.clone(), form.attempt.clone());
  ctx.watch(&actions.git.init.state(), move |state| match state.outcome() {
    Some(Ok(Made(request, _))) => commit.run(request.clone()),
    Some(Err(Failed(folder, error))) => attempt.set(Attempt(Some((folder.clone(), Err(error.clone()))))),
    None => {}
  });
  let (attempt, chosen, program, detect) = (
    form.attempt.clone(),
    form.folder.clone(),
    form.program.clone(),
    actions.clone(),
  );
  ctx.watch(&actions.git.commit.state(), move |state| {
    let (folder, result) = match state.outcome() {
      Some(Ok(Committed(folder, made))) => (folder.clone(), Ok(made.clone())),
      Some(Err(Failed(folder, error))) => (folder.clone(), Err(error.clone())),
      None => return,
    };
    attempt.set(Attempt(Some((folder.clone(), result))));
    // The repository is there now, with or without its commit: the block says what git sees.
    if chosen.get_untracked().0.as_deref() == Some(folder.as_path()) {
      detect.detect(folder, program.get_untracked().0);
    }
  });
}

/// The first commit the block takes: what the person chose, else the folder's files; an empty one for a folder
/// with nothing to add.
fn first_commit(form: &Form, surveyed: Option<&Result<Survey, SurveyError>>) -> FirstCommit {
  if matches!(surveyed, Some(Ok(survey)) if survey.files == 0) {
    return FirstCommit::Empty;
  }
  form.first.get().0.unwrap_or(FirstCommit::Files)
}

/// Starts making the repository and its first commit in `folder`, or only the commit in a repository without one.
fn start(form: &Form, actions: &Actions, (shown, folder): (&Shown, &Path), first: FirstCommit) {
  form.attempt.set(Attempt::default());
  let request = Request {
    folder: folder.to_path_buf(),
    first,
  };
  match shown {
    Shown::NoCommits { .. } => actions.git.commit.run(request),
    Shown::NotRepository | Shown::Missing => actions.git.init.run(request),
  }
}
