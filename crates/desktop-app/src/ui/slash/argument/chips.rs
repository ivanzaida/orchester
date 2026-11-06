//! The chips of a command with an argument before the message (`Chip · /watch-module` `i5HmI` and `Chip · Webhooks v2`
//! `i0tO8` in `02` `c8pALl`): the command's own, its 11 icon and its `/name` in Geist Mono 11 `text_secondary`, which a
//! press opens the picker again with; then each choice taken, its 11 icon, its label in the same words and an 11 `x`
//! in `text_muted` that removes it; each on the skill chip's `surface_raised` pill, in a `border_strong` stroke.

use lurq::{
  app::{ctx::Ctx, events::mouse_event::MouseEvent, theme::PaletteColor},
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Element},
};

use super::{Arguing, Choice};
use crate::{
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    slash::{
      SlashField,
      chip::{chip_words, pill_frame},
    },
  },
};

/// The chips of the command the composer named `id` holds, while it holds one.
pub(in crate::ui::slash) fn chips(ctx: &Ctx, id: &str, field: &SlashField) -> Vec<Element> {
  let Some(held) = field.argument.get() else {
    return Vec::new();
  };
  let mut chips = vec![command_chip(ctx, id, field, &held)];
  chips.extend(held.choices.iter().map(|choice| choice_chip(ctx, id, field, choice)));
  chips
}

/// The command's chip, `{id}-command-chip`: a press opens its picker again.
fn command_chip(ctx: &Ctx, id: &str, field: &SlashField, held: &Arguing) -> Element {
  let chip_id = format!("{id}-command-chip");
  let name = format!("/{}", held.command);
  let reopen = field.clone();
  pressable(move || reopen.set_picking(true))
    .id(chip_id.clone())
    .describe(
      "a11y_name",
      ctx.t_args("composer.argument.choose", [("command", &*name)]),
    )
    .describe("command", held.command)
    .describe("picking", held.picking.to_string())
    .flex_shrink(0.0)
    .child(
      pill_frame(&format!("{chip_id}-pill"), PaletteColor::BorderStrong)
        .child(icon(held.icon, IconSize::Xs, PaletteColor::TextSecondary))
        .child(chip_words(&format!("{chip_id}-name"), &name)),
    )
    .into()
}

/// The chip of `choice`, `{id}-choice-{key}`, whose `x` removes it.
fn choice_chip(ctx: &Ctx, id: &str, field: &SlashField, choice: &Choice) -> Element {
  let chip_id = format!("{id}-choice-{}", choice.key);
  let (field, key) = (field.clone(), choice.key.clone());
  let remove = Button::empty()
    .id(format!("{chip_id}-remove"))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe(
      "a11y_name",
      ctx.t_args("composer.argument.remove", [("name", &*choice.label)]),
    )
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      field.remove_choice(&key);
    })
    .child(icon(Icon::X, IconSize::Xs, PaletteColor::TextMuted));
  pill_frame(&chip_id, PaletteColor::BorderStrong)
    .describe("choice", choice.key.clone())
    .child(icon(choice.icon, IconSize::Xs, PaletteColor::TextSecondary))
    .child(chip_words(&format!("{chip_id}-name"), &choice.label))
    .child(remove)
    .into()
}
