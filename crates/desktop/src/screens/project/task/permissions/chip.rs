//! The run's permissions as the composer's chip (`zQwSy` in `BRMpj`, `J` in `l7cEtA`): a shield and
//! what the step's agent may do in this run (*Edit files · Commands ask*), in `$muted`, or in
//! `$danger` once the permissions were changed for this run beyond its profile's; in a narrow
//! window the shield alone (`vOF1a`). Pressed, it opens the run's permissions popover.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::Text,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};

use super::{RunPermissionsProps, RunPermissionsRow};
use crate::{
  agents::permissions::{Length, parts},
  theme::{ExtraRadius, ExtraSpacing},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// The chip's height (`zQwSy`).
const CHIP_HEIGHT: f32 = 28.0;

impl RunPermissionsRow {
  /// The chip; `short` to its shield alone.
  pub(super) fn chip(&self, ctx: &Ctx, props: &RunPermissionsProps, short: bool) -> Element {
    let permissions = &props.permissions;
    let tone = if permissions.changed {
      PaletteColor::Danger
    } else {
      PaletteColor::TextSecondary
    };
    let (repository, commands, _) = parts(
      ctx,
      &permissions.permissions,
      permissions.network_stays_on,
      Length::Compact,
    );
    let words = ctx.t_args(
      "runs.permissions.chip",
      [("repository", &*repository), ("commands", &*commands)],
    );
    let (open, edit, base) = (self.open.clone(), self.edit.clone(), permissions.base.clone());
    let chip = pressable(surface().id("conversation-permissions"), move || {
      edit.open(&base);
      open.set(true);
    })
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .rounded(ExtraRadius::Round.role())
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .ref_element(self.anchor.clone())
    .describe("changed", permissions.changed.to_string())
    .child(icon(Icon::Shield, IconSize::Entry, tone.clone()));
    if short {
      return chip.into();
    }
    chip
      .child(
        Text::new(&words)
          .variant(TypographyStyle::FieldLabel)
          .color(tone)
          .nowrap(),
      )
      .into()
  }
}
