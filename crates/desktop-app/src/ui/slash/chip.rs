//! The skill chip a composer holds before its message (ORC-325; `Skill chip` `dBGRj` in `06d` `lXQi1`, `07c`
//! `f0b717`, `06c` `DunaJ`): a `graduation-cap` 12 in `text_secondary`, the skill's name in Geist Mono 11
//! `text_secondary` and a 12 `x` in `text_muted` that removes it, 4 apart, on a `surface_raised` pill in a `border`
//! stroke, 2 over and under them and 8 at their sides, 6 before the message's first line. Under the pointer a tooltip
//! names the skill and its published version (`FKUJD`). A skill cannot reach the agent with a message yet, so while
//! the chip holds one nothing is sent, and a quiet line under the box says why.

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Button, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::skills::offered::OfferedSkill;

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    tooltip::Tooltip,
  },
};

/// `field`, the composer's message field, after the chip of the skill `skill` holds while it holds one, in the
/// composer named `id`. The field keeps its place in the tree either way, and with it its focus.
pub fn with_skill_chip(ctx: &mut Ctx, id: &str, skill: &Signal<Option<OfferedSkill>>, field: Element) -> Element {
  let chip = skill_chip(ctx, id, skill);
  Row::new()
    .id(format!("{id}-skill-line"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .with_children(chip)
    .child(
      Row::new()
        .id(format!("{id}-message"))
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(field),
    )
    .into()
}

/// Under the box of the composer named `id`, while its chip holds a skill: that nothing is sent, and why.
pub fn send_held(ctx: &Ctx, id: &str) -> Element {
  Text::new(&ctx.t("composer.skill.sendHeld"))
    .id(format!("{id}-skill-held"))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}

/// The chip of the skill `skill` holds, in the composer named `id`, while it holds one; its `x` empties it.
fn skill_chip(ctx: &mut Ctx, id: &str, skill: &Signal<Option<OfferedSkill>>) -> Option<Element> {
  let chosen = skill.get()?;
  let chip_id = format!("{id}-skill-chip");
  let name = chosen.slug.clone();
  let remove_label = ctx.t_args("composer.skill.remove", [("name", &*name)]);
  let held = skill.clone();
  let remove = Button::empty()
    .id(format!("{chip_id}-remove"))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe("a11y_name", remove_label)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      held.set(None);
    })
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted));
  let chip = Row::new()
    .id(chip_id.clone())
    .describe("skill", name.clone())
    .describe("version", chosen.version.get().to_string())
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .child(icon(Icon::GraduationCap, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&name)
        .id(format!("{chip_id}-name"))
        .variant(SlashText::Chip.role())
        .nowrap(),
    )
    .child(remove);
  let tip = ctx.t_args(
    "composer.skill.tooltip",
    [
      ("name", name.to_string()),
      ("version", chosen.version.get().to_string()),
    ],
  );
  Some(Tooltip::new(chip_id, tip).wrap(ctx, chip))
}
