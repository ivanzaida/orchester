//! The two ways into a team side by side (`Columns` `ER5j6` in `B914L`): Orchester Cloud on the left, a code on the
//! right, each in a card of its own under a glyph tile, its title and what it is for. While the Cloud column signs in,
//! the code column waits, faded (`BD519`).

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, IconSize, StartupText},
  ui::icon::{Icon, icon},
};

/// A glyph tile at the start of a way's head and of a team's or an invite's row (`Glyph` `lsoj3`).
const TILE: f32 = 32.0;

/// The Cloud column and the code column, the code column faded while `waiting`.
pub(super) fn columns(cloud: Element, code: Element, waiting: bool) -> Element {
  let code = Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(code);
  Row::new()
    .id("join-team-ways")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(cloud))
    .child(if waiting { code.opacity(DISABLED_OPACITY) } else { code })
    .into()
}

/// A way's card, `id`, with its head over `parts`.
pub(super) fn way(id: &str, head: Element, parts: Vec<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Xl)
    .spacing(SpacingSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(head)
    .with_children(parts)
    .into()
}

/// A way's head: its glyph on a tile, its title and what it is for.
pub(super) fn head(id: &str, glyph: Icon, (title, sub): (&str, &str)) -> Element {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(tile(glyph))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(StartupText::WayTitle.role()),
        )
        .child(
          Text::new(sub)
            .id(format!("{id}-sub"))
            .variant(StartupText::Sub.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// `glyph` in 16 `text_secondary` on a 32 tile, `surface_raised` inside a `border` stroke, radius 8.
pub(super) fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(TILE))
    .height(Dimension::Px(TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextSecondary))
}
