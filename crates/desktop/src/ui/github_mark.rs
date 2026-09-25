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
/// Icon size in the row's icon surface (`eg6Nq`) and the new task's repository (`iuGdG`).
const MARK_SIZE: f32 = 18.0;
/// Icon size in the heading of the detected repository block (`s2kOs`).
const SMALL_MARK_SIZE: f32 = 16.0;

pub fn github_mark(ctx: &Ctx, color: PaletteColor) -> Element {
  mark(ctx, color, MARK_SIZE)
}

/// The mark beside a block heading.
pub fn small_github_mark(ctx: &Ctx, color: PaletteColor) -> Element {
  mark(ctx, color, SMALL_MARK_SIZE)
}

fn mark(ctx: &Ctx, color: PaletteColor, size: f32) -> Element {
  let stroke = ctx.theme().palette_color(color);
  Svg::new(SvgData::from_str(GITHUB_SVG).with_stroke(stroke))
    .size(Dimension::Px(size), Dimension::Px(size))
    .into()
}
