//! Hover hints of the status bar items: the full activity text when it is truncated, why a
//! count is unknown, the build of the version. The design names them ("tooltip") without
//! drawing one, so the hint uses the popover's raised panel style at label size.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Placement,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, HitTestBehavior, border::Border},
};

use super::POPOVER_GAP;

/// Items that have a hint, each with its own anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintAnchor {
  Runner,
  Activity,
  Agents,
  NeedsYou,
  Version,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
  anchor: HintAnchor,
  text: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for Hint {}

/// Element refs of the items with a hint; the activity item's also anchors the popover.
#[derive(Clone, Default)]
pub struct Anchors {
  runner: ElementRef,
  activity: ElementRef,
  agents: ElementRef,
  needs_you: ElementRef,
  version: ElementRef,
}

impl Anchors {
  pub fn get(&self, anchor: HintAnchor) -> &ElementRef {
    match anchor {
      HintAnchor::Runner => &self.runner,
      HintAnchor::Activity => &self.activity,
      HintAnchor::Agents => &self.agents,
      HintAnchor::NeedsYou => &self.needs_you,
      HintAnchor::Version => &self.version,
    }
  }
}

/// Attaches hints to items and shows the one under the pointer.
pub struct Hints {
  shown: Signal<Option<Hint>>,
  anchors: Anchors,
}

impl Hints {
  pub fn new(shown: &Signal<Option<Hint>>, anchors: &Anchors) -> Self {
    Self {
      shown: shown.clone(),
      anchors: anchors.clone(),
    }
  }

  /// `row` shows `text` while the pointer is over it.
  pub fn attach(&self, row: Row, anchor: HintAnchor, text: Arc<str>) -> Row {
    let (enter, leave) = (self.shown.clone(), self.shown.clone());
    let hint = Hint { anchor, text };
    row
      .ref_element(self.anchors.get(anchor).clone())
      .on_mouse_enter(move || enter.set(Some(hint.clone())))
      .on_mouse_leave(move || {
        if leave.get_untracked().is_some_and(|shown| shown.anchor == anchor) {
          leave.set(None);
        }
      })
  }

  pub fn popup(&self) -> Option<Element> {
    let hint = self.shown.get()?;
    let panel = Column::new()
      .id("status-bar-hint")
      .padding_horizontal(SpacingSize::Sm)
      .padding_vertical(SpacingSize::Xs)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&hint.text)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      );
    Some(
      Popup::new(self.anchors.get(hint.anchor).clone(), panel)
        .open_when(true)
        .placement(Placement::TopStart)
        .offset(0.0, POPOVER_GAP)
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false)
        .into(),
    )
  }
}
