//! The sidebar's runner card (`O2Jjjf` in `zo9Py`, states `O5NoB1`): the open project's runner as
//! icon and label, with the agent slots in use while it is online, the reconnect attempt while it
//! reconnects, and that it could not start or restart once it gave up. The status bar reports the
//! same runner on every screen and holds its actions; the card only shows where it stands.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  runner::{
    RunnerStatus,
    state::{RECONNECT_LIMIT, RunnerState},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
  workspace::OpenProject,
};

/// The connected indicator (`fK4mR`).
const DOT_SIZE: f32 = 5.0;

/// What leads the card's title: the online dot or a state icon.
enum Mark {
  Dot,
  Icon(Icon),
}

pub fn runner_card(ctx: &Ctx, open: &OpenProject, status: &RunnerStatus) -> Element {
  let state = status.state_for(&open.repo_root);
  let (mark, title, detail) = look(ctx, &state, status);
  let mark: Element = match mark {
    Mark::Dot => Row::new()
      .size(Dimension::Px(DOT_SIZE), Dimension::Px(DOT_SIZE))
      .background(PaletteColor::TextPrimary)
      .rounded(ExtraRadius::Pill.role())
      .into(),
    Mark::Icon(glyph) => icon(glyph, IconSize::Small, PaletteColor::TextPrimary).into(),
  };
  Column::new()
    .id("sidebar-runner")
    .describe("state", state_name(&state))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(ExtraRadius::Notice.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Indicator.role())
        .align_items(Alignment::Center)
        .child(mark)
        .child(
          Text::new(&ctx.t(title))
            .id("sidebar-runner-state")
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        ),
    )
    .with_children(detail.map(|detail| {
      Text::new(&detail)
        .id("sidebar-runner-detail")
        .variant(ExtraTypography::Footnote.role())
    }))
    .into()
}

/// The mark, the title key and the line under it of `state`. Stopping and stopped are not drawn;
/// they read like the other states and have no second line.
fn look(ctx: &Ctx, state: &RunnerState, status: &RunnerStatus) -> (Mark, &'static str, Option<Arc<str>>) {
  match state {
    RunnerState::Starting => (
      Mark::Icon(Icon::Loader),
      "runner.card.starting",
      Some(ctx.t("runner.card.startingDetail")),
    ),
    RunnerState::Online => (Mark::Dot, "runner.card.online", Some(slots(ctx, status))),
    RunnerState::Reconnecting { attempt, .. } => (
      Mark::Icon(Icon::RefreshCw),
      "runner.card.reconnecting",
      Some(attempt_line(ctx, *attempt, status.interrupted())),
    ),
    RunnerState::Failed { attempts, .. } => {
      let detail = if *attempts == 0 {
        "runner.card.notStarted"
      } else {
        "runner.card.notRestarted"
      };
      (Mark::Icon(Icon::Unplug), "runner.card.offline", Some(ctx.t(detail)))
    }
    RunnerState::Stopping => (Mark::Icon(Icon::Loader), "runner.card.stopping", None),
    RunnerState::Stopped => (Mark::Icon(Icon::Unplug), "runner.card.stopped", None),
  }
}

/// "2 of 4 agent slots in use", from what the runner last reported; that it has not reported yet
/// right after it came online.
fn slots(ctx: &Ctx, status: &RunnerStatus) -> Arc<str> {
  let Some(work) = status.work() else {
    return ctx.t("runner.card.slotsUnknown");
  };
  let (used, slots) = (work.agents_working().to_string(), work.agent_slots.to_string());
  let key = if work.agent_slots.get() == 1 {
    "runner.card.slots.one"
  } else {
    "runner.card.slots.other"
  };
  ctx.t_args(key, [("used", used.as_str()), ("slots", slots.as_str())])
}

/// "Attempt 2 of 5 · runs interrupted", or only the attempt when no run was interrupted.
fn attempt_line(ctx: &Ctx, attempt: u32, interrupted: usize) -> Arc<str> {
  let key = match interrupted {
    0 => "runner.card.attempt",
    1 => "runner.card.attemptInterrupted.one",
    _ => "runner.card.attemptInterrupted.other",
  };
  let (attempt, limit) = (attempt.to_string(), RECONNECT_LIMIT.to_string());
  ctx.t_args(key, [("attempt", attempt.as_str()), ("limit", limit.as_str())])
}

/// The state a test reads from the card.
fn state_name(state: &RunnerState) -> &'static str {
  match state {
    RunnerState::Starting => "starting",
    RunnerState::Online => "online",
    RunnerState::Reconnecting { .. } => "reconnecting",
    RunnerState::Stopping => "stopping",
    RunnerState::Stopped => "stopped",
    RunnerState::Failed { .. } => "offline",
  }
}
