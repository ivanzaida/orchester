//! The GitHub mark (lucide `github`, the glyph of the GitHub repository block `VCpFK` in `kkGWj`). The bundled
//! lucide font no longer carries brand icons, so the mark is lucide's SVG (ISC license, see
//! `assets/fonts/lucide-LICENSE.txt`), its stroke in a theme color, as large as an icon of `size`.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};

use crate::theme::IconSize;

const GITHUB_SVG: &str = include_str!("../../assets/icons/github.svg");

pub fn github_mark(ctx: &Ctx, size: IconSize, color: PaletteColor) -> Element {
  let stroke = ctx.theme().palette_color(color);
  let side = Dimension::Px(size.size());
  Svg::new(SvgData::from_str(GITHUB_SVG).with_stroke(stroke))
    .size(side, side)
    .into()
}
