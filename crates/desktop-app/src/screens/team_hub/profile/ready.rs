//! *Ready here?* (`Ready here?` `cLHSH` in `Agents · Profile` `NDZXC`): the parts the profile needs on this computer,
//! each ready or with what is missing and the fix at its row's end, under *N missing* in the color of the worst.

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

/// The section of `parts`, `missing` of which the readiness counts; `press` is what a press on a part's fix does.
pub(in crate::screens) fn section<F: Offer>(
  ctx: &Ctx,
  parts: &[Part<F>],
  missing: usize,
  (press, busy): (&dyn Fn(&F) -> Press, Busy),
) -> Column {
  let blocked = parts
    .iter()
    .any(|part| matches!(part.state, State::Missing { blocking: true, .. }));
  let (words, color) = if missing == 0 {
    (ctx.t("team_hub.readiness.allSet"), PaletteColor::Success)
  } else {
    (
      ctx.t_args("team_hub.readiness.missing", [("count", missing.to_string())]),
      if blocked {
        PaletteColor::Danger
      } else {
        PaletteColor::Warning
      },
    )
  };
  let count = Text::new(&words)
    .id("team-hub-readiness-count")
    .variant(HubText::Hint.role())
    .color(color)
    .nowrap();
  let rows = parts.iter().map(|part| part_row(ctx, part, (press, busy))).collect();
  rows::section(
    "team-hub-readiness",
    &ctx.t("team_hub.readiness.title"),
    vec![count.into()],
    rows,
  )
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
