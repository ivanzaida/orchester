//! The open project's runner in the shell: the sidebar card (`O2Jjjf` in `zo9Py`, states
//! `O5NoB1`) and the connection notice at the top of the overview (02.F `MAn7V`, 02.G `YHS1v`,
//! 02.H `EFY7I`). Agent slots and runs do not exist yet, so neither shows counts of them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Rect, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};

use crate::{
  messages,
  runner::{
    RunnerStatus, Runners,
    state::{RECONNECT_LIMIT, RunnerState},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    countdown::{Countdown, CountdownProps},
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
};

/// Online indicator size (`fK4mR`) and notice action height (`atylT`, `Yfte7`).
const INDICATOR_SIZE: f32 = 5.0;
const NOTICE_ACTION_HEIGHT: f32 = 32.0;

/// Sidebar runner card (`O2Jjjf`) in the states of `O5NoB1`.
pub fn card(ctx: &Ctx, state: &RunnerState) -> Element {
  let (glyph, title, detail) = card_copy(state);
  let marker: Element = match glyph {
    Some(glyph) => icon(glyph, IconSize::Small, PaletteColor::TextPrimary).into(),
    None => Rect::new(Dimension::Px(INDICATOR_SIZE), Dimension::Px(INDICATOR_SIZE))
      .background(PaletteColor::TextPrimary)
      .rounded(RadiusSize::Lg)
      .into(),
  };
  let detail = match state {
    RunnerState::Reconnecting { attempt, .. } => attempt_text(ctx, detail, *attempt),
    _ => ctx.t(detail),
  };
  Column::new()
    .id("runner-status")
    .describe("state", state_name(state))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(ExtraRadius::Notice.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Indicator.role())
        .align_items(Alignment::Center)
        .child(marker)
        .child(
          Text::new(&ctx.t(title))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        ),
    )
    .child(Text::new(&detail).variant(ExtraTypography::Footnote.role()))
    .into()
}

/// Icon (none: the online dot), title key and detail key of the card.
fn card_copy(state: &RunnerState) -> (Option<Icon>, &'static str, &'static str) {
  match state {
    RunnerState::Starting => (Some(Icon::Loader), "runner.card.starting", "runner.card.startingDetail"),
    RunnerState::Online { .. } => (None, "runner.card.online", "runner.card.onlineDetail"),
    RunnerState::Reconnecting { .. } => (
      Some(Icon::RefreshCw),
      "runner.card.reconnecting",
      "runner.card.reconnectingDetail",
    ),
    RunnerState::Stopping => (Some(Icon::Loader), "runner.card.stopping", "runner.card.stoppingDetail"),
    RunnerState::Stopped => (Some(Icon::Unplug), "runner.card.offline", "runner.card.stoppedDetail"),
    RunnerState::Failed { attempts: 0, .. } => (
      Some(Icon::Unplug),
      "runner.card.offline",
      "runner.card.notStartedDetail",
    ),
    RunnerState::Failed { .. } => (
      Some(Icon::Unplug),
      "runner.card.offline",
      "runner.card.notRestartedDetail",
    ),
  }
}

/// For tooling (`lurq_read_tree`): the runner state without reading the copy.
fn state_name(state: &RunnerState) -> &'static str {
  match state {
    RunnerState::Starting => "starting",
    RunnerState::Online { .. } => "online",
    RunnerState::Reconnecting { .. } => "reconnecting",
    RunnerState::Stopping => "stopping",
    RunnerState::Stopped => "stopped",
    RunnerState::Failed { .. } => "failed",
  }
}

fn attempt_text(ctx: &Ctx, key: &str, attempt: u32) -> Arc<str> {
  let attempt = attempt.to_string();
  let limit = RECONNECT_LIMIT.to_string();
  ctx.t_args(key, [("attempt", attempt.as_str()), ("limit", limit.as_str())])
}

/// The connection notice at the top of the overview, while the runner is not simply online.
pub fn connection_notice(
  ctx: &mut Ctx,
  status: &RunnerStatus,
  state: &RunnerState,
  runners: &Runners,
) -> Option<Element> {
  let notice = match state {
    RunnerState::Reconnecting { attempt, .. } => Notice::new(
      Icon::RefreshCw,
      attempt_text(ctx, "runner.connection.reconnectingTitle", *attempt),
      ctx.t("runner.connection.reconnectingDetail"),
    )
    .action(countdown(
      ctx,
      status,
      "runner.connection.nextAttempt",
      "runner.connection.attempting",
    ))
    .action(open_log(ctx, runners)),
    RunnerState::Online { reconnected: true } => Notice::new(
      Icon::CircleCheck,
      ctx.t("runner.connection.reconnectedTitle"),
      ctx.t("runner.connection.reconnectedDetail"),
    )
    .action(countdown(
      ctx,
      status,
      "runner.connection.closesIn",
      "runner.connection.closing",
    ))
    .action(dismiss(runners)),
    RunnerState::Failed { cause, attempts } => {
      let (title, detail) = if *attempts == 0 {
        (
          ctx.t("runner.connection.notStartedTitle"),
          message_text(ctx, &messages::runner_failure(cause)),
        )
      } else {
        let attempts = attempts.to_string();
        (
          ctx.t("runner.connection.notRestartedTitle"),
          ctx.t_args(
            "runner.connection.notRestartedDetail",
            [("attempts", attempts.as_str())],
          ),
        )
      };
      Notice::new(Icon::Unplug, title, detail)
        .action(open_log(ctx, runners))
        .action(restart(ctx, runners))
    }
    _ => return None,
  };
  Some(notice.emphasized().id("runner-notice").into())
}

/// Seconds until the current state ends on its own (`Q1TRov`, `R4Nsnb`).
fn countdown(ctx: &mut Ctx, status: &RunnerStatus, counting: &'static str, done: &'static str) -> Element {
  let Some(until) = status.until() else {
    return Column::new().into();
  };
  ctx.mount_keyed::<Countdown>(counting, CountdownProps { until, counting, done })
}

/// "Open runner log" (`atylT`, `JUsfL`).
fn open_log(ctx: &Ctx, runners: &Runners) -> Element {
  let runners = runners.clone();
  ActionButton::new(ctx.t("runner.connection.openLog"))
    .icon(Icon::FileText)
    .id("runner-open-log")
    .height(Dimension::Px(NOTICE_ACTION_HEIGHT))
    .on_click(move || runners.open_log())
    .render(ctx)
}

/// "Restart runner" (`Yfte7`).
fn restart(ctx: &Ctx, runners: &Runners) -> Element {
  let runners = runners.clone();
  ActionButton::new(ctx.t("runner.connection.restart"))
    .primary()
    .icon(Icon::RefreshCw)
    .id("runner-restart")
    .height(Dimension::Px(NOTICE_ACTION_HEIGHT))
    .on_click(move || runners.restart())
    .render(ctx)
}

/// Dismiss control of the reconnected notice (`KGM0S`).
fn dismiss(runners: &Runners) -> Element {
  let runners = runners.clone();
  Button::empty()
    .button()
    .id("runner-notice-dismiss")
    .cursor(CursorIcon::Pointer)
    .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary))
    .on_click(move |_| runners.dismiss_reconnected_notice())
    .into()
}
