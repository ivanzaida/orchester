//! A secret in the list (`vxAMD`): its kind's icon, its name in mono, the project, environment
//! and kind, how many tools use it, and a chevron. The shown secret's row is raised.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::secret::Secret;

use super::labels::{environment_key, kind_icon, kind_key};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    link::Link,
    pressable::surface,
    status_label::status_label,
  },
};

/// Row height (`vxAMD`).
const ROW_HEIGHT: f32 = 84.0;

pub struct SecretRow<'a> {
  pub secret: &'a Secret,
  pub project: &'a str,
  /// Tools that use the secret.
  pub uses: usize,
  pub selected: bool,
}

pub fn secret_row(ctx: &Ctx, row: &SecretRow, link: Link) -> Element {
  let secret = row.secret;
  let scope = ctx.t_args(
    "secrets.row.scope",
    [
      ("project", row.project),
      ("environment", &*ctx.t(environment_key(secret.environment()))),
      ("kind", &*ctx.t(kind_key(secret.kind()))),
    ],
  );
  let (usage_icon, usage) = match row.uses {
    0 => (Icon::Minus, ctx.t("secrets.usage.none")),
    1 => (Icon::Link, ctx.t("secrets.usage.one")),
    count => (
      Icon::Link,
      ctx.t_args("secrets.usage.other", [("count", count.to_string().as_str())]),
    ),
  };
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(secret.name().as_str())
        .variant(ExtraTypography::MonoTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&scope)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let mut element = surface()
    .id(Arc::<str>::from(format!("secret-{}", secret.id())))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_vertical(ExtraSpacing::Group.role())
    .padding_horizontal(SpacingSize::Md)
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(
      kind_icon(secret.kind()),
      IconSize::Feature,
      PaletteColor::TextSecondary,
    ))
    .child(identity)
    .child(status_label(usage_icon, &usage))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  // Rows are outside a form, so they are pointer-only: no Tab stop and no focus ring.
  if row.selected {
    element = element
      .background(PaletteColor::AccentMuted)
      .rounded(ExtraRadius::Notice.role());
    return element.describe("state", "selected").into();
  }
  link
    .on(element.background(ExtraColor::Surface.role()))
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .into()
}
