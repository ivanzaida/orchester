//! The lucide `github` icon (`eg6Nq` in the GitHub connection row `iYGKy`). The bundled lucide
//! font no longer carries brand icons, so the icon is the lucide SVG (ISC license, see
//! `assets/fonts/lucide-LICENSE.txt`) with its stroke in a theme color.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Svg,
  node::{Element, dimension::Dimension},
  svg::SvgData,
};

const GITHUB_SVG: &str = include_str!("../../assets/icons/github.svg");
/// Icon size in the row's icon surface (`eg6Nq`).
const MARK_SIZE: f32 = 18.0;

pub fn github_mark(ctx: &Ctx, color: PaletteColor) -> Element {
  let stroke = ctx.theme().palette_color(color);
  Svg::new(SvgData::from_str(GITHUB_SVG).with_stroke(stroke))
    .size(Dimension::Px(MARK_SIZE), Dimension::Px(MARK_SIZE))
    .into()
}
