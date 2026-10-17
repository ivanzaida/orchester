//! Why the latest sync did not read the list, over the list it keeps (`docs/design/orchester-workspace.md`,
//! "GitHub", rule 9). GitHub unreachable or failing, and a rate limit, are a notice on `surface_raised` with
//! the cause's icon (`Notice · Showing pull requests from 14:02` in `uuaob`); another account or a missing
//! scope is the warning notice with the command to run in a terminal, Check again and, for another account,
//! *Use this account* (`Account notice` `CWJuh` in `WUOYP`). The states the design draws no notice for are
//! worded like the GitHub connection's; Open Connections is where they are fixed.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{GitHubState, REQUIRED_SCOPE},
  pull_request::SyncFailure,
  repository::RepositoryError,
  time::Timestamp,
};
use orchester_workspace::{
  display,
  pull_requests::{list::LinkedList, status::Remedy},
};

use super::status::{account, minutes_until};
use crate::{
  navigation::link::Open,
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    command_row::CommandRow,
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the notice's buttons do; `None` while a sync or *Use this account* runs.
pub(super) struct NoticeActions {
  pub check_again: Option<Handler>,
  pub use_account: Option<Handler>,
  pub open_connections: Open,
}

/// The notice for `list`'s failed sync, `project` the project's name; `None` after a sync that read it.
pub(super) fn failure_notice(
  ctx: &mut Ctx,
  list: &LinkedList,
  project: &str,
  now: Timestamp,
  actions: &NoticeActions,
) -> Option<Element> {
  let failure = list.failure.as_ref()?;
  let time = list.synced_at.map(display::clock);
  let notice = match failure {
    SyncFailure::RateLimited { resets_at } => rate_limited(ctx, list, time.as_deref(), *resets_at, now),
    SyncFailure::Account(state) => account_notice(ctx, list, project, time.as_deref(), state),
    SyncFailure::Repository(error) => repository(ctx, list, time.as_deref(), error),
  };
  let notice = with_command(ctx, notice, list, failure);
  Some(with_buttons(ctx, notice, &Remedy::of(failure), actions).into())
}

fn rate_limited(
  ctx: &Ctx,
  list: &LinkedList,
  time: Option<&str>,
  resets_at: Option<Timestamp>,
  now: Timestamp,
) -> Notice {
  let account = list.expected.as_str();
  let detail = match (resets_at, time) {
    (None, _) => ctx.t_args("pull_requests.notice.rateLimitedUnknown", [("account", account)]),
    (Some(at), Some(time)) => ctx.t_args(
      "pull_requests.notice.rateLimited",
      [
        ("account", account.to_owned()),
        ("minutes", minutes_until(now, at).to_string()),
        ("time", time.to_owned()),
      ],
    ),
    (Some(at), None) => ctx.t_args(
      "pull_requests.notice.rateLimitedNoList",
      [
        ("account", account.to_owned()),
        ("minutes", minutes_until(now, at).to_string()),
      ],
    ),
  };
  Notice::new(
    "pull-requests-notice",
    NoticeTone::Info,
    ctx.t("pull_requests.notice.rateLimitedTitle"),
  )
  .icon(Icon::Timer)
  .body(detail)
}

fn account_notice(ctx: &Ctx, list: &LinkedList, project: &str, time: Option<&str>, state: &GitHubState) -> Notice {
  let warning = |title: Arc<str>, body: Arc<str>| {
    Notice::new("pull-requests-notice", NoticeTone::Warning, title)
      .icon(Icon::CircleAlert)
      .body(body)
      .stacked()
  };
  match state {
    GitHubState::WrongAccount { active, expected } => {
      let names = [
        ("active", active.as_str().to_owned()),
        ("expected", expected.as_str().to_owned()),
        ("project", project.to_owned()),
      ];
      let body = match time {
        Some(time) => {
          let [active, expected, project] = names.clone();
          ctx.t_args(
            "pull_requests.notice.wrongAccount",
            [active, expected, project, ("time", time.to_owned())],
          )
        }
        None => ctx.t_args("pull_requests.notice.wrongAccountNoList", names.clone()),
      };
      let [active, expected, _] = names;
      warning(
        ctx.t_args("pull_requests.notice.wrongAccountTitle", [active, expected]),
        body,
      )
    }
    GitHubState::MissingScope { account } => {
      let names = [
        ("account", account.as_str().to_owned()),
        ("scope", REQUIRED_SCOPE.to_owned()),
      ];
      let body = match time {
        Some(time) => {
          let [account, scope] = names;
          ctx.t_args(
            "pull_requests.notice.missingScope",
            [account, scope, ("time", time.to_owned())],
          )
        }
        None => ctx.t_args("pull_requests.notice.missingScopeNoList", names),
      };
      warning(super::status::account(ctx, state), body)
    }
    GitHubState::NotSignedIn => warning(
      ctx.t_args(
        "pull_requests.notice.notSignedInTitle",
        [("host", list.repo.host().as_str())],
      ),
      ctx.t("pull_requests.notice.notSignedIn"),
    ),
    other => warning(account(ctx, other), ctx.t(hint_key(other))),
  }
}

/// How the GitHub connection explains a state the page draws no notice for.
const fn hint_key(state: &GitHubState) -> &'static str {
  match state {
    GitHubState::NotChecked => "pull_requests.hint.notChecked",
    GitHubState::SignInUnknown => "pull_requests.hint.signInUnknown",
    GitHubState::ProgramMissing => "pull_requests.hint.programMissing",
    GitHubState::Unrecognized => "pull_requests.hint.unrecognized",
    GitHubState::CouldNotRun
    | GitHubState::SignedIn { .. }
    | GitHubState::NotSignedIn
    | GitHubState::WrongAccount { .. }
    | GitHubState::MissingScope { .. } => "pull_requests.hint.couldNotRun",
  }
}

fn repository(ctx: &Ctx, list: &LinkedList, time: Option<&str>, error: &RepositoryError) -> Notice {
  let title = || match time {
    Some(time) => ctx.t_args("pull_requests.notice.listFrom", [("time", time)]),
    None => ctx.t("pull_requests.notice.notSynced"),
  };
  let failed = |glyph: Icon, body: Arc<str>| {
    Notice::new("pull-requests-notice", NoticeTone::Info, title())
      .icon(glyph)
      .mark(lurq::app::theme::PaletteColor::Danger)
      .body(body)
  };
  match error {
    RepositoryError::Network { .. } => {
      let key = match time {
        Some(_) => "pull_requests.notice.unreachable",
        None => "pull_requests.notice.unreachableNoList",
      };
      failed(Icon::CloudOff, ctx.t(key))
    }
    RepositoryError::ServerError { status, .. } => failed(
      Icon::ServerCrash,
      match status {
        Some(status) => ctx.t_args("pull_requests.notice.serverError", [("status", status.to_string())]),
        None => ctx.t("pull_requests.notice.serverErrorUnknown"),
      },
    ),
    RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) => Notice::new(
      "pull-requests-notice",
      NoticeTone::Warning,
      ctx.t_args(
        "pull_requests.notice.noAccessTitle",
        [
          ("account", list.expected.as_str()),
          ("owner", list.repo.owner()),
          ("name", list.repo.name()),
        ],
      ),
    )
    .icon(Icon::FolderLock)
    .body(ctx.t("pull_requests.notice.noAccess"))
    .stacked(),
    RepositoryError::NotSignedIn(_) => account_notice(ctx, list, "", time, &GitHubState::NotSignedIn),
    RepositoryError::Unrecognized { .. } => failed(Icon::CircleHelp, ctx.t("pull_requests.notice.unrecognized")),
    RepositoryError::Run { .. } => failed(Icon::CircleAlert, ctx.t("pull_requests.notice.couldNotRun")),
    _ => failed(Icon::CircleAlert, ctx.t("pull_requests.notice.failed")),
  }
}

/// The command `gh` needs run in a terminal, with Copy, for an account the project cannot use.
fn with_command(ctx: &mut Ctx, notice: Notice, list: &LinkedList, failure: &SyncFailure) -> Notice {
  let advice = match failure {
    SyncFailure::Account(state) => state.advice(),
    SyncFailure::Repository(RepositoryError::NotSignedIn(_)) => GitHubState::NotSignedIn.advice(),
    SyncFailure::RateLimited { .. } | SyncFailure::Repository(_) => None,
  };
  let Some(advice) = advice else {
    return notice;
  };
  let command = advice.command(list.repo.host());
  let (copier, copied) = (Copier::of(ctx), command.clone());
  let row = CommandRow::new("pull-requests-command", command, ctx.t("common.copy"), move || {
    copier.copy(&copied)
  })
  .dollar_prompt()
  .render(ctx);
  notice.part(row)
}

/// Check again, and *Use this account* or Open Connections, as the failure offers.
fn with_buttons(ctx: &Ctx, notice: Notice, remedy: &Remedy, actions: &NoticeActions) -> Notice {
  let button = |id: &str, key: &str, glyph: Icon, run: Option<&Handler>| {
    let button = ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t(key)).icon(glyph);
    match run.cloned() {
      Some(run) => button.on_click(move || run()),
      None => button.disabled(),
    }
  };
  let check_again = || {
    button(
      "pull-requests-check-again",
      "pull_requests.notice.checkAgain",
      Icon::RefreshCw,
      actions.check_again.as_ref(),
    )
  };
  match remedy {
    Remedy::None => notice,
    Remedy::CheckAgain => notice.action(check_again()),
    Remedy::CheckAgainOrUseAccount { active } => {
      let label = ctx.t_args("pull_requests.notice.useAccount", [("account", active.as_str())]);
      let mut using =
        ActionButton::new("pull-requests-use-account", ButtonKind::Secondary, label).icon(Icon::UserRoundCheck);
      using = match actions.use_account.clone() {
        Some(run) => using.on_click(move || run()),
        None => using.disabled(),
      };
      notice.action(check_again()).action(using)
    }
    Remedy::CheckAgainOrConnections => notice.action(check_again()).action(
      ActionButton::new(
        "pull-requests-open-connections",
        ButtonKind::Secondary,
        ctx.t("pull_requests.unavailable.openConnections"),
      )
      .icon(Icon::Plug)
      .opens(actions.open_connections.clone()),
    ),
  }
}

/// Why the latest read of one pull request failed, over what was read before (`Couldn't reach GitHub` in
/// `c07NAC`); `synced_at` is when that was.
pub(super) fn detail_notice(ctx: &Ctx, failure: &SyncFailure, synced_at: Option<Timestamp>) -> Element {
  let id = "pull-request-notice";
  let failed = |glyph: Icon, title: Arc<str>, body: Arc<str>| {
    Notice::new(id, NoticeTone::Info, title)
      .icon(glyph)
      .mark(lurq::app::theme::PaletteColor::Danger)
      .body(body)
  };
  let notice = match failure {
    SyncFailure::RateLimited { resets_at } => {
      Notice::new(id, NoticeTone::Info, ctx.t("pull_requests.notice.rateLimitedTitle"))
        .icon(Icon::Timer)
        .body(match resets_at {
          Some(at) => ctx.t_args(
            "pull_requests.detail.notice.rateLimited",
            [("minutes", minutes_until(Timestamp::now(), *at).to_string())],
          ),
          None => ctx.t("pull_requests.detail.notice.rateLimitedUnknown"),
        })
    }
    SyncFailure::Account(state) => Notice::new(id, NoticeTone::Warning, account(ctx, state))
      .icon(Icon::CircleAlert)
      .body(ctx.t("pull_requests.detail.notice.account")),
    SyncFailure::Repository(RepositoryError::Network { .. }) => failed(
      Icon::CloudOff,
      ctx.t("pull_requests.detail.notice.unreachable"),
      ctx.t("pull_requests.detail.notice.mayHaveChanged"),
    ),
    SyncFailure::Repository(RepositoryError::ServerError { .. }) => failed(
      Icon::ServerCrash,
      ctx.t("pull_requests.detail.notice.troubled"),
      ctx.t("pull_requests.detail.notice.mayHaveChanged"),
    ),
    SyncFailure::Repository(RepositoryError::PullRequestNotFound { .. }) => {
      Notice::new(id, NoticeTone::Warning, ctx.t("pull_requests.detail.notice.notFound"))
        .icon(Icon::CircleHelp)
        .body(ctx.t("pull_requests.detail.notice.notFoundDetail"))
    }
    SyncFailure::Repository(_) => failed(
      Icon::CircleAlert,
      ctx.t("pull_requests.detail.notice.failed"),
      match synced_at {
        Some(at) => ctx.t_args("pull_requests.detail.notice.failedFrom", [("time", display::clock(at))]),
        None => ctx.t("pull_requests.detail.notice.failedNothing"),
      },
    ),
  };
  notice.into()
}
