//! Why a turn failed (`Failed` `ItPC2` in `4 · Report and the rest` `VZKyw`): for the newest one a `danger_muted`
//! line, radius 6, 8 over and under, 10 in at its start and 6 at its end, its parts 10 apart: a 13 `circle-alert` in
//! `danger`, what happened (12.5) over what it means (12 `text_secondary`), *Retry*, and the place to fix it when the
//! connection is why (*Open settings*, *Open Connections*, or *Pick account* in the composer's runs-on picker); for an
//! earlier one the words of its divider.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::AccountStartProblem, id::ProjectId, orchestrator::TurnFailure};

use super::timeline::TurnRow;
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    tooltip::Truncated,
  },
};

/// Why the newest turn failed, with *Retry* and where to fix it.
pub(super) fn failure_line(ctx: &mut Ctx, row: &TurnRow, failure: &TurnFailure) -> Element {
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
    TurnFailure::Account(problem) => (unreachable(), account(ctx, problem), Fix::Account),
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
  Row::new()
    .id("orchestrator-failure")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Sm)
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Snug.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::DangerMuted)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::Danger))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id("orchestrator-failure-title")
            .variant(OrchestratorText::Line.role())
            .width(Dimension::Pct(100.0)),
        )
        // One line (`LsXK1`): a longer reason is cut, whole in its tooltip.
        .child(Truncated::new("orchestrator-failure-body", body, TypographyStyle::Description).render(ctx)),
    )
    .with_children(retry(ctx, row))
    .with_children(fix.button(ctx, row))
    .into()
}

/// *Retry* sends the words once more: a message with attached files, which it would send without them, is sent again
/// from the composer instead.
fn retry(ctx: &Ctx, row: &TurnRow) -> Option<ActionButton> {
  if !row.turn.attachments.is_empty() {
    return None;
  }
  let ask = row.env.ask_again.clone();
  let words = row.turn.text.clone();
  let retry = ActionButton::new(
    "orchestrator-retry",
    ButtonKind::Secondary,
    ctx.t("orchestrator.failure.retry"),
  )
  .icon(Icon::RotateCcw)
  .small();
  Some(if row.can_ask {
    retry.on_click(move || ask(words.clone()))
  } else {
    retry.disabled()
  })
}

/// Where the person fixes what kept the Orchestrator from answering.
#[derive(Clone, Copy)]
enum Fix {
  Connections,
  Settings,
  /// Another account, from the composer's runs-on picker.
  Account,
  None,
}

impl Fix {
  fn button(self, ctx: &mut Ctx, row: &TurnRow) -> Option<ActionButton> {
    let project: ProjectId = row.env.project;
    let (id, key, glyph, page) = match self {
      Self::Connections => (
        "orchestrator-open-connections",
        "orchestrator.failure.openConnections",
        Icon::Plug,
        ProjectPage::Connections(ConnectionsView::default()),
      ),
      Self::Settings => (
        "orchestrator-open-settings",
        "orchestrator.notReady.openSettings",
        Icon::Settings,
        ProjectPage::OrchestratorSettings,
      ),
      Self::Account => {
        let pick = row.env.pick_account.clone();
        return Some(
          ActionButton::new(
            "orchestrator-pick-account",
            ButtonKind::Ghost,
            ctx.t("orchestrator.failure.pickAccount"),
          )
          .icon(Icon::UserRound)
          .on_click(move || pick()),
        );
      }
      Self::None => return None,
    };
    let navigation = Navigation::of(ctx);
    Some(
      ActionButton::new(id, ButtonKind::Ghost, ctx.t(key))
        .icon(glyph)
        .opens(navigation.to(Route::Project(project, page))),
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
  }
}

/// An earlier turn's failure, on its divider.
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
