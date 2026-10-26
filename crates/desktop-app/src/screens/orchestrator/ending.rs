//! How a turn ended (`4 · Report and the rest` `VZKyw`): while the Orchestrator answers, its one working line
//! (`Working` `o5Ak8`: a 13 loader in `text_secondary`, what it does in 12.5 `text_secondary`, and *Stop* as a ghost,
//! what stopping does in its tooltip); stopped or cut short, a centered divider (`Stopped` `LWHoS`: a `border` line at
//! each side of the words in 11.5 `text_muted` and *Ask again* in 12/500 `text_secondary`, which sends the person's
//! words once more); failed, the danger line with *Retry* ([`super::failure`]), or for an earlier turn a divider. A
//! turn a wait started has no words of the person's to send again, so it ends with its divider alone.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::{TurnFailure, TurnState};
use orchester_workspace::display;

use super::{
  failure::{earlier, failure_line},
  timeline::TurnRow,
};
use crate::{
  theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// The working line's height (`o5Ak8`).
const LINE_HEIGHT: f32 = 32.0;
/// A divider's lines' height (`EpB7m` in `LWHoS`).
const RULE_HEIGHT: f32 = 1.0;

/// How the turn ended, with what the person can do about the newest one; nothing for a wait that started no turn.
pub(super) fn ending(ctx: &mut Ctx, row: &TurnRow) -> Vec<Element> {
  let Some(state) = &row.turn.state else {
    return Vec::new();
  };
  match state {
    // A message that waits, or one taken by an answer no longer kept, has no ending of its own.
    TurnState::Answered | TurnState::Queued(_) | TurnState::Steering | TurnState::Joined => Vec::new(),
    TurnState::Thinking if row.last => vec![thinking(ctx, row)],
    TurnState::Thinking => Vec::new(),
    TurnState::Stopped => {
      let key = if row.turn.acted() {
        "orchestrator.stopped.acted"
      } else {
        "orchestrator.stopped.nothing"
      };
      // When it was stopped; a turn kept before the time was recorded says when it was sent.
      let time = row.turn.ended.or(row.turn.at).map(display::clock).unwrap_or_default();
      let words = ctx.t_args(key, [("time", time.as_str())]);
      let again = ask_again(ctx, row);
      vec![divider("orchestrator-stopped", &words, again)]
    }
    TurnState::Failed(TurnFailure::Interrupted) => {
      let again = ask_again(ctx, row);
      vec![divider(
        "orchestrator-interrupted",
        &ctx.t("orchestrator.interrupted"),
        again,
      )]
    }
    TurnState::Failed(failure) if row.last && !row.turn.on_its_own() => vec![failure_line(ctx, row, failure)],
    TurnState::Failed(failure) => vec![divider("orchestrator-failed", &earlier(ctx, failure), None)],
  }
}

/// The working line with *Stop*, what stopping does in its tooltip.
fn thinking(ctx: &mut Ctx, row: &TurnRow) -> Element {
  let stop = row.env.stop.clone();
  let mut button = ActionButton::new("orchestrator-stop", ButtonKind::Ghost, ctx.t("orchestrator.stop"))
    .icon(Icon::Square)
    .small();
  button = if row.stopping {
    button.loading()
  } else {
    button.on_click(move || stop())
  };
  let button = Tooltip::new("orchestrator-stop-note", ctx.t("orchestrator.stopNote")).wrap(ctx, button);
  Row::new()
    .id("orchestrator-thinking")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINE_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("orchestrator.thinking"))
        .variant(OrchestratorText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(button)
    .into()
}

/// *Ask again* for the newest turn a person's message started, what it does in its tooltip; `None` for any other.
/// *Ask again* sends the words once more, so a message with attached files, which it would send without them, does
/// not offer it: the person sends them again from the composer.
fn ask_again(ctx: &mut Ctx, row: &TurnRow) -> Option<Element> {
  if !row.last || row.turn.on_its_own() || !row.turn.attachments.is_empty() {
    return None;
  }
  let label: Arc<str> = ctx.t("orchestrator.askAgain");
  let words = Text::new(&label)
    .variant(TypographyStyle::Button)
    .color(PaletteColor::TextSecondary)
    .nowrap();
  let link = if row.can_ask {
    let (ask, said) = (row.env.ask_again.clone(), row.turn.text.clone());
    pressable(move || ask(said.clone()))
      .describe("state", "enabled")
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .cursor(CursorIcon::Pointer)
  } else {
    inert(lurq::components::Button::empty())
      .describe("state", "disabled")
      .opacity(DISABLED_OPACITY)
  };
  let link = link
    .id("orchestrator-ask-again")
    .describe("a11y_name", label)
    .rounded(ExtraRadius::Link.role())
    .child(words);
  Some(Tooltip::new("orchestrator-ask-again", ctx.t("orchestrator.askAgainNote")).wrap(ctx, link))
}

/// A centered divider named `id`: a `border` line at each side of `words` and, when there is one, `action`.
pub(super) fn divider(id: &str, words: &str, action: Option<Element>) -> Element {
  // A line 1 high (`EpB7m`): a row without height draws no inside stroke.
  let rule = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(RULE_HEIGHT))
      .background(PaletteColor::Border)
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(rule())
    .child(
      Text::new(words)
        .id(format!("{id}-label"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .with_children(action)
    .child(rule())
    .into()
}
