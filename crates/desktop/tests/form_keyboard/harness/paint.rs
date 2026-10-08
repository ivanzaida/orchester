//! What the last headless pass painted: the fill under an element, and the theme it was painted with to compare it
//! with or to change.

use lurq::{
  app::theme::{PaletteColor, Theme},
  layout::quad::{Quad, QuadContent},
  node::color::Color,
};

use super::Desktop;

impl Desktop {
  /// The fill painted topmost at the middle of `id` in the last pass, text and strokes aside: its
  /// own fill, or what shows through where it paints none.
  pub fn fill_at(&mut self, id: &str) -> Option<Color> {
    let bounds = self.bounds(id);
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self
      .tree
      .painted_quads()
      .into_iter()
      .filter(|quad| covers(quad, x, y))
      .filter_map(|quad| match quad.content {
        QuadContent::Rect { color, .. } if color.a() > 0 => Some(color),
        _ => None,
      })
      .last()
  }

  /// `role` as the desktop's theme resolves it.
  pub fn color(&self, role: impl Into<PaletteColor>) -> Color {
    self.app.theme().palette_color(role)
  }

  /// The desktop's theme; a change to it reaches the window as a theme change does.
  pub fn theme(&self) -> &Theme {
    self.app.theme()
  }
}

/// Whether `quad` paints the point (`x`, `y`): inside its frame and its clip.
fn covers(quad: &Quad, x: f32, y: f32) -> bool {
  let inside =
    |left: f32, top: f32, width: f32, height: f32| x >= left && x <= left + width && y >= top && y <= top + height;
  let clip = &quad.clip;
  inside(quad.x, quad.y, quad.width, quad.height) && (!clip.active || inside(clip.x, clip.y, clip.width, clip.height))
}
