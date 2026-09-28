//! Dialog backdrop (`a8U756`): the scrim over the screen with the dialog centred in it, as the
//! design draws every backdrop over its 1440 x 960 screen frame and nothing else. The window
//! chrome around the screen stays undimmed (design doc "Window chrome"). While a dialog is open
//! only the dialog and the title bar work: the title bar above keeps its drag and window controls,
//! so the pointer passes through that band, while the status bar below is covered by a clear band
//! that takes the pointer, so it shows no hover and takes no clicks. It never takes the focus, so
//! Tab stays in the dialog. A dialog never touches the left and right window edges: a dialog that
//! can shrink keeps `ExtraSpacing::DialogMargin` from them in a narrow window. The title bar's
//! tabs, + and tab list do not react while a dialog is open ([`DialogLayer`]): the dialog belongs
//! to its tab's page, and switching away would hide it.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, dimension::Dimension},
};

use super::pressable::surface;
use crate::theme::{ExtraColor, ExtraSpacing, STATUS_BAR_HEIGHT, TITLE_BAR_HEIGHT};

/// How many dialogs the window shows, provided by the root: every backdrop counts while it is
/// mounted.
#[derive(Clone)]
pub struct DialogLayer {
  open: Signal<usize>,
  /// Identity, for comparing props that hold the layer.
  id: Arc<()>,
}

impl PartialEq for DialogLayer {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.id, &other.id)
  }
}

impl lurq::app::component::DevtoolsInspectable for DialogLayer {}

impl Default for DialogLayer {
  fn default() -> Self {
    Self {
      open: Signal::new(0),
      id: Arc::default(),
    }
  }
}

impl DialogLayer {
  /// Whether a dialog is open, read reactively.
  pub fn is_open(&self) -> bool {
    self.open.get() > 0
  }

  /// Whether a dialog is open, for an event handler.
  pub fn is_open_now(&self) -> bool {
    self.open.get_untracked() > 0
  }
}

/// Counts its backdrop's dialog as open for as long as it is mounted.
struct DialogPresence {
  layer: Option<DialogLayer>,
}

impl Component for DialogPresence {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let layer = ctx.use_context::<DialogLayer>();
    if let Some(layer) = &layer {
      layer.open.update(|open| *open += 1);
    }
    Self { layer }
  }

  fn render(&self, _ctx: &mut Ctx) -> impl Into<Element> {
    Element::new()
  }
}

impl Drop for DialogPresence {
  fn drop(&mut self) {
    if let Some(layer) = &self.layer {
      layer.open.update(|open| *open = open.saturating_sub(1));
    }
  }
}

/// `dialog` over the scrim; mounted in a `Modal` that targets the whole window.
pub fn backdrop(ctx: &mut Ctx, dialog: impl Into<Element>) -> Element {
  let scrim = Column::new()
    .id("dialog-backdrop")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_horizontal(ExtraSpacing::DialogMargin.role())
    .background(ExtraColor::Scrim.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(dialog);
  // Clear, so the status bar shows through undimmed, but it takes the pointer before the bar. A
  // pointer-only surface, so a press on it also leaves the focus in the dialog.
  let status_bar_cover = surface()
    .id("dialog-status-bar-cover")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STATUS_BAR_HEIGHT));
  // The band above the scrim belongs to the title bar: the pointer passes through it.
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(Dimension::Px(TITLE_BAR_HEIGHT))
    .hit_test(HitTestBehavior::ContentOnly)
    .child(scrim)
    .child(status_bar_cover)
    .child(ctx.mount::<DialogPresence>(()))
    .into()
}
