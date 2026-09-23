//! The open project's runner in the shell: the sidebar card (`O2Jjjf` in `zo9Py`), the
//! offline panel (`nW2Yu` in `y8TKW8`, a recovery panel `iN90G`) and the reconnect notices.
//! Agent slots do not exist yet, so the card shows no capacity.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  messages,
  runner::{
    Runners,
    state::{RECONNECT_LIMIT, RunnerFailure, RunnerState},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, message_text, notice::notice, panel::RecoveryPanel},
};

/// Runner indicator size (`fK4mR`).
const INDICATOR_SIZE: f32 = 5.0;

/// Sidebar runner card (`O2Jjjf`).
pub fn card(ctx: &Ctx, state: &RunnerState) -> Element {
  let (title, detail, indicator) = match state {
    RunnerState::Starting => ("runner.starting", "runner.startingDetail", PaletteColor::TextMuted),
    RunnerState::Online { .. } => ("runner.online", "runner.onlineDetail", PaletteColor::TextPrimary),
    RunnerState::Reconnecting { .. } => (
      "runner.reconnecting",
      "runner.reconnectingDetail",
      PaletteColor::TextMuted,
    ),
    RunnerState::Stopping => ("runner.stopping", "runner.stoppingDetail", PaletteColor::TextMuted),
    RunnerState::Stopped | RunnerState::Failed(_) => {
      ("runner.offline", "runner.offlineDetail", PaletteColor::TextMuted)
    }
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
        .child(
          Rect::new(Dimension::Px(INDICATOR_SIZE), Dimension::Px(INDICATOR_SIZE))
            .background(indicator)
            .rounded(RadiusSize::Lg),
        )
        .child(
          Text::new(&ctx.t(title))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        ),
    )
    .child(Text::new(&ctx.t(detail)).variant(ExtraTypography::Footnote.role()))
    .into()
}

/// For tooling (`lurq_read_tree`): the runner state without reading the copy.
fn state_name(state: &RunnerState) -> &'static str {
  match state {
    RunnerState::Starting => "starting",
    RunnerState::Online { .. } => "online",
    RunnerState::Reconnecting { .. } => "reconnecting",
    RunnerState::Stopping => "stopping",
    RunnerState::Stopped => "stopped",
    RunnerState::Failed(_) => "failed",
  }
}

/// The notice above the overview while the runner reconnects or just came back (placeholder
/// presentation with the inline notice `q5LzI` until the notices are designed).
pub fn reconnect_notice(ctx: &Ctx, state: &RunnerState) -> Option<Element> {
  match state {
    RunnerState::Reconnecting { attempt, .. } => {
      let attempt = attempt.to_string();
      let limit = RECONNECT_LIMIT.to_string();
      let detail = ctx.t_args(
        "runner.droppedDetail",
        [("attempt", attempt.as_str()), ("limit", limit.as_str())],
      );
      Some(notice(Icon::Unplug, &ctx.t("runner.droppedTitle"), &detail))
    }
    RunnerState::Online { reconnected: true } => Some(notice(
      Icon::Plug,
      &ctx.t("runner.reconnectedTitle"),
      &ctx.t("runner.reconnectedDetail"),
    )),
    _ => None,
  }
}

/// The offline panel (`nW2Yu`) with the restart action, when the runner is not running and
/// no longer retrying.
pub fn offline_panel(ctx: &Ctx, state: &RunnerState, runners: &Runners) -> Option<Element> {
  let explanation: Arc<str> = match state {
    RunnerState::Failed(failure) => message_text(ctx, &messages::runner_failure(failure)),
    RunnerState::Stopped => ctx.t("runner.stoppedDetail"),
    _ => return None,
  };
  let runners = runners.clone();
  let restart = ActionButton::new(ctx.t("runner.restart"))
    .icon(Icon::RefreshCw)
    .id("runner-restart")
    .on_click(move || runners.restart())
    .render(ctx);
  let mut panel = RecoveryPanel::new(Icon::Unplug, &ctx.t("runner.offlineTitle"), &explanation).action(restart);
  if let RunnerState::Failed(RunnerFailure::MissingExecutable { path }) = state {
    panel = panel.detail(
      Text::new(&path.display().to_string())
        .variant(TypographyStyle::Caption)
        .selectable(true),
    );
  }
  Some(panel.into())
}
