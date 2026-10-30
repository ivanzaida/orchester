//! What keeps a frame out of captures made for inspection (`lurq_screenshot`, a DevTools node screenshot): the page's
//! pixels hold whatever the page holds, and lurq paints over only sensitive text, by the box its glyphs reach within
//! their clip. So one glyph no one sees, scaled past any window, reaches the whole of the page, which clips it. It adds
//! nothing to what the person sees, nor to what automation reads but `sensitive=true` on the page. What the page draws
//! over the frame is covered with it: only the tree says what is there.

use lurq::{
  components::Text,
  node::{Element, HitTestBehavior, TextTransformMode, dimension::Dimension, transform::Transform2D},
};

use crate::theme::BrowserColor;

/// The glyph, for its box round the middle of its line: scaled from there, it grows past all four sides of the page.
const GLYPH: &str = "M";
/// Far past the largest window, from a glyph some pixels wide.
const SCALE: f32 = 4096.0;

/// The cover, for a stack that clips to the page. lurq rasterizes a bitmap-scaled glyph at four times its size at most.
pub fn cover() -> Element {
  Text::new(GLYPH)
    .id("browser-page-cover")
    .sensitive()
    .color(BrowserColor::PageCover.role())
    .nowrap()
    .text_transform_mode(TextTransformMode::Bitmap)
    .transform(Transform2D::scale_uniform(SCALE))
    .absolute(0.0, 0.0, Dimension::Auto, Dimension::Auto)
    .hit_test(HitTestBehavior::None)
    .into()
}
