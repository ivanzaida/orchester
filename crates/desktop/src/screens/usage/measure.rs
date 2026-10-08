//! Fixed column widths measured with the theme's fonts, for a table laid out narrower than the
//! design draws it: each column as wide as its heading or its widest cell, found once per table and
//! given to the heading row and every row alike. lurq measures text only through a canvas, so the
//! table keeps one empty, zero-size canvas to measure with; it draws nothing and holds no text.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  canvas::{CanvasFont, CanvasHandle},
  components::Canvas,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use crate::theme::chart::canvas_font;

/// A text in the font it is drawn in.
#[derive(Clone, PartialEq)]
pub struct Piece {
  text: Arc<str>,
  font: CanvasFont,
}

impl Piece {
  pub fn new(ctx: &Ctx, text: Arc<str>, style: impl Into<TypographyStyle>) -> Self {
    Self {
      text,
      font: canvas_font(&ctx.theme().typography_style(style)),
    }
  }
}

/// What a cell shows: texts side by side, and the room around and between them (an outcome label's
/// padding and the gap after its icon).
#[derive(Clone, PartialEq)]
pub struct Shape {
  pieces: Vec<Piece>,
  room: f32,
}

impl Shape {
  pub fn new(pieces: Vec<Piece>, room: f32) -> Self {
    Self { pieces, room }
  }

  pub fn text(piece: Piece) -> Self {
    Self::new(vec![piece], 0.0)
  }

  fn width(&self, canvas: &CanvasHandle) -> Option<f32> {
    let context = canvas.context_2d();
    let mut width = self.room;
    for piece in &self.pieces {
      context.set_font(piece.font.clone());
      width += match context.measure_text(&piece.text) {
        Ok(metrics) => metrics.width,
        Err(error) => {
          tracing::debug!(%error, "a table column waits for the canvas text service");
          return None;
        }
      };
    }
    Some(width.ceil())
  }
}

/// The widths last measured, with the shapes they were measured for (texts, fonts and the room around them): a theme
/// with other fonts, text sizes or spacing measures again.
#[derive(Clone, Default, PartialEq)]
struct Measured {
  columns: Vec<Vec<Shape>>,
  widths: Vec<f32>,
}

impl lurq::app::component::DevtoolsInspectable for Measured {}

/// Measures the fixed columns of one table.
pub struct Measurer {
  reference: ElementRef,
  wanted: Arc<Mutex<Vec<Vec<Shape>>>>,
  measured: Signal<Measured>,
}

impl Measurer {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      wanted: Arc::new(Mutex::new(Vec::new())),
      measured: ctx.signal(Measured::default()),
    }
  }

  /// The width of each column of `columns`, each the widest of its shapes, once measured: the
  /// first layout of new texts has none yet, and the one after it does.
  pub fn widths(&self, columns: Vec<Vec<Shape>>) -> Option<Vec<f32>> {
    let measured = self.measured.get();
    let current = measured.columns == columns;
    *self.wanted.lock() = columns;
    current.then_some(measured.widths)
  }

  /// The canvas to measure with: empty and without size.
  pub fn canvas(&self) -> Element {
    Canvas::new()
      .ref_element(self.reference.clone())
      .width(Dimension::Px(0.0))
      .height(Dimension::Px(0.0))
      .absolute_position(0.0, 0.0)
      .into()
  }

  /// Measures what the last render asked for, if it asked for new texts, and renders again.
  pub fn after_layout(&self) {
    let Some(canvas) = self.reference.as_canvas() else {
      return;
    };
    let wanted = self.wanted.lock().clone();
    // Layout runs on every hover and resize; shapes already measured keep their widths.
    if self.measured.get_untracked().columns == wanted {
      return;
    }
    let widths: Option<Vec<f32>> = wanted
      .iter()
      .map(|shapes| {
        shapes
          .iter()
          .map(|shape| shape.width(&canvas))
          .try_fold(0.0_f32, |widest, width| Some(widest.max(width?)))
      })
      .collect();
    // Text the canvas cannot measure yet (its text service is not attached) is measured after
    // the next layout.
    let Some(widths) = widths else {
      return;
    };
    self.measured.set(Measured {
      columns: wanted,
      widths,
    });
  }
}
