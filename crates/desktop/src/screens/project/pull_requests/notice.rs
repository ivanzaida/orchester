//! Why the latest sync did not read the list, above the list it keeps (`docs/design/
//! orchester-workspace.md`, "GitHub", rule 9): GitHub unreachable (08.E `FYQMv`), rate limited
//! (08.F `P1tSu`), another account (08.G `E9ZThy`), a missing scope (08.H `tu1fW`), and the
//! states the design has no Pull requests frame for, worded like the GitHub connection's.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  core::Signal,
  layout::layout_kind::Justify,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{GhAdvice, GitHubState, REQUIRED_SCOPE},
  pull_request::SyncFailure,
  repository::RepositoryError,
  time::Timestamp,
};

use super::view::{list_time, minutes_until};
use crate::{
  pull_requests::LinkedInbox,
  screens::{
    github::status::{command_ids, status_label},
    project::page::TOPBAR_BUTTON_HEIGHT,
  },
  ui::{
    button::ActionButton,
    command_row::{CommandPlace, Copied, command_row},
    icon::Icon,
    notice::Notice,
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the buttons under a notice do; `None` while a sync or check runs.
pub struct NoticeActions {
  pub check_again: Option<Handler>,
  pub use_account: Option<Handler>,
  pub open_connections: Handler,
  pub copied: Signal<Copied>,
}

/// The buttons a failure offers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Buttons {
  None,
  CheckAgain,
  CheckAgainAndUseAccount,
  CheckAgainAndConnections,
}

/// The notice, command row and buttons for `inbox`'s failure; `None` after a successful sync.
pub fn failure_notice(
  ctx: &Ctx,
  inbox: &LinkedInbox,
  project: &str,
  now: Timestamp,
  actions: &NoticeActions,
) -> Option<Element> {
  let failure = inbox.failure.as_ref()?;
  let time = list_time(inbox);
  let time = time.as_deref();
  let (notice, advice, buttons) = match failure {
    SyncFailure::RateLimited { resets_at } => (rate_limited(ctx, inbox, time, *resets_at, now), None, Buttons::None),
    SyncFailure::Account(state) => account(ctx, inbox, project, time, state),
    SyncFailure::Repository(error) => repository(ctx, inbox, time, error),
  };
  let host = inbox.repo.host();
  let mut column = Column::new()
    .id("pull-requests-notice")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(notice);
  if let Some(advice) = advice {
    let (id, copy_id) = command_ids(&advice);
    column = column.child(command_row(
      ctx,
      id,
      copy_id,
      &advice.command(host),
      CommandPlace::Page,
      &actions.copied,
    ));
  }
  if buttons != Buttons::None {
    column = column.child(button_row(ctx, buttons, failure, actions));
  }
  Some(column.into())
}

fn rate_limited(
  ctx: &Ctx,
  inbox: &LinkedInbox,
  time: Option<&str>,
  resets_at: Option<Timestamp>,
  now: Timestamp,
) -> Notice {
  let account = inbox.expected.as_str();
  let detail = match (resets_at, time) {
    (None, _) => ctx.t_args("pullRequests.notice.rateLimitedUnknown", [("account", account)]),
    (Some(at), time) => {
      let minutes = minutes_until(now, at).to_string();
      match time {
        Some(time) => ctx.t_args(
          "pullRequests.notice.rateLimited",
          [("account", account), ("minutes", &minutes), ("time", time)],
        ),
        None => ctx.t_args(
          "pullRequests.notice.rateLimitedNoList",
          [("account", account), ("minutes", &minutes)],
        ),
      }
    }
  };
  Notice::new(Icon::Timer, ctx.t("pullRequests.notice.rateLimitedTitle"), detail)
}

fn account(
  ctx: &Ctx,
  inbox: &LinkedInbox,
  project: &str,
  time: Option<&str>,
  state: &GitHubState,
) -> (Notice, Option<GhAdvice>, Buttons) {
  let advice = state.advice();
  match state {
    GitHubState::WrongAccount { active, expected } => {
      let names = [
        ("active", active.as_str()),
        ("expected", expected.as_str()),
        ("project", project),
      ];
      let detail = match time {
        Some(time) => ctx.t_args(
          "pullRequests.notice.wrongAccount",
          [names[0], names[1], names[2], ("time", time)],
        ),
        None => ctx.t_args("pullRequests.notice.wrongAccountNoList", names),
      };
      let title = ctx.t_args("pullRequests.notice.wrongAccountTitle", [names[0], names[1]]);
      (
        Notice::new(Icon::UserX, title, detail),
        advice,
        Buttons::CheckAgainAndUseAccount,
      )
    }
    GitHubState::MissingScope { account } => {
      let names = [("account", account.as_str()), ("scope", REQUIRED_SCOPE)];
      let detail = match time {
        Some(time) => ctx.t_args("pullRequests.notice.missingScope", [names[0], names[1], ("time", time)]),
        None => ctx.t_args("pullRequests.notice.missingScopeNoList", names),
      };
      let title = ctx.t_args("github.status.missingScope", [("scope", REQUIRED_SCOPE)]);
      (Notice::new(Icon::KeyRound, title, detail), advice, Buttons::CheckAgain)
    }
    GitHubState::NotSignedIn => (not_signed_in(ctx, inbox), advice, Buttons::CheckAgain),
    GitHubState::NotChecked => connection_state(ctx, state, "github.hint.notChecked"),
    GitHubState::SignedIn { .. } => connection_state(ctx, state, "github.hint.signedIn"),
    GitHubState::SignInUnknown => connection_state(ctx, state, "github.hint.signInUnknown"),
    GitHubState::ProgramMissing => connection_state(ctx, state, "github.hint.programMissing"),
    GitHubState::Unrecognized => connection_state(ctx, state, "github.hint.unrecognized"),
    GitHubState::CouldNotRun => connection_state(ctx, state, "github.hint.couldNotRun"),
  }
}

/// A state the Pull requests screen has no design for, worded like the GitHub connection's
/// (`US3dx` and its hint); the connection is where it is fixed.
fn connection_state(ctx: &Ctx, state: &GitHubState, hint: &str) -> (Notice, Option<GhAdvice>, Buttons) {
  let (glyph, title) = status_label(state);
  let notice = Notice::new(glyph, ctx.t_args(title, [("scope", REQUIRED_SCOPE)]), ctx.t(hint));
  (notice, None, Buttons::CheckAgainAndConnections)
}

fn not_signed_in(ctx: &Ctx, inbox: &LinkedInbox) -> Notice {
  Notice::new(
    Icon::AlertCircle,
    ctx.t_args(
      "pullRequests.notice.notSignedInTitle",
      [("host", inbox.repo.host().as_str())],
    ),
    ctx.t("github.hint.notSignedIn"),
  )
}

fn repository(
  ctx: &Ctx,
  inbox: &LinkedInbox,
  time: Option<&str>,
  error: &RepositoryError,
) -> (Notice, Option<GhAdvice>, Buttons) {
  let title = || match time {
    Some(time) => ctx.t_args("pullRequests.notice.listFrom", [("time", time)]),
    None => ctx.t("pullRequests.notice.notSynced"),
  };
  match error {
    RepositoryError::Network { .. } => {
      let key = if time.is_some() {
        "pullRequests.notice.unreachable"
      } else {
        "pullRequests.notice.unreachableNoList"
      };
      (Notice::new(Icon::CloudOff, title(), ctx.t(key)), None, Buttons::None)
    }
    RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) => {
      let repo = &inbox.repo;
      let heading = ctx.t_args(
        "pullRequests.notice.noAccessTitle",
        [
          ("account", inbox.expected.as_str()),
          ("owner", repo.owner()),
          ("name", repo.name()),
        ],
      );
      let notice = Notice::new(Icon::FolderLock, heading, ctx.t("pullRequests.notice.noAccess"));
      (notice, None, Buttons::CheckAgainAndConnections)
    }
    RepositoryError::NotSignedIn(_) => (not_signed_in(ctx, inbox), Some(GhAdvice::Login), Buttons::CheckAgain),
    RepositoryError::Unrecognized { .. } => (
      Notice::new(Icon::CircleHelp, title(), ctx.t("pullRequests.notice.unrecognized")),
      None,
      Buttons::None,
    ),
    RepositoryError::Run { .. } => (
      Notice::new(Icon::AlertCircle, title(), ctx.t("pullRequests.notice.couldNotRun")),
      None,
      Buttons::CheckAgainAndConnections,
    ),
    _ => (
      Notice::new(Icon::AlertCircle, title(), ctx.t("pullRequests.notice.failed")),
      None,
      Buttons::None,
    ),
  }
}

/// Check again, and Use this account or Open Connections, on the right (`yhdDU`, `mDOLF`).
fn button_row(ctx: &Ctx, buttons: Buttons, failure: &SyncFailure, actions: &NoticeActions) -> Element {
  let check = ActionButton::new(ctx.t("github.checkAgain"))
    .icon(Icon::RefreshCw)
    .id("pull-requests-check-again")
    .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
  let check = match actions.check_again.clone() {
    Some(handler) => check.on_click(move || handler()),
    None => check.on_click(|| {}).enabled(false),
  };
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .justify(Justify::End)
    .child(check.render(ctx));
  match (buttons, failure) {
    (Buttons::CheckAgainAndUseAccount, SyncFailure::Account(GitHubState::WrongAccount { active, .. })) => {
      let button = ActionButton::new(ctx.t_args("github.connection.useAccount", [("account", active.as_str())]))
        .icon(Icon::UserCheck)
        .id("pull-requests-use-account")
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
      let button = match actions.use_account.clone() {
        Some(handler) => button.on_click(move || handler()),
        None => button.on_click(|| {}).enabled(false),
      };
      row = row.child(button.render(ctx));
    }
    (Buttons::CheckAgainAndConnections, _) => {
      let open = actions.open_connections.clone();
      row = row.child(
        ActionButton::new(ctx.t("pullRequests.unavailable.openConnections"))
          .icon(Icon::Plug)
          .id("pull-requests-open-connections")
          .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
          .on_click(move || open())
          .render(ctx),
      );
    }
    _ => {}
  }
  row.into()
}
