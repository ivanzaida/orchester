//! *Create pull request* beside *Merge into main* (`docs/design/orchester-workspace.md`, "GitHub", rules 3 to 7):
//! the button (`git-pull-request`) with its *Open as draft* switch, on until the person turns it off (06.A
//! `mtPZd`); under way while the run's branch is pushed and the pull request opened, on the project's shared
//! operations; unavailable with why when the project has no usable GitHub connection, the folder no GitHub remote,
//! `gh` acts as another account or lacks the `repo` scope, the run recorded no base branch, or the branch has an
//! open pull request already. Once opened, the card says so with its number and *Review PR* opens it; a refused
//! push or GitHub refusing to open it says why in place, with the recovery the rules name (06.B `YDKJS`) and *Try
//! again*. When the pull request would include commits of the local base its remote lacks, nothing is pushed and
//! the person is asked in place ([`super::unpushed`]).

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  command::CommandError,
  pull_request::{PushFailure, SETUP_GIT},
  repository::RepositoryError,
};
use orchester_workspace::{
  background,
  messages::Message,
  pull_requests::create::{CreateProblem, Created, Readiness, Unavailable, UnpushedBase, readiness_in},
  tasks::{
    detail::RunView,
    integration_jobs::{PullRequestArgs, PullRequestJob},
  },
};

use super::{
  super::TaskContext,
  unpushed::{self, Choices, Refused, Unpushed},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    command_row::CommandRow,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    toggle::{Toggle, ToggleKind},
  },
};

/// Whether the run can have a pull request as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Option<Readiness>);

impl DevtoolsInspectable for Read {}

/// What the Ready card knows of creating a pull request: whether it can (`None` while it is read), and the *Open
/// as draft* switch, which the card keeps for the run.
#[derive(Clone)]
pub struct PullRequestRead {
  pub readiness: Option<Result<Readiness, Message>>,
  pub draft: Signal<bool>,
  /// The run whose question about the base's unpushed commits window automation tried to answer.
  pub person_only: Signal<Option<Refused>>,
}

/// Whether the task's latest run can have a pull request, read off the UI thread for each run and each time the
/// task is read again, once the page's other reads of the checkout `settled`; `None` while it is read, and
/// without a run.
pub fn read(ctx: &mut Ctx, task: &TaskContext, settled: bool) -> Option<Result<Readiness, Message>> {
  let programs = Services::of(ctx).programs;
  let run = task.detail.run.as_ref().map(RunView::id).filter(|_| settled);
  let deps = (
    task.project.open.repo_root.clone(),
    run,
    task.project.feed.revision.get(),
  );
  let load = ctx.future(deps, move |(root, run, _)| {
    let programs = programs.clone();
    async move {
      let Some(run) = run else {
        return Ok(Read(None));
      };
      background::run(move || readiness_in(&root, &programs, run))
        .await?
        .map(|readiness| Read(Some(readiness)))
        .map_err(|error| {
          tracing::warn!(%error, "cannot read whether the run can have a pull request");
          error.message()
        })
    }
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(Read(readiness)), None) => readiness.map(Ok),
    (None, None) => None,
  }
}

/// The card's parts for creating a pull request.
pub struct Parts {
  /// The button and its switch, for the actions row after *Merge into main*.
  pub actions: Vec<Element>,
  /// Why the button cannot be used now.
  pub reason: Option<Arc<str>>,
  /// What came of creating one: it was opened, or why not.
  pub outcome: Option<Element>,
  /// The run's open pull request, which *Review PR* opens.
  pub review: Option<NonZeroU64>,
}

pub fn parts(ctx: &mut Ctx, task: &TaskContext, run: &RunView, read: &PullRequestRead) -> Parts {
  let job = task.jobs.run(run.id());
  if let PullRequestJob::Created(created) = &job.pull_request {
    return Parts {
      actions: Vec::new(),
      reason: None,
      outcome: Some(opened(ctx, created)),
      review: NonZeroU64::new(created.pull_request.number),
    };
  }
  let open = match (&job.pull_request, &read.readiness) {
    (PullRequestJob::Failed(CreateProblem::AlreadyOpen { number: Some(number) }), _)
    | (_, Some(Ok(Readiness::Open { number }))) => NonZeroU64::new(*number),
    _ => None,
  };
  let start = start(ctx, task, run, (read, UnpushedBase::Refuse));
  let state = if let Some(number) = open {
    State::Unavailable(ctx.t_args("decisions.ready.pullRequest.open", [("number", number.to_string())]))
  } else if job.pull_request == PullRequestJob::Running {
    State::UnderWay
  } else if job.busy() {
    State::Waits(ctx.t("decisions.ready.pullRequest.waits"))
  } else {
    match &read.readiness {
      None => State::Reading,
      Some(Err(error)) => State::Unavailable(message_text(ctx, error)),
      Some(Ok(Readiness::Unavailable(unavailable))) => State::Unavailable(unavailable_reason(ctx, unavailable)),
      Some(Ok(_)) => State::Ready,
    }
  };
  let outcome = match &job.pull_request {
    PullRequestJob::Failed(CreateProblem::BaseNotPushed { commits, base, remote }) if open.is_none() => {
      let unpushed = Unpushed {
        commits: *commits,
        base,
        remote,
      };
      let dismiss = task.project.feed.jobs.of(run.id());
      let choices = Choices {
        anyway: (state == State::Ready).then(|| start_anyway(ctx, task, run, read)),
        dismiss: Arc::new(move || dismiss.dismiss_unpushed_base()),
        person_only: read.person_only.clone(),
        run: run.id(),
      };
      Some(unpushed::question(ctx, &unpushed, choices))
    }
    PullRequestJob::Failed(problem) if open.is_none() => {
      let again = (state == State::Ready).then(|| start.clone());
      Some(failure(ctx, run, problem, again))
    }
    _ => None,
  };
  Parts {
    actions: vec![button(ctx, &state, start), draft_switch(ctx, &state, &read.draft)],
    reason: state.reason(),
    outcome,
    review: open,
  }
}

/// Where the button stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
  /// Whether it can be used is read.
  Reading,
  Ready,
  /// The branch is pushed and the pull request opened now.
  UnderWay,
  /// Another operation of the run works; it waits, and says so.
  Waits(Arc<str>),
  Unavailable(Arc<str>),
}

impl State {
  fn reason(&self) -> Option<Arc<str>> {
    match self {
      Self::Waits(reason) | Self::Unavailable(reason) => Some(reason.clone()),
      Self::Reading | Self::Ready | Self::UnderWay => None,
    }
  }
}

fn button(ctx: &Ctx, state: &State, start: Start) -> Element {
  let button = ActionButton::new(
    "task-create-pull-request",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.pullRequest.create"),
  )
  .icon(Icon::GitPullRequest);
  match state {
    State::Ready => button.on_click(move || start()),
    State::UnderWay => button.loading(),
    State::Reading | State::Waits(_) => button.disabled(),
    State::Unavailable(reason) => button.unavailable(reason.clone()),
  }
  .into()
}

/// *Open as draft*, on until the person turns it off; fixed while the button cannot be pressed.
fn draft_switch(ctx: &Ctx, state: &State, draft: &Signal<bool>) -> Element {
  let switch = Toggle::new(
    "task-pull-request-draft",
    ToggleKind::Switch,
    draft.get(),
    ctx.t("decisions.ready.pullRequest.draft"),
  );
  if *state != State::Ready {
    return switch.into();
  }
  let draft = draft.clone();
  switch.on_change(move |on| draft.set(on)).into()
}

pub(super) type Start = Arc<dyn Fn() + Send + Sync>;

/// Creates it including the base's unpushed commits, once a person read that it would.
fn start_anyway(ctx: &mut Ctx, task: &TaskContext, run: &RunView, read: &PullRequestRead) -> Start {
  start(ctx, task, run, (read, UnpushedBase::Include))
}

/// Starts creating the run's pull request with the switch's choice; `unpushed` says whether it stops when the base
/// has commits its remote lacks.
fn start(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (read, unpushed): (&PullRequestRead, UnpushedBase),
) -> Start {
  let services = Services::of(ctx);
  let job = task.project.feed.jobs.of(run.id());
  let feed = task.project.feed.clone();
  let (draft, person_only) = (read.draft.clone(), read.person_only.clone());
  let args = PullRequestArgs {
    open: task.project.open.clone(),
    task: task.detail.task.id(),
    key: task.detail.task.key().to_string(),
    number: run.run.number().get(),
    run: run.id(),
    draft: true,
    unpushed,
    programs: services.programs,
    activity: services.activity,
    changed: Arc::new(move || feed.saved()),
  };
  Arc::new(move || {
    // Each press asks anew: a refusal of window automation belongs to the question it answered.
    person_only.set(None);
    let args = PullRequestArgs {
      draft: draft.get_untracked(),
      ..args.clone()
    };
    job.start_pull_request(args);
  })
}

/// Why the button cannot be used.
fn unavailable_reason(ctx: &Ctx, unavailable: &Unavailable) -> Arc<str> {
  match unavailable {
    Unavailable::NoConnection => ctx.t("decisions.ready.pullRequest.noConnection"),
    Unavailable::NoGitHubRemote => ctx.t("decisions.ready.pullRequest.noRemote"),
    Unavailable::WrongAccount { active, expected } => ctx.t_args(
      "decisions.ready.pullRequest.wrongAccount",
      [("active", active.as_str()), ("expected", expected.as_str())],
    ),
    Unavailable::MissingScope { host, .. } => {
      ctx.t_args("decisions.ready.pullRequest.missingScope", [("host", host.as_str())])
    }
    Unavailable::NoBaseBranch => ctx.t("decisions.ready.pullRequest.noBranch"),
  }
}

/// "Pull request #143 opened as a draft." under the actions.
fn opened(ctx: &Ctx, created: &Created) -> Element {
  let key = if created.draft {
    "decisions.ready.pullRequest.openedDraft"
  } else {
    "decisions.ready.pullRequest.opened"
  };
  let number = created.pull_request.number.to_string();
  Row::new()
    .id("task-pull-request-opened")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GitPullRequest, IconSize::Sm, PaletteColor::Success).flex_shrink(0.0))
    .child(
      Text::new(&ctx.t_args(key, [("number", number)]))
        .id("task-pull-request-opened-text")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// Why no pull request was opened, with the recovery the GitHub rules name and *Try again* (`again`, `None` while
/// it cannot be pressed).
fn failure(ctx: &mut Ctx, run: &RunView, problem: &CreateProblem, again: Option<Start>) -> Element {
  let (title, body) = failure_words(ctx, run, problem);
  let notice = Notice::new("task-pull-request-failure", NoticeTone::Warning, title)
    .icon(Icon::TriangleAlert)
    .body(body)
    .stacked();
  let notice = if matches!(
    problem,
    CreateProblem::Push {
      failure: PushFailure::Authentication,
      ..
    }
  ) {
    let copier = Copier::of(ctx);
    notice.part(
      CommandRow::new(
        "task-pull-request-command",
        SETUP_GIT,
        ctx.t("common.copy"),
        move || copier.copy(SETUP_GIT),
      )
      .dollar_prompt()
      .render(ctx),
    )
  } else {
    notice
  };
  let retry = ActionButton::new(
    "task-pull-request-retry",
    ButtonKind::Secondary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RefreshCw);
  let retry = match again {
    Some(again) => retry.on_click(move || again()),
    None => retry.disabled(),
  };
  notice.action(retry).into()
}

/// The failure notice's title and what it says.
fn failure_words(ctx: &Ctx, run: &RunView, problem: &CreateProblem) -> (Arc<str>, Arc<str>) {
  let branch = run.run.branch().as_str();
  let not_opened = || ctx.t("decisions.ready.pullRequest.failure.notOpened");
  match problem {
    CreateProblem::Push { failure, remote, host } => push_words(ctx, failure, (branch, remote.as_str(), host.as_str())),
    CreateProblem::NoCommits => (
      not_opened(),
      ctx.t_args(
        "decisions.ready.pullRequest.failure.noCommits",
        [
          ("branch", branch),
          ("base", run.run.base_branch().map_or("", |base| base.as_str())),
        ],
      ),
    ),
    CreateProblem::GitHub(error) => (
      ctx.t("decisions.ready.pullRequest.failure.ghTitle"),
      github_words(ctx, error),
    ),
    CreateProblem::Unavailable(unavailable) => (not_opened(), unavailable_reason(ctx, unavailable)),
    CreateProblem::NotReady => (not_opened(), ctx.t("decisions.ready.pullRequest.failure.notReady")),
    CreateProblem::Store(message) => (not_opened(), message_text(ctx, message)),
    CreateProblem::AlreadyOpen { .. } => (not_opened(), ctx.t("decisions.ready.pullRequest.failure.alreadyOpen")),
    // The card asks this in place (`unpushed::question`); these are its words, should it ever be a failure.
    CreateProblem::BaseNotPushed { commits, base, remote } => unpushed::words(
      ctx,
      &Unpushed {
        commits: *commits,
        base,
        remote,
      },
    ),
  }
}

fn push_words(ctx: &Ctx, failure: &PushFailure, (branch, remote, host): (&str, &str, &str)) -> (Arc<str>, Arc<str>) {
  match failure {
    PushFailure::Authentication => (
      ctx.t_args(
        "decisions.ready.pullRequest.failure.signInTitle",
        [("host", host), ("branch", branch)],
      ),
      ctx.t("decisions.ready.pullRequest.failure.signIn"),
    ),
    PushFailure::Rejected { reason } => (
      ctx.t_args(
        "decisions.ready.pullRequest.failure.rejectedTitle",
        [("remote", remote), ("branch", branch)],
      ),
      ctx.t_args(
        "decisions.ready.pullRequest.failure.rejected",
        [("reason", reason.as_str()), ("remote", remote)],
      ),
    ),
    PushFailure::Failed { reason } => (
      ctx.t_args("decisions.ready.pullRequest.failure.pushTitle", [("branch", branch)]),
      ctx.t_args(
        "decisions.ready.pullRequest.failure.pushFailed",
        [("reason", reason.as_str())],
      ),
    ),
    PushFailure::NotRun(CommandError::NotFound) => (
      ctx.t_args("decisions.ready.pullRequest.failure.pushTitle", [("branch", branch)]),
      ctx.t("decisions.ready.pullRequest.failure.gitMissing"),
    ),
    PushFailure::NotRun(_) => (
      ctx.t_args("decisions.ready.pullRequest.failure.pushTitle", [("branch", branch)]),
      ctx.t("decisions.ready.pullRequest.failure.gitStopped"),
    ),
  }
}

/// What GitHub refusing to open the pull request means.
fn github_words(ctx: &Ctx, error: &RepositoryError) -> Arc<str> {
  match error {
    RepositoryError::Network { .. } => ctx.t("decisions.ready.pullRequest.failure.unreachable"),
    RepositoryError::ServerError { .. } => ctx.t("decisions.ready.pullRequest.failure.troubled"),
    RepositoryError::RateLimited => ctx.t("decisions.ready.pullRequest.failure.rateLimited"),
    RepositoryError::NotSignedIn(host) => ctx.t_args(
      "decisions.ready.pullRequest.failure.notSignedIn",
      [("host", host.as_str())],
    ),
    RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) => {
      ctx.t("decisions.ready.pullRequest.failure.noAccess")
    }
    RepositoryError::Unrecognized { detail, .. } => ctx.t_args(
      "decisions.ready.pullRequest.failure.ghSaid",
      [("detail", detail.as_str())],
    ),
    _ => ctx.t("decisions.ready.pullRequest.failure.ghFailed"),
  }
}
