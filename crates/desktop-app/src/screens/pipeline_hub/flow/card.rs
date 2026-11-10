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
use orchester_workspace::pipelines::flow::{DeliverCard, StepCard};

use super::words;
use crate::{
  screens::hub::agent_row::tag_chip,
  theme::{ControlHeight, ExtraColor, ExtraSpacing, FlowText, IconSize},
  ui::icon::{Icon, icon},
};

/// Where a card sits, which decides whether its number stands in front of its name: inside a parallel group the steps
/// have no node of their own.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
  OnTheLine,
  InGroup,
}

/// The card of `step`; `open` is the button that opens the step.
pub(super) fn step(ctx: &lurq::app::ctx::Ctx, step: &StepCard, (open, place): (Button, Place)) -> Element {
  let id = format!("pipeline-hub-flow-step-{}", step.key);
  let not_ready = step.runs.needs_you();
  let mut top = Row::new()
    .id(format!("{id}-top"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  if place == Place::InGroup {
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
    .with_children(
      step
        .access
        .filter(|_| !not_ready)
        .map(|access| tag_chip(&format!("{id}-access"), &words::access(ctx, access))),
    )
    .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
  let mut body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(top)
    .child(runs_line(ctx, &id, step, not_ready));
  if let Some(route) = &step.on_failure {
    body = body.child(
      Row::new()
        .id(format!("{id}-failure"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&words::failure(ctx, route))
            .id(format!("{id}-failure-words"))
            .variant(FlowText::Line.role())
            .color(PaletteColor::TextMuted),
        ),
    );
  }
  let open = if place == Place::InGroup {
    open.height(Dimension::Pct(100.0))
  } else {
    open
  };
  surface(open, &id, not_ready, body)
    .describe("step", step.key.to_string())
    .describe("number", words::number(step.number))
    .describe("state", if not_ready { "not_ready" } else { "ready" })
    .into()
}

/// The Deliver card, numbered after the steps.
pub(super) fn deliver(ctx: &lurq::app::ctx::Ctx, deliver: &DeliverCard, open: Button) -> Element {
  let id = "pipeline-hub-flow-deliver";
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(name(id, &ctx.t("pipeline_hub.flow.deliver")))
    .child(Row::new().flex(1.0))
    .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(top)
    .child(
      Text::new(&words::deliver(ctx, deliver))
        .id(format!("{id}-line"))
        .variant(FlowText::Line.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0)),
    );
  surface(open, id, false, body)
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

/// What the card runs, or in `warning` with a glyph what keeps the step from running.
fn runs_line(ctx: &lurq::app::ctx::Ctx, id: &str, step: &StepCard, not_ready: bool) -> Element {
  let words = words::runs(ctx, &step.runs);
  let line = Text::new(&words)
    .id(format!("{id}-runs"))
    .variant(FlowText::Line.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  if !not_ready {
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
/// above and under; a step that is not ready in a `warning` outline.
fn surface(open: Button, id: &str, not_ready: bool, body: Column) -> Button {
  let outline = if not_ready {
    PaletteColor::Warning
  } else {
    PaletteColor::Border
  };
  open
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfacePanel)
    .border_inside(BorderSize::Sm, outline)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()).border(Border::inside(
      BorderSize::Sm,
      if not_ready {
        PaletteColor::Warning
      } else {
        PaletteColor::BorderStrong
      },
    )))
    .child(body)
}
