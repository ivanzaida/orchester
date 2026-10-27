//! Page tabs (`Page tab/active` `loUWZ`, `Page tab/default` `NspMt`): 34 high, 2 at their sides, 20
//! apart, on a 1 `border` line (`Workflow tabs` `bxhee`). The current tab's label is 13/500
//! `text_primary` over a 2 `accent` underline, the others' 13 `text_secondary`; a count follows the label
//! in its words (`All 24`), or as a 12 `text_muted` number after it (Settings' *Accounts 3*), or, for what waits for
//! the person, in a badge 6 after it: mono 11/500 `warning` on a `warning_muted` pill, 1 over and under and 6 at its
//! sides (`Count badge` `CiYKk`). A tab not built yet is its
//! label in 13 `text_muted` and an 11 `text_muted` lock, 5 apart, with why in its tooltip (`Page tab/locked`
//! `Smb4t`). Tabs are pointer-only.
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
  components::{Button, Row, Spacer, Stack, Text},
  core::ElementRef,
  layout::{Alignment, StackAlignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, Press, inert, pressable},
  tooltip::Tooltip,
};
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
};

/// `Page tab/*` height.
const TAB_HEIGHT: f32 = 34.0;
/// `Segmented` height, and a text segment's (`c4p0b`) and an icon segment's width (`wsGU4`).
const SEGMENTED_HEIGHT: f32 = 26.0;
const TEXT_SEGMENT_HEIGHT: f32 = 22.0;
const ICON_SEGMENT_WIDTH: f32 = 28.0;

#[derive(Clone)]
enum TabState {
  Open(Press),
  /// The page shown, which a press does not open again: it keeps the panel it shows.
  Here,
  /// Not built yet, for this reason.
  Unavailable(Arc<str>),
}

pub struct PageTab {
  id: Arc<str>,
  /// The label with its count, as the screen words it.
  label: Arc<str>,
  current: bool,
  state: TabState,
  explanation: Option<Arc<str>>,
  /// A count of what waits for the person, in a badge after the label.
  badge: Option<usize>,
  /// How many there are, a muted number after the label.
  count: Option<usize>,
}

impl PageTab {
  fn of(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, current: bool, state: TabState) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      current,
      state,
      explanation: None,
      badge: None,
      count: None,
    }
  }

  /// A tab that `on_open` opens; `label` already holds its count, if any ([`counted`]).
  pub fn new(
    id: impl Into<Arc<str>>,
    label: impl Into<Arc<str>>,
    current: bool,
    on_open: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self::of(id, label, current, TabState::Open(Press::run(on_open)))
  }

  /// A tab that is a place of its own: a press opens it with `open` by a browser's rules (`navigation::link`).
  pub fn to(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, current: bool, open: Open) -> Self {
    Self::of(id, label, current, TabState::Open(Press::Open(open)))
  }

  /// A tab whose page is not built yet: its label and a lock, and `reason` in its tooltip; it cannot be
  /// opened.
  pub fn unavailable(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, reason: impl Into<Arc<str>>) -> Self {
    Self::of(id, label, false, TabState::Unavailable(reason.into()))
  }

  /// The current tab, which a press leaves as it is: no action, and no place to open again, e.g. a page that keeps
  /// the panel it shows.
  pub fn current(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self::of(id, label, true, TabState::Here)
  }

  /// Opens an explanation panel for an unavailable feature, with the design's lock glyph.
  pub fn explanation(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.explanation = Some(reason.into());
    self
  }

  /// `count` things waiting for the person, in a badge after the label (`CiYKk`); its element id is `<id>-badge`.
  #[must_use]
  pub const fn badge(mut self, count: usize) -> Self {
    self.badge = Some(count);
    self
  }

  /// How many there are, in 12 `text_muted` after the label (`mIlfH` in Settings' `Tabs`); its element id is
  /// `<id>-count`.
  #[must_use]
  pub const fn count(mut self, count: usize) -> Self {
    self.count = Some(count);
    self
  }

  fn render(self, ctx: &mut Ctx) -> Element {
    let open = match self.state {
      TabState::Unavailable(reason) => {
        let tab = inert(Button::empty())
          .id(self.id.clone())
          .height(Dimension::Px(TAB_HEIGHT))
          .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .describe("a11y_name", self.label.clone())
          .describe("state", "unavailable")
          .describe("reason", reason.clone())
          .child(
            Text::new(&self.label)
              .id(format!("{}-label", self.id))
              .variant(TypographyStyle::Body)
              .color(PaletteColor::TextMuted)
              .nowrap(),
          )
          .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted).id(format!("{}-icon", self.id)));
        return Tooltip::new(self.id, reason).wrap(ctx, tab);
      }
      TabState::Open(open) => Some(open),
      TabState::Here => None,
    };
    let (typography, color) = if self.current {
      (ExtraTypography::Strong.role(), PaletteColor::TextPrimary)
    } else {
      (TypographyStyle::Body, PaletteColor::TextSecondary)
    };
    let label = Text::new(&self.label)
      .id(format!("{}-label", self.id))
      .variant(typography)
      .color(color)
      .nowrap();
    let button = open
      .map_or_else(|| Button::empty().focusable(false), Press::button)
      .id(self.id.clone())
      .height(Dimension::Px(TAB_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .describe("a11y_name", self.label.clone())
      .describe("state", if self.current { "current" } else { "enabled" });
    let button = match (self.badge, self.count) {
      (Some(count), _) => button.child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(label)
          .child(count_badge(&format!("{}-badge", self.id), count)),
      ),
      (None, Some(count)) => button.child(label).child(
        Text::new(&count.to_string())
          .id(format!("{}-count", self.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      ),
      (None, None) => button.child(label),
    };
    let button = if self.explanation.is_some() {
      button.child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
    } else {
      button
    };
    let button: Element = if self.current {
      button
        .border_bottom(Border::inside(BorderSize::Md, PaletteColor::Accent))
        .into()
    } else {
      button.into()
    };
    match self.explanation {
      Some(reason) => Tooltip::new(self.id, reason).wrap(ctx, button),
      None => button,
    }
  }
}

/// A tab's count of what waits for the person (`CiYKk`).
fn count_badge(id: &str, count: usize) -> Row {
  Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::WarningMuted)
    .rounded(ExtraRadius::Full.role())
    .child(
      Text::new(&count.to_string())
        .variant(ExtraTypography::MonoLabel.role())
        .color(PaletteColor::Warning)
        .nowrap(),
    )
}

/// A tab's label followed by its count, as the locale words it (`All 24`).
pub fn counted(ctx: &Ctx, label: &str, count: usize) -> Arc<str> {
  ctx.t_args(
    "ui.tabCount",
    [("label", label.to_owned()), ("count", count.to_string())],
  )
}

/// The row of page tabs on its line, as wide as its parent. The line is drawn under the tabs, so the
/// current tab's underline covers it (`BzK1B`).
pub fn page_tabs(ctx: &mut Ctx, id: &str, tabs: impl IntoIterator<Item = PageTab>) -> Stack {
  let tabs: Vec<Element> = tabs.into_iter().map(|tab| tab.render(ctx)).collect();
  tab_line(ctx, id, tabs)
}

/// [`page_tabs`] with `end` at the right end of the line, e.g. a page's Settings after its lists (`NestZ` in
/// `W721XP`).
pub fn page_tabs_with_end(
  ctx: &mut Ctx,
  id: &str,
  tabs: impl IntoIterator<Item = PageTab>,
  end: impl IntoIterator<Item = PageTab>,
) -> Stack {
  let mut row: Vec<Element> = tabs.into_iter().map(|tab| tab.render(ctx)).collect();
  row.push(Spacer::new().flex(1.0).into());
  row.extend(end.into_iter().map(|tab| tab.render(ctx)));
  tab_line(ctx, id, row)
}

/// [`page_tabs`] with `note` at the right end of the line, e.g. why a tab cannot be opened (`NWxZx` in `ptqq0`).
pub fn page_tabs_with_note(
  ctx: &mut Ctx,
  id: &str,
  tabs: impl IntoIterator<Item = PageTab>,
  note: impl Into<Element>,
) -> Stack {
  let mut row: Vec<Element> = tabs.into_iter().map(|tab| tab.render(ctx)).collect();
  row.push(Spacer::new().flex(1.0).into());
  row.push(note.into());
  tab_line(ctx, id, row)
}

fn tab_line(ctx: &Ctx, id: &str, tabs: Vec<Element>) -> Stack {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Stack::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .stack_align(StackAlignment::BottomStart)
    .child(
      Row::new()
        .id(format!("{id}-line"))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tabs.role())
        .align_items(Alignment::Center)
        .with_children(tabs),
    )
}

/// What a segment shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentFace {
  Text(Arc<str>),
  /// A glyph; the label names it for tools.
  Icon(Icon, Arc<str>),
  /// A 13 glyph before the label, 6 apart, both in the label's color (`System` `J9tyr` in Settings › General).
  Labelled(Icon, Arc<str>),
}

pub struct Segment {
  id: Arc<str>,
  face: SegmentFace,
  chosen: bool,
  choose: Arc<dyn Fn() + Send + Sync>,
  element: Option<ElementRef>,
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
      element: None,
    }
  }

  /// The segment's element, for a popup it opens to anchor to: lurq switches to that popup in one press, from
  /// another open dropdown, only when the pressed segment is the popup's anchor itself.
  pub fn element(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
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
    if let Some(element) = self.element {
      button = button.ref_element(element);
    }
    let (text, glyph) = if self.chosen {
      (PaletteColor::TextPrimary, PaletteColor::TextPrimary)
    } else {
      (PaletteColor::TextMuted, PaletteColor::TextMuted)
    };
    let typography = if self.chosen {
      TypographyStyle::Label
    } else {
      TypographyStyle::Description
    };
    button = match self.face {
      SegmentFace::Text(label) => {
        let button = button
          .height(Dimension::Px(TEXT_SEGMENT_HEIGHT))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .describe("a11y_name", label.clone())
          .child(Text::new(&label).id(label_id).variant(typography).color(text).nowrap());
        chosen_fill(button, self.chosen, ExtraColor::Pressed.role())
      }
      SegmentFace::Labelled(face, label) => {
        let button = button
          .height(Dimension::Px(TEXT_SEGMENT_HEIGHT))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .describe("a11y_name", label.clone())
          .child(icon(face, IconSize::Base, glyph))
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
