//! Why a turn failed: for the newest one a danger notice with *Retry* (and *Open Connections* or *Open settings* when
//! the connection is why), for an earlier one the words of its event line.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{connection::AccountStartProblem, id::ProjectId, orchestrator::TurnFailure};

use super::timeline::TurnRow;
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// Why the newest turn failed, with *Retry* and where to fix it.
pub(super) fn failure_notice(ctx: &mut Ctx, row: &TurnRow, failure: &TurnFailure) -> Element {
  let connection = row.env.connection.clone();
  let unreachable = || match &connection {
    Some(name) => ctx.t_args("orchestrator.failure.unreachable", [("connection", &**name)]),
    None => ctx.t("orchestrator.failure.unreachableUnknown"),
  };
  let (title, body, fix) = match failure {
    TurnFailure::NotSignedIn => (
      unreachable(),
      ctx.t("orchestrator.failure.notSignedIn"),
      Fix::Connections,
    ),
    TurnFailure::ConnectionGone => (
      unreachable(),
      ctx.t("orchestrator.failure.connectionGone"),
      Fix::Settings,
    ),
    TurnFailure::Account(problem) => (unreachable(), account(ctx, problem), Fix::Connections),
    TurnFailure::CannotStart { .. } => (unreachable(), ctx.t("orchestrator.failure.cannotStart"), Fix::None),
    TurnFailure::Failed { reason } => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t_args("orchestrator.failure.failedBody", [("reason", reason.as_str())]),
      Fix::None,
    ),
    TurnFailure::Interrupted => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t("orchestrator.interrupted"),
      Fix::None,
    ),
    TurnFailure::TimeLimit { minutes } => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t_args("orchestrator.failure.timeLimit", [("minutes", minutes.to_string())]),
      Fix::None,
    ),
  };
  let mut notice = Notice::new("orchestrator-failure", NoticeTone::Danger, title)
    .icon(Icon::CloudOff)
    .body(body);
  // *Retry* sends the words once more: a message with attached files, which it would send without them, is sent
  // again from the composer instead.
  if row.turn.attachments.is_empty() {
    let ask = row.env.ask_again.clone();
    let words = row.turn.text.clone();
    let mut retry = ActionButton::new(
      "orchestrator-retry",
      ButtonKind::Secondary,
      ctx.t("orchestrator.failure.retry"),
    )
    .icon(Icon::RotateCcw);
    retry = if row.can_ask {
      retry.on_click(move || ask(words.clone()))
    } else {
      retry.disabled()
    };
    notice = notice.action(retry);
  }
  if let Some(open) = fix.button(ctx, row.env.project) {
    notice = notice.action(open);
  }
  notice.into()
}

/// Where the person fixes what kept the Orchestrator from answering.
#[derive(Clone, Copy)]
enum Fix {
  Connections,
  Settings,
  None,
}

impl Fix {
  fn button(self, ctx: &mut Ctx, project: ProjectId) -> Option<ActionButton> {
    let (id, key, page) = match self {
      Self::Connections => (
        "orchestrator-open-connections",
        "orchestrator.failure.openConnections",
        ProjectPage::Connections(ConnectionsView::default()),
      ),
      Self::Settings => (
        "orchestrator-open-settings",
        "orchestrator.notReady.openSettings",
        ProjectPage::OrchestratorSettings,
      ),
      Self::None => return None,
    };
    let navigation = Navigation::of(ctx);
    let route = Route::Project(project, page);
    Some(
      ActionButton::new(id, ButtonKind::Primary, ctx.t(key))
        .icon(Icon::ArrowUpRight)
        .opens(navigation.to(route)),
    )
  }
}

/// Why the connection's account cannot start the CLI.
fn account(ctx: &Ctx, problem: &AccountStartProblem) -> Arc<str> {
  match problem {
    AccountStartProblem::NoAccount => ctx.t("orchestrator.failure.accountNone"),
    AccountStartProblem::FolderMissing { folder } => ctx.t_args(
      "orchestrator.failure.accountFolderMissing",
      [("folder", folder.display().to_string().as_str())],
    ),
    AccountStartProblem::Unsupported => ctx.t("orchestrator.failure.accountUnsupported"),
  }
}

/// An earlier turn's failure, on its event line.
pub(super) fn earlier(ctx: &Ctx, failure: &TurnFailure) -> Arc<str> {
  match failure {
    TurnFailure::NotSignedIn => ctx.t("orchestrator.failure.earlierNotSignedIn"),
    TurnFailure::Failed { reason } => ctx.t_args("orchestrator.failure.earlier", [("reason", reason.as_str())]),
    TurnFailure::TimeLimit { minutes } => ctx.t_args(
      "orchestrator.failure.earlierTimeLimit",
      [("minutes", minutes.to_string())],
    ),
    TurnFailure::ConnectionGone
    | TurnFailure::Account(_)
    | TurnFailure::CannotStart { .. }
    | TurnFailure::Interrupted => ctx.t("orchestrator.failure.earlierUnreachable"),
  }
}
