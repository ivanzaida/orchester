//! How much of the pipeline canvas is shown (`iarCw`, the zoom controls of 03.01 `Q21HOA`): the
//! zoom level, stepped by the buttons and shortcuts and continuous with the wheel, fitting the
//! whole pipeline into view, and panning within the zoomed content. The canvas keeps each
//! pipeline's zoom for the rest of the session ([`SessionZooms`]).

use std::{collections::HashMap, sync::Arc};

use orchester_core::id::ResourceId;
use parking_lot::Mutex;

/// The zoom levels the buttons and shortcuts step through.
const STEPS: [f32; 11] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 2.0];
/// A wheel notch is 40 logical pixels of delta and zooms by 10 %.
const WHEEL_NOTCH: f32 = 40.0;
const WHEEL_FACTOR: f32 = 1.1;
/// Two zoom levels closer than this are the same step.
const SAME_STEP: f32 = 0.005;

/// A zoom level between 25 % and 200 %.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom(f32);

impl Zoom {
  pub const FULL: Self = Self(1.0);
  pub const MIN: Self = Self(STEPS[0]);
  pub const MAX: Self = Self(STEPS[STEPS.len() - 1]);

  /// `factor` held within [`Self::MIN`] and [`Self::MAX`]; a factor that is not a number is 100 %.
  pub fn new(factor: f32) -> Self {
    if factor.is_nan() {
      return Self::FULL;
    }
    Self(factor.clamp(Self::MIN.0, Self::MAX.0))
  }

  pub const fn factor(self) -> f32 {
    self.0
  }

  /// The level as the controls show it, e.g. `75` for 75 %.
  pub fn percent(self) -> u32 {
    // At most 200 and at least 25 after rounding, so the cast keeps the value.
    (self.0 * 100.0).round() as u32
  }

  /// The next step above this level, or the largest.
  pub fn zoomed_in(self) -> Self {
    STEPS
      .into_iter()
      .find(|step| *step > self.0 + SAME_STEP)
      .map_or(Self::MAX, Self)
  }

  /// The next step below this level, or the smallest.
  pub fn zoomed_out(self) -> Self {
    STEPS
      .into_iter()
      .rev()
      .find(|step| *step < self.0 - SAME_STEP)
      .map_or(Self::MIN, Self)
  }

  /// This level after a wheel turn of `delta_y` logical pixels; turning away from the user
  /// (positive) zooms in.
  pub fn wheeled(self, delta_y: f32) -> Self {
    Self::new(self.0 * WHEEL_FACTOR.powf(delta_y / WHEEL_NOTCH))
  }

  /// The largest level at which `content` fits into `viewport`, but not above 100 %: a small
  /// pipeline is not enlarged.
  pub fn fitting(content: Size, viewport: Size) -> Self {
    if content.width <= 0.0 || content.height <= 0.0 || viewport.width <= 0.0 || viewport.height <= 0.0 {
      return Self::FULL;
    }
    let factor = (viewport.width / content.width)
      .min(viewport.height / content.height)
      .min(Self::FULL.0);
    Self::new(factor)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
  pub width: f32,
  pub height: f32,
}

/// The zoom and the scroll offset of the zoomed content within the viewport. The offset never
/// goes past the content: at the top left it is zero, and content smaller than the viewport
/// stays at the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
  zoom: Zoom,
  x: f32,
  y: f32,
}

impl lurq::app::component::DevtoolsInspectable for View {}

impl View {
  /// `zoom`, scrolled to the top left.
  pub const fn new(zoom: Zoom) -> Self {
    Self { zoom, x: 0.0, y: 0.0 }
  }

  pub const fn zoom(&self) -> Zoom {
    self.zoom
  }

  /// How far the zoomed content is scrolled, in viewport pixels.
  pub const fn offset(&self) -> (f32, f32) {
    (self.x, self.y)
  }

  /// Zooms to `zoom`, keeping the content under `anchor` (a point of the viewport) where it is.
  pub fn zoomed(self, zoom: Zoom, anchor: (f32, f32), content: Size, viewport: Size) -> Self {
    let ratio = zoom.0 / self.zoom.0;
    Self {
      zoom,
      x: (self.x + anchor.0) * ratio - anchor.0,
      y: (self.y + anchor.1) * ratio - anchor.1,
    }
    .within(content, viewport)
  }

  /// Moves the content by `(dx, dy)` viewport pixels: dragging it right shows more of its left.
  pub fn panned(self, dx: f32, dy: f32, content: Size, viewport: Size) -> Self {
    Self {
      x: self.x - dx,
      y: self.y - dy,
      ..self
    }
    .within(content, viewport)
  }

  /// The whole of `content` at the level that fits it into `viewport`.
  pub fn fit(content: Size, viewport: Size) -> Self {
    Self::new(Zoom::fitting(content, viewport))
  }

  /// This view with its offset held within the zoomed `content`.
  pub fn within(self, content: Size, viewport: Size) -> Self {
    let limit = |content: f32, viewport: f32| (content * self.zoom.0 - viewport).max(0.0);
    Self {
      x: self.x.clamp(0.0, limit(content.width, viewport.width)),
      y: self.y.clamp(0.0, limit(content.height, viewport.height)),
      ..self
    }
  }
}

/// The zoom of each pipeline shown in this session, so reopening a pipeline shows it as it was
/// left. The root provides it; nothing is stored beyond the session.
#[derive(Clone, Default)]
pub struct SessionZooms(Arc<Mutex<HashMap<ResourceId, Zoom>>>);

impl SessionZooms {
  /// The zoom `pipeline` was left at, 100 % the first time.
  pub fn get(&self, pipeline: ResourceId) -> Zoom {
    self.0.lock().get(&pipeline).copied().unwrap_or(Zoom::FULL)
  }

  pub fn set(&self, pipeline: ResourceId, zoom: Zoom) {
    self.0.lock().insert(pipeline, zoom);
  }
}
