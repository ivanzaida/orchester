//! Page tabs (`Page tab/active` `loUWZ`, `Page tab/default` `NspMt`): 34 high, 2 at their sides, 20
//! apart, on a 1 `border` line (`Workflow tabs` `bxhee`). The current tab's label is 13/500
//! `text_primary` over a 2 `accent` underline, the others' 13 `text_secondary`; a count follows the label
//! in its words (`All 24`). A tab not built yet is an 11 lock and its label in 13 `text_muted`, 5
//! apart, faded to 75 % (`Page tab · Artifacts · unavailable` `W4QYDM`). Tabs are pointer-only.
//!
//! Segmented control (`Segmented/text` `S5Fsve`, `Segmented` `C9VsTA`): 26 high, `surface_input` inside
//! a `border` stroke, radius 6, 2 inside and between segments. The chosen segment is filled with
//! `pressed` and its label 12/500 `text_primary`, the others 12 `text_muted`; a text segment is 22 high,
//! radius 4, 10 at its sides; an icon segment is 28 wide, radius 4, its 13 glyph `text_primary` when
//! chosen and `text_muted` otherwise, the chosen one filled with `hover`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, pressable},
};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, UNAVAILABLE_TAB_OPACITY};

/// `Page tab/*` height.
const TAB_HEIGHT: f32 = 34.0;
/// `Segmented` height, and a text segment's (`c4p0b`) and an icon segment's width (`wsGU4`).
const SEGMENTED_HEIGHT: f32 = 26.0;
const TEXT_SEGMENT_HEIGHT: f32 = 22.0;
const ICON_SEGMENT_WIDTH: f32 = 28.0;

#[derive(Clone)]
enum TabState {
  Open(Arc<dyn Fn() + Send + Sync>),
  Unavailable,
}

pub struct PageTab {
  id: Arc<str>,
  /// The label with its count, as the screen words it.
  label: Arc<str>,
  current: bool,
  state: TabState,
}

impl PageTab {
  /// A tab that `on_open` opens; `label` already holds its count, if any ([`counted`]).
  pub fn new(
    id: impl Into<Arc<str>>,
    label: impl Into<Arc<str>>,
    current: bool,
    on_open: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      current,
      state: TabState::Open(Arc::new(on_open)),
    }
  }

  /// A tab whose page is not built yet: a lock before its label, faded, and it cannot be opened.
  pub fn unavailable(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      current: false,
      state: TabState::Unavailable,
    }
  }
}

impl From<PageTab> for Element {
  fn from(tab: PageTab) -> Self {
    let open = match tab.state {
      TabState::Unavailable => {
        return Row::new()
          .id(tab.id.clone())
          .height(Dimension::Px(TAB_HEIGHT))
          .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .opacity(UNAVAILABLE_TAB_OPACITY)
          .describe("a11y_name", tab.label.clone())
          .describe("state", "unavailable")
          .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted).id(format!("{}-icon", tab.id)))
          .child(
            Text::new(&tab.label)
              .id(format!("{}-label", tab.id))
              .variant(TypographyStyle::Body)
              .color(PaletteColor::TextMuted)
              .nowrap(),
          )
          .into();
      }
      TabState::Open(open) => open,
    };
    let (typography, color) = if tab.current {
      (ExtraTypography::Strong.role(), PaletteColor::TextPrimary)
    } else {
      (TypographyStyle::Body, PaletteColor::TextSecondary)
    };
    let button = pressable(move || open())
      .id(tab.id.clone())
      .height(Dimension::Px(TAB_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
      .describe("a11y_name", tab.label.clone())
      .describe("state", if tab.current { "current" } else { "enabled" })
      .child(
        Text::new(&tab.label)
          .id(format!("{}-label", tab.id))
          .variant(typography)
          .color(color)
          .nowrap(),
      );
    if tab.current {
      button
        .border_bottom(Border::inside(BorderSize::Md, PaletteColor::Accent))
        .into()
    } else {
      button.into()
    }
  }
}

/// A tab's label followed by its count, as the locale words it (`All 24`).
pub fn counted(ctx: &Ctx, label: &str, count: usize) -> Arc<str> {
  ctx.t_args(
    "ui.tabCount",
    [("label", label.to_owned()), ("count", count.to_string())],
  )
}

/// The row of page tabs on its line, as wide as its parent.
pub fn page_tabs(id: &str, tabs: impl IntoIterator<Item = PageTab>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tabs.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(tabs.into_iter().map(Element::from))
}

/// What a segment shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentFace {
  Text(Arc<str>),
  /// A glyph; the label names it for tools.
  Icon(Icon, Arc<str>),
}

pub struct Segment {
  id: Arc<str>,
  face: SegmentFace,
  chosen: bool,
  choose: Arc<dyn Fn() + Send + Sync>,
}

impl Segment {
  pub fn new(
    id: impl Into<Arc<str>>,
    face: SegmentFace,
    chosen: bool,
    choose: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      face,
      chosen,
      choose: Arc::new(choose),
    }
  }

  fn render(self, focus: Focus) -> Element {
    let choose = self.choose;
    let label_id = format!("{}-label", self.id);
    let mut button = focus
      .apply(pressable(move || choose()))
      .id(self.id)
      .justify(Justify::Center)
      .rounded(RadiusSize::Sm)
      .describe("state", if self.chosen { "chosen" } else { "enabled" });
    let (text, glyph) = if self.chosen {
      (PaletteColor::TextPrimary, PaletteColor::TextPrimary)
    } else {
      (PaletteColor::TextMuted, PaletteColor::TextMuted)
    };
    button = match self.face {
      SegmentFace::Text(label) => {
        let typography = if self.chosen {
          TypographyStyle::Label
        } else {
          TypographyStyle::Description
        };
        let button = button
          .height(Dimension::Px(TEXT_SEGMENT_HEIGHT))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .describe("a11y_name", label.clone())
          .child(Text::new(&label).id(label_id).variant(typography).color(text).nowrap());
        chosen_fill(button, self.chosen, ExtraColor::Pressed.role())
      }
      SegmentFace::Icon(face, label) => {
        let button = button
          .width(Dimension::Px(ICON_SEGMENT_WIDTH))
          .height(Dimension::Pct(100.0))
          .describe("a11y_name", label)
          .child(icon(face, IconSize::Base, glyph).id(label_id));
        chosen_fill(button, self.chosen, ExtraColor::Hover.role())
      }
    };
    button.into()
  }
}

fn chosen_fill(button: Button, chosen: bool, fill: PaletteColor) -> Button {
  if chosen {
    button.background(fill)
  } else {
    button.hovered_style(Style::new().background(ExtraColor::Hover.role()))
  }
}

/// A segmented control of `segments`; in a form its segments are Tab stops.
pub fn segmented(id: &str, segments: impl IntoIterator<Item = Segment>, focus: Focus) -> Row {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(SEGMENTED_HEIGHT))
    .padding(ExtraSpacing::Tight.role())
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(segments.into_iter().map(|segment| segment.render(focus)))
}
