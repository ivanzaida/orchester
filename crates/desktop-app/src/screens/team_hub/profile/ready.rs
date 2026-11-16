//! *Ready here?* (`Ready here?` `cLHSH` in `Agents · Profile` `NDZXC`): the parts the profile needs on this computer,
//! each ready or with what is missing and the fix at its row's end, under *N missing* in the color of the worst.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  fix::Offer,
  parts::{Part, State},
};
use crate::{
  screens::team_hub::{actions::Busy, rows},
  theme::{HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, Press, inert},
  },
};

/// The section of `parts` under the heading `title`, `missing` of which the readiness counts; `press` is what a press
/// on a part's fix does.
pub(in crate::screens) fn section<F: Offer>(
  ctx: &Ctx,
  (title, parts): (&str, &[Part<F>]),
  missing: usize,
  (press, busy): (&dyn Fn(&F) -> Press, Busy),
) -> Column {
  section_saying(ctx, (title, parts), counted(ctx, parts, missing), (press, busy))
}

/// What the section's count says: *all set* in `success`, or *N missing* in `warning` (`danger` when a part blocks).
pub(in crate::screens) fn counted<F>(ctx: &Ctx, parts: &[Part<F>], missing: usize) -> (Arc<str>, PaletteColor) {
  if missing == 0 {
    return (ctx.t("team_hub.readiness.allSet"), PaletteColor::Success);
  }
  let blocked = parts
    .iter()
    .any(|part| matches!(part.state, State::Missing { blocking: true, .. }));
  (
    ctx.t_args("team_hub.readiness.missing", [("count", missing.to_string())]),
    if blocked {
      PaletteColor::Danger
    } else {
      PaletteColor::Warning
    },
  )
}

/// [`section`] whose count says `words` in `color`.
pub(in crate::screens) fn section_saying<F: Offer>(
  ctx: &Ctx,
  (title, parts): (&str, &[Part<F>]),
  (words, color): (Arc<str>, PaletteColor),
  (press, busy): (&dyn Fn(&F) -> Press, Busy),
) -> Column {
  let count = Text::new(&words)
    .id("team-hub-readiness-count")
    .variant(HubText::Hint.role())
    .color(color)
    .nowrap();
  let rows = parts.iter().map(|part| part_row(ctx, part, (press, busy))).collect();
  rows::section("team-hub-readiness", title, vec![count.into()], rows)
}

fn part_row<F: Offer>(ctx: &Ctx, part: &Part<F>, (press, busy): (&dyn Fn(&F) -> Press, Busy)) -> Row {
  let id = format!("team-hub-ready-{}", part.id);
  let (glyph, color, words, fix, said) = match &part.state {
    State::Ready(words) => (
      Icon::CircleCheck,
      PaletteColor::Success,
      words,
      None,
      PaletteColor::TextSecondary,
    ),
    State::Missing { words, fix, blocking } => (
      if *blocking { Icon::CircleX } else { Icon::CircleAlert },
      if *blocking {
        PaletteColor::Danger
      } else {
        PaletteColor::Warning
      },
      words,
      fix.as_ref(),
      PaletteColor::TextPrimary,
    ),
  };
  let value = Row::new()
    .id(format!("{id}-value"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(words)
        .id(format!("{id}-words"))
        .variant(HubText::Status.role())
        .color(said)
        .flex(1.0),
    )
    .with_children(fix.map(|fix| link(ctx, fix, &id, (press(fix), busy))));
  rows::row(&id, &part.label, None, Element::from(value))
}

/// *Set path →*: the fix of a missing part, a link at its row's end; inert while a fix is taken or the hub is read
/// again.
fn link<F: Offer>(ctx: &Ctx, fix: &F, id: &str, (press, busy): (Press, Busy)) -> Button {
  let button = match busy {
    Busy::Idle => Focus::Link.apply(press.button()),
    Busy::Taking | Busy::Reading => inert(Button::empty().align_items(Alignment::Center)),
  };
  button.id(format!("{id}-{}", fix.name())).child(
    Text::new(&ctx.t(fix.link_key()))
      .variant(HubText::PanelLink.role())
      .nowrap(),
  )
}
