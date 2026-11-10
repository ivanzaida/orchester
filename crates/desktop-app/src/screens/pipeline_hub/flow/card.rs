//! A card of the flow (`Card · 01 Research` `uxj4K`, `Card · 04 Run checks` `Iv58p` in `Steps` `uSoll`; the amber `Card
//! · 02 Review` `zLMvF` in P8): the name, what the step runs, a pill for what it may do to the repository and `›`. A
//! card that sends the run back says where. Under the pointer it takes the `hover` fill and a `border_strong` outline;
//! a press opens the step. The step that cannot run as it is stands in `warning`, with *Choose profile* when nothing is
//! chosen yet.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Button, Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pipelines::flow::{Conflicts, DeliverCard, ReturnsTo, StepCard};

use super::words;
use crate::{
  screens::hub::agent_row::tag_chip,
  theme::{ControlHeight, DRAGGED_OPACITY, ExtraColor, ExtraSpacing, FlowText, IconSize},
  ui::icon::{Icon, icon},
};

/// Where a card sits, which decides whether its number stands in front of its name: inside a parallel group the steps
/// have no node of their own.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
  OnTheLine,
  InGroup,
}

/// How a card looks while a card is dragged: faded where it was lifted from (`Card · 05 Review` `s1SIUj` in P3), or
/// outlined in `accent` where dropping makes the two run in parallel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
  Plain,
  Lifted,
  Target,
}

/// A hovered card's handle (`Drag handle` `yylYY`) takes 14 and 6 of gap before its name, and its second line is
/// indented to stand under the name (`Line row` `aB9FX`).
const HANDLE_INDENT: f32 = 20.0;
/// What a card with a handle has at its start, where one without has `md` (`Card · 03 Implement · hover` `GnnMB`).
const HANDLE_START: f32 = 6.0;

/// How a card is pressed and where it stands: `open` opens the step, and is `None` for a card that only shows its step
/// (a published version read-only, which has no step to open and no hover, `›`, handle or ⋯); `fix` is the pill of a
/// step that cannot run, which opens what it names. A card the pointer or the keys are on has its `handle` and its ⋯
/// (`more`).
pub(super) struct Opening {
  pub open: Option<Button>,
  pub fix: Option<(Button, std::sync::Arc<str>)>,
  pub place: Place,
  pub handle: Option<Element>,
  pub more: Option<Element>,
  pub look: Look,
}

/// The card of `step`.
pub(super) fn step(ctx: &lurq::app::ctx::Ctx, step: &StepCard, opening: Opening) -> Element {
  let Opening {
    open,
    fix,
    place,
    handle,
    more,
    look,
  } = opening;
  let handled = handle.is_some();
  let id = format!("pipeline-hub-flow-step-{}", step.key);
  let not_ready = step.runs.needs_you();
  let mut top = Row::new()
    .id(format!("{id}-top"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  top = top.with_children(handle);
  if place == Place::InGroup {
    // A card without a pill keeps the pill row's height, so the titles of a group's cards share one baseline.
    top = top.min_height(Dimension::Px(ControlHeight::Helper.value()));
    top = top.child(
      Text::new(&words::number(step.number))
        .id(format!("{id}-number"))
        .variant(FlowText::CardNumber.role()),
    );
  }
  top = top
    .child(name(&id, &step.name))
    .child(Row::new().flex(1.0))
    .with_children(words::choose(ctx, &step.runs).map(|choose| choose_chip(&id, &choose)))
    .with_children(fix.map(|(button, label)| {
      button
        .id(format!("{id}-fix"))
        .child(choose_chip(&format!("{id}-fix"), &label))
    }))
    .with_children(
      step
        .access
        .filter(|_| !not_ready)
        .map(|access| tag_chip(&format!("{id}-access"), &words::access(ctx, access))),
    )
    .with_children(more)
    .with_children(
      open
        .is_some()
        .then(|| icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)),
    );
  let indent = if handled { HANDLE_INDENT } else { 0.0 };
  let mut body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(top)
    .child(indented(
      second_line(&id, "runs", &words::runs(ctx, &step.runs), not_ready),
      handled,
    ));
  if let Some(route) = &step.on_failure {
    body = body.child(failure_line(ctx, (&id, step, route), indent));
  }
  let open = open.map(|open| {
    if place == Place::InGroup {
      open.height(Dimension::Pct(100.0))
    } else {
      open
    }
  });
  surface(open, &id, (not_ready, handled, look), body)
    .describe("step", step.key.to_string())
    .describe("number", words::number(step.number))
    .describe("state", if not_ready { "not_ready" } else { "ready" })
    .into()
}

/// Where a failure sends the run, under what the card runs: *↵ changes requested → 03*.
fn failure_line(ctx: &lurq::app::ctx::Ctx, (id, step, route): (&str, &StepCard, &ReturnsTo), indent: f32) -> Row {
  Row::new()
    .id(format!("{id}-failure"))
    .padding(Padding::new().left(Dimension::Px(indent)))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words::failure(ctx, step, route))
        .id(format!("{id}-failure-words"))
        .variant(FlowText::Line.role())
        .color(PaletteColor::TextMuted),
    )
}

/// `line`, standing under the name of a card that has its handle.
fn indented(line: Element, handled: bool) -> Element {
  if !handled {
    return line;
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(Dimension::Px(HANDLE_INDENT)))
    .child(line)
    .into()
}

/// The Deliver card, numbered after the steps.
pub(super) fn deliver(ctx: &lurq::app::ctx::Ctx, deliver: &DeliverCard, open: Option<Button>) -> Element {
  let id = "pipeline-hub-flow-deliver";
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(name(id, &ctx.t("pipeline_hub.flow.deliver")))
    .child(Row::new().flex(1.0))
    .with_children(
      open
        .is_some()
        .then(|| icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)),
    );
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(top)
    .child(second_line(
      id,
      "line",
      &words::deliver(ctx, deliver),
      matches!(deliver.conflicts, Conflicts::ProfileNotPublished(_)),
    ));
  surface(open, id, (false, false, Look::Plain), body)
    .describe("number", words::number(deliver.number))
    .into()
}

fn name(id: &str, name: &str) -> Text {
  Text::new(name)
    .id(format!("{id}-name"))
    .variant(FlowText::Name.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0))
}

/// A card's second line: what it runs, or how it merges. In `warning` with a glyph when something keeps it from
/// working.
fn second_line(id: &str, part: &str, words: &str, warning: bool) -> Element {
  let line = Text::new(words)
    .id(format!("{id}-{part}"))
    .variant(FlowText::Line.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  if !warning {
    return line.into();
  }
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Warning))
    .child(line.color(PaletteColor::Warning))
    .into()
}

/// *Choose profile* (`Choose` `fzH6p`): a `warning_muted` chip, 22 high, radius 4, 8 at its sides. It is part of the
/// card, which opens the step where it is chosen.
fn choose_chip(id: &str, words: &str) -> Row {
  Row::new()
    .id(format!("{id}-choose"))
    .height(Dimension::Px(ControlHeight::Helper.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .background(PaletteColor::WarningMuted)
    .child(
      Text::new(words)
        .id(format!("{id}-choose-words"))
        .variant(FlowText::Hint.role())
        .color(PaletteColor::Warning)
        .nowrap(),
    )
}

/// A card's frame (`Card · 01 Research`): `surface_panel` in a `border` outline, radius 6, 12 at its sides and 8
/// above and under; a step that is not ready in a `warning` outline. Pressed through `open`, it takes the hover fill;
/// without one it only shows its step.
fn surface(open: Option<Button>, id: &str, (not_ready, handled, look): (bool, bool, Look), body: Column) -> Button {
  let outline = match (look, not_ready) {
    (Look::Target, _) => Border::inside(BorderSize::Lg, PaletteColor::Accent),
    (_, true) => Border::inside(BorderSize::Sm, PaletteColor::Warning),
    (_, false) => Border::inside(BorderSize::Sm, PaletteColor::Border),
  };
  let hovered = match (look, not_ready) {
    (Look::Target, _) => outline.clone(),
    (_, true) => Border::inside(BorderSize::Sm, PaletteColor::Warning),
    (_, false) => Border::inside(BorderSize::Sm, PaletteColor::BorderStrong),
  };
  let padding = if handled {
    Padding::new()
      .top(SpacingSize::Sm)
      .bottom(SpacingSize::Sm)
      .right(SpacingSize::Md)
      .left(Dimension::Px(HANDLE_START))
  } else {
    Padding::symmetric(SpacingSize::Md, SpacingSize::Sm)
  };
  let pressable = open.is_some();
  let card = open
    .unwrap_or_else(Button::empty)
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(padding)
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfacePanel)
    .border(outline)
    .rounded(RadiusSize::Md);
  // A card that only shows its step does nothing, so the keyboard does not stop on it.
  let card = if pressable {
    card.hovered_style(Style::new().background(ExtraColor::Hover.role()).border(hovered))
  } else {
    card.focusable(false)
  }
  .child(body);
  if look == Look::Lifted {
    card.opacity(DRAGGED_OPACITY)
  } else {
    card
  }
}
