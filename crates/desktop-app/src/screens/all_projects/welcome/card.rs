//! A way in on the first start (`Way · Add a project from a folder` `PpEGM` in 01 `H9CZ1`): its glyph on a tile, its
//! title and what it is, and at its foot what it needs and its actions, on a `surface_raised` card inside a `border`
//! stroke. The three cards share the row's width equally and are as high as the row.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, StartupText},
  ui::icon::{Icon, icon},
};

/// The glyph's tile (`Glyph` `K58Ubi`).
const TILE: f32 = 36.0;
/// The row of the three ways (`Ways` `NXAOo`).
const HEIGHT: f32 = 300.0;

/// What a way's card shows.
pub(super) struct Way {
  /// The card's element id; its parts' ids start with it.
  pub id: &'static str,
  pub glyph: Icon,
  pub title: String,
  pub body: String,
  /// What the way needs, after its 12 glyph (`Meta` `IpneK`).
  pub meta: (Icon, String),
  /// Its buttons, and a badge after them when it has one (`Actions` `fDXLO`).
  pub actions: Vec<Element>,
}

pub(super) fn card(way: Way) -> Element {
  let Way {
    id,
    glyph,
    title,
    body,
    meta: (meta_glyph, meta),
    actions,
  } = way;
  let tile = Row::new()
    .width(Dimension::Px(TILE))
    .height(Dimension::Px(TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary));
  let text = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&title)
        .id(format!("{id}-title"))
        .variant(StartupText::CardTitle.role()),
    )
    .child(
      Text::new(&body)
        .id(format!("{id}-body"))
        .variant(StartupText::CardBody.role())
        .width(Dimension::Pct(100.0)),
    );
  let meta = Row::new()
    .id(format!("{id}-meta"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(meta_glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&meta).variant(StartupText::CardMeta.role()).nowrap());
  let actions = Row::new()
    .id(format!("{id}-actions"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(actions);
  Column::new()
    .id(id)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(HEIGHT))
    .padding(SpacingSize::Xl)
    .spacing(SpacingSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(tile)
    .child(text)
    .child(Spacer::new().flex(1.0))
    .child(meta)
    .child(actions)
    .into()
}
