//! What the strip learns from its own layout: the active tab's width as lurq laid it out, so `tab_fit`
//! reserves its real room rather than its widest (lurq measures no text before layout), and whether the
//! tab list held the focus, so the focus has somewhere to go when the list goes away.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{ElementRef, Signal},
};

use crate::navigation::tabs::TabId;

/// A width this close to the last one is the same layout.
const SAME_WIDTH: f32 = 0.5;

/// What decides the active tab's width: which tab, its label and the room its marks take.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveKey {
  pub tab: TabId,
  pub label: String,
  pub marks: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Measured {
  key: ActiveKey,
  width: f32,
}

impl DevtoolsInspectable for Measured {}

/// The active tab's laid-out width, watched through its element.
pub struct ActiveWidth {
  element: ElementRef,
  measured: Signal<Option<Measured>>,
}

impl ActiveWidth {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      element: ctx.element_ref(),
      measured: ctx.signal(None),
    }
  }

  /// The element the active tab is laid out as.
  pub fn element(&self) -> ElementRef {
    self.element.clone()
  }

  /// The width the active tab `key` was laid out at, once it has been; watches it from now on, so a new
  /// width renders the strip again.
  pub fn width(&self, key: &ActiveKey) -> Option<f32> {
    let (measured, watched) = (self.measured.clone(), key.clone());
    self.element.observe_rect(move |rect| {
      let known = measured
        .get_untracked()
        .is_some_and(|last| last.key == watched && (last.width - rect.width).abs() < SAME_WIDTH);
      if rect.width > 0.0 && !known {
        measured.set(Some(Measured {
          key: watched.clone(),
          width: rect.width,
        }));
      }
    });
    self
      .measured
      .get()
      .filter(|measured| measured.key == *key)
      .map(|measured| measured.width)
  }
}

/// Whether the tab list holds the focus: open, or its button focused. The list writes `open`; the strip
/// keeps the button's element.
#[derive(Clone)]
pub struct ListHold {
  pub button: ElementRef,
  pub open: Arc<AtomicBool>,
}

impl Default for ListHold {
  fn default() -> Self {
    Self {
      button: ElementRef::new(),
      open: Arc::new(AtomicBool::new(false)),
    }
  }
}

/// The strip's for its life.
impl PartialEq for ListHold {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl std::fmt::Debug for ListHold {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("ListHold").finish_non_exhaustive()
  }
}

impl ListHold {
  /// Whether the list held the focus as it was laid out last.
  pub fn holds(&self) -> bool {
    self.open.load(Ordering::Relaxed) || (self.button.is_attached() && self.button.focus_signal().get_untracked())
  }

  pub fn set_open(&self, open: bool) {
    self.open.store(open, Ordering::Relaxed);
  }
}
