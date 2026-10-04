//! Hover hints of the status bar items: the full activity text when it is truncated, why a
//! count is unknown, where the MCP server listens, the build of the
//! version. The design names them ("tooltip") without
//! drawing one, so the hint uses the popover's raised panel style at label size.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Placement,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension},
};

use super::POPOVER_GAP;
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// Items that have a hint, each with its own anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintAnchor {
  Runner,
  Activity,
  Agents,
  Tokens,
  Mcp,
  Version,
  Server,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
  anchor: HintAnchor,
  /// One line per server over the text, for the server item (10.43 A, F).
  lines: Arc<[HintLine]>,
  text: Arc<str>,
}

/// A server in the server item's hint (`DfYNE`): its connection's icon, name and state, and a mono
/// detail under them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HintLine {
  pub icon: Icon,
  pub name: Arc<str>,
  pub state: Arc<str>,
  pub detail: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for Hint {}

/// Element refs of the items with a hint; the activity item's also anchors the popover.
#[derive(Clone, Default)]
pub struct Anchors {
  runner: ElementRef,
  activity: ElementRef,
  agents: ElementRef,
  tokens: ElementRef,
  mcp: ElementRef,
  version: ElementRef,
  server: ElementRef,
}

impl Anchors {
  pub fn get(&self, anchor: HintAnchor) -> &ElementRef {
    match anchor {
      HintAnchor::Runner => &self.runner,
      HintAnchor::Activity => &self.activity,
      HintAnchor::Agents => &self.agents,
      HintAnchor::Tokens => &self.tokens,
      HintAnchor::Mcp => &self.mcp,
      HintAnchor::Version => &self.version,
      HintAnchor::Server => &self.server,
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

  /// `item` shows `text` while the pointer is over it.
  pub fn attach(&self, item: Button, anchor: HintAnchor, text: Arc<str>) -> Button {
    self.attach_lines(item, anchor, Arc::new([]), text)
  }

  /// `item` shows `lines`, then `text`, while the pointer is over it.
  pub fn attach_lines(&self, item: Button, anchor: HintAnchor, lines: Arc<[HintLine]>, text: Arc<str>) -> Button {
    let (enter, leave) = (self.shown.clone(), self.shown.clone());
    let hint = Hint { anchor, lines, text };
    item
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
    // Under server lines the text is a muted note (`L7VPr`).
    let color = if hint.lines.is_empty() {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    let panel = if hint.lines.is_empty() {
      Column::new()
        .padding_horizontal(SpacingSize::Sm)
        .padding_vertical(SpacingSize::Xs)
    } else {
      // The server item's hint (`FoEjc`): 8 all round, 8 between its lines.
      Column::new()
        .padding(SpacingSize::Sm)
        .spacing(SpacingSize::Sm)
        .with_children(hint.lines.iter().map(line))
    };
    let panel = panel
      .id("status-bar-hint")
      .max_width(Dimension::Px(super::popover::WIDTH))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&hint.text)
          .id("status-bar-hint-text")
          .variant(TypographyStyle::Caption)
          .color(color),
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

/// A server's line in the hint: icon, name and state, and the detail under them (`DfYNE`).
fn line(line: &HintLine) -> Element {
  Column::new()
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(icon(line.icon, IconSize::Small, PaletteColor::TextPrimary))
        .child(
          Text::new(&line.name)
            .variant(ExtraTypography::HintHeading.role())
            .nowrap(),
        )
        .child(Text::new(&line.state).variant(TypographyStyle::Caption).nowrap()),
    )
    .child(
      Text::new(&line.detail)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}
