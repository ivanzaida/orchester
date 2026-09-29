//! The 1px dots at 24px intervals on the pipeline canvas (`C6Bjps`). A single cached SVG tile
//! keeps the background independent of graph pan/zoom without creating thousands of UI nodes.

use std::{fmt::Write, sync::OnceLock};

use lurq::{node::color::Color, svg::SvgData};

const PITCH: usize = 24;
const OFFSET: usize = 20;
const DOT: usize = 1;
const DOTS_PER_SIDE: usize = 42;
pub const CANVAS_GRID_TILE: f32 = (PITCH * DOTS_PER_SIDE) as f32;

static GRID: OnceLock<SvgData> = OnceLock::new();

pub fn canvas_grid(color: Color) -> SvgData {
  GRID.get_or_init(tile).clone().with_fill(color)
}

fn tile() -> SvgData {
  let size = PITCH * DOTS_PER_SIDE;
  let mut svg = format!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 {size} {size}\"><path d=\""
  );
  for row in 0..DOTS_PER_SIDE {
    for column in 0..DOTS_PER_SIDE {
      let x = OFFSET + column * PITCH;
      let y = OFFSET + row * PITCH;
      write!(svg, "M{x} {y}h{DOT}v{DOT}h-{DOT}z ").expect("writing to String cannot fail");
    }
  }
  svg.push_str("\"/></svg>");
  SvgData::from_str(&svg)
}
