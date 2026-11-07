//! The person's watch command as the conversation keeps it (`Your command` `bZe0q` in `04` `OKtX2`): at the chat's end,
//! a bubble on `own_message`, radius 12, 10 by 14, its parts 6 apart, at most 560 wide: the command's chip and one chip
//! per target, 6 apart, then the order typed after them in 13/1.55 `text_primary`. Under it, 4 apart, *Applied 09:14*
//! as a message's delivery line. The app applied it itself: it started no turn, so it has no actions.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchestrator::{
    WatchTarget,
    watch::{NamedTarget, WatchCommand},
  },
  time::Timestamp,
};
use orchester_workspace::display;

use super::target_name;
use crate::{
  theme::{ExtraColor, ExtraSpacing, OrchestratorText},
  ui::{chat::delivery_line, icon::Icon, slash::sent_chip},
};

/// The widest the bubble grows (`Bubble` `Zgsjp`).
const BUBBLE_WIDTH: f32 = 560.0;

/// The bubble of `command`, kept at `at`, with its delivery line; `id` names it.
pub(super) fn bubble(ctx: &Ctx, id: &str, command: &WatchCommand, at: Option<Timestamp>) -> Element {
  let (name, glyph, targets, order) = match command {
    WatchCommand::Watch { targets, order }
      if targets.iter().all(|named| matches!(named.target, WatchTarget::Task(_))) =>
    {
      ("watch-task", Icon::ScanEye, targets.as_slice(), order.trim())
    }
    WatchCommand::Watch { targets, order } => ("watch-module", Icon::Eye, targets.as_slice(), order.trim()),
    WatchCommand::Unwatch { targets } => ("unwatch", Icon::EyeOff, targets.as_slice(), ""),
    WatchCommand::UnwatchAll => ("unwatch", Icon::EyeOff, &[][..], ""),
    // Its row is the answer from the live state (`super::super::watching`); this is only its bubble's chip.
    WatchCommand::Watching => ("watching", Icon::ListTree, &[][..], ""),
  };
  let chips = Row::new()
    .id(format!("{id}-chips"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(sent_chip(&format!("{id}-command"), glyph, &format!("/{name}")))
    .with_children(
      targets
        .iter()
        .enumerate()
        .map(|(index, named)| target_chip(ctx, &format!("{id}-target-{index}"), named)),
    );
  let order = (!order.is_empty()).then(|| {
    Text::new(order)
      .id(format!("{id}-order"))
      .variant(OrchestratorText::CommandOrder.role())
      .selectable(true)
  });
  let bubble = Column::new()
    .id(format!("{id}-bubble"))
    .max_width(Dimension::Px(BUBBLE_WIDTH))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Snug.role())
    .background(ExtraColor::OwnMessage.role())
    .rounded(RadiusSize::Lg)
    .child(chips)
    .with_children(order);
  let applied = at.map(|at| {
    let words = ctx.t_args("orchestrator.watch.applied", [("time", display::clock(at))]);
    delivery_line(
      &format!("{id}-delivery"),
      (Icon::CheckCheck, PaletteColor::Success),
      &words,
    )
  });
  Column::new()
    .id(id.to_owned())
    .describe("outcome", "command")
    .describe("command", name)
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::End)
    .child(bubble)
    .with_children(applied)
    .into()
}

/// The chip of `named`, a target of the command, named `id`: a module's `boxes`, a task's `square-check`.
fn target_chip(ctx: &Ctx, id: &str, named: &NamedTarget) -> Element {
  let glyph = match named.target {
    WatchTarget::Task(_) => Icon::SquareCheck,
    WatchTarget::Module(_) | WatchTarget::WithoutModule | WatchTarget::Project => Icon::Boxes,
  };
  sent_chip(id, glyph, &target_name(ctx, named))
}
