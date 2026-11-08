//! The badges of the commands a composer holds, pinned in a row of their own at the top of its box (`Badges` `FBanv`
//! in P2 `WbHSt` of `Proposal · watch composer v2` `LRg3R`): 6 apart, wrapping when they don't fit (P6 `H9acpg`), and
//! no row while none is held. A badge (`Badge · Watch · Orchestrator watch` `VvRWU`) is one sentence: the command's 12
//! icon and verb in 12/500 `text_secondary`, a `·` in `text_muted`, its first two choices in 12 `text_primary`, then
//! *+N* for the rest on a round `hover` pill in 11 `text_secondary` (P3 `iIfbM`), and a 12 `x` in `text_muted` that
//! removes the badge; 5 apart on a round `surface_raised` pill in a `border_strong` stroke, 3 over and under, 9 before
//! and 6 after. A press on it opens its picker again.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor},
  },
  components::{Button, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::Arguing;
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    slash::SlashField,
  },
};

/// How many choices a badge names before *+N*.
const NAMED: usize = 2;

/// The row of the badges the composer named `id` holds, `{id}-badges`; `None` while it holds none.
pub(in crate::ui::slash) fn badges(ctx: &Ctx, id: &str, field: &SlashField) -> Option<Element> {
  let held = field.argument.get();
  if held.is_empty() {
    return None;
  }
  Some(
    Row::new()
      .id(format!("{id}-badges"))
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(held.iter().map(|badge| one(ctx, id, field, badge)))
      .into(),
  )
}

/// The badge of `badge`, `{id}-badge-{command}`: a press opens its picker again, its `x` removes it.
fn one(ctx: &Ctx, id: &str, field: &SlashField, badge: &Arguing) -> Element {
  let badge_id = format!("{id}-badge-{}", badge.command);
  let name = format!("/{}", badge.command);
  let reopen = (field.clone(), badge.command);
  let pill = Row::new()
    .id(format!("{badge_id}-pill"))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Fine.role())
        .bottom(ExtraSpacing::Fine.role())
        .left(ExtraSpacing::ChipStart.role())
        .right(ExtraSpacing::Snug.role()),
    )
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Full.role())
    .child(icon(badge.badge.icon, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&badge.badge.verb)
        .id(format!("{badge_id}-verb"))
        .variant(SlashText::BadgeVerb.role())
        .nowrap(),
    )
    .with_children(targets(ctx, &badge_id, badge))
    .child(remove(ctx, (&badge_id, field), badge));
  pressable(move || reopen.0.set_picking(Some(reopen.1)))
    .id(badge_id)
    .describe(
      "a11y_name",
      ctx.t_args("composer.argument.choose", [("command", &*name)]),
    )
    .describe("command", badge.command)
    .describe("picking", badge.picking.to_string())
    .describe("choices", badge.choices.len().to_string())
    .flex_shrink(0.0)
    .child(pill)
    .into()
}

/// What follows the verb while the badge holds a choice: a `·`, the first [`NAMED`] choices, and *+N* for the rest.
fn targets(ctx: &Ctx, badge_id: &str, badge: &Arguing) -> Vec<Element> {
  let labels: Vec<&Arc<str>> = badge.choices.iter().map(|choice| &choice.label).collect();
  let named = match labels.as_slice() {
    [] => return Vec::new(),
    [one] => (*one).clone(),
    [first, second, ..] => ctx.t_args("composer.argument.pair", [("first", &***first), ("second", &***second)]),
  };
  let mut parts: Vec<Element> = vec![
    Text::new(&ctx.t("composer.argument.dot"))
      .variant(SlashText::BadgeTarget.role())
      .color(PaletteColor::TextMuted)
      .nowrap()
      .into(),
    Text::new(&named)
      .id(format!("{badge_id}-targets"))
      .variant(SlashText::BadgeTarget.role())
      .nowrap()
      .into(),
  ];
  let rest = labels.len().saturating_sub(NAMED);
  if rest > 0 {
    parts.push(
      Row::new()
        .id(format!("{badge_id}-more"))
        .padding(Padding::horizontal(ExtraSpacing::Pair.role()))
        .background(ExtraColor::Hover.role())
        .rounded(ExtraRadius::Full.role())
        .child(
          Text::new(&ctx.t_args("composer.argument.more", [("count", rest.to_string())]))
            .variant(SlashText::BadgeMore.role())
            .nowrap(),
        )
        .into(),
    );
  }
  parts
}

/// The badge's `x`, `{badge_id}-remove`.
fn remove(ctx: &Ctx, (badge_id, field): (&str, &SlashField), badge: &Arguing) -> Button {
  let (field, command) = (field.clone(), badge.command);
  Button::empty()
    .id(format!("{badge_id}-remove"))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe(
      "a11y_name",
      ctx.t_args("composer.argument.remove", [("name", &*badge.badge.verb)]),
    )
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      field.remove_badge(command);
    })
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted))
}
