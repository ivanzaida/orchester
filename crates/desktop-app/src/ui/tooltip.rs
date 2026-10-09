//! Tooltip (`Tooltip` `b6q3OZ`): a short label in 11/500 `text_inverse` on `text_primary`, radius 4, 4
//! over and under it and 8 at its sides, an optional shortcut 8 after it in Geist Mono 10 at 60 %. It
//! shows while the pointer rests on its anchor and lets the pointer through; a long one wraps. [`Truncated`]
//! is text cut to one line with an ellipsis that keeps its full value in a tooltip, shown only while the
//! text is cut (handover §5.3).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{CollisionStrategy, Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraSpacing, ExtraTypography, SHORTCUT_OPACITY};

/// How long the pointer rests on the anchor before the tooltip shows.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
/// The widest a tooltip's words run before they wrap.
const TEXT_MAX_WIDTH: f32 = 400.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
  id: Arc<str>,
  text: Arc<str>,
  shortcut: Option<Arc<str>>,
  /// The anchor is this text, cut to its room: the tooltip shows only while it is cut.
  cut: Option<CutText>,
}

/// How a cut text is set.
#[derive(Clone, Debug, PartialEq)]
struct CutText {
  typography: TypographyStyle,
  color: Option<PaletteColor>,
}

impl DevtoolsInspectable for Tooltip {}

impl Tooltip {
  /// A tooltip saying `text`; its panel's element id is `<id>-tooltip`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      shortcut: None,
      cut: None,
    }
  }

  /// The keys that do the same, after the label.
  pub fn shortcut(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.shortcut = Some(keys.into());
    self
  }

  /// `anchor` with this tooltip.
  pub fn wrap(self, ctx: &mut Ctx, anchor: impl Into<Element>) -> Element {
    let key = format!("tooltip-{}", self.id);
    ctx.mount_keyed_with::<TooltipHost>(&key, self, vec![anchor.into()])
  }

  fn panel(&self) -> Row {
    let mut panel = Row::new()
      .id(format!("{}-tooltip", self.id))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::TextPrimary)
      .rounded(RadiusSize::Sm)
      .child(
        Text::new(&self.text)
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextInverse)
          .max_width(Dimension::Px(TEXT_MAX_WIDTH)),
      );
    if let Some(keys) = &self.shortcut {
      panel = panel.child(
        Text::new(keys)
          .variant(ExtraTypography::MonoSmall.role())
          .color(PaletteColor::TextInverse)
          .opacity(SHORTCUT_OPACITY)
          .nowrap(),
      );
    }
    panel
  }
}

struct TooltipHost {
  shown: Signal<bool>,
  /// The pointer rests on the anchor: a cut text's whole copy is laid out to measure it.
  resting: Signal<bool>,
  timer: Timeout,
  anchor: ElementRef,
  /// A cut text as it is laid out, and as wide as it would be whole.
  cut: ElementRef,
  whole: ElementRef,
}

impl Component for TooltipHost {
  type Props = Tooltip;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let (cut, whole) = (ctx.element_ref(), ctx.element_ref());
    let (show, measured, unmeasured) = (shown.clone(), cut.clone(), whole.clone());
    let timer = ctx.create_timeout(TOOLTIP_DELAY, move || {
      // Text that fits says all it has: only a cut one gets the tooltip. Without a cut text the copy
      // that measures it is not laid out.
      if !unmeasured.is_attached() || unmeasured.width() > measured.width() + CUT_TOLERANCE {
        show.set(true);
      }
    });
    Self {
      shown,
      resting: ctx.signal(false),
      timer,
      anchor: ctx.element_ref(),
      cut,
      whole,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tooltip = ctx.props::<Tooltip>().clone();
    let (rest, leave, hide) = (self.timer.clone(), self.timer.clone(), self.shown.clone());
    let (resting, left) = (self.resting.clone(), self.resting.clone());
    let anchor = Row::new()
      .ref_element(self.anchor.clone())
      .align_items(Alignment::Center)
      .on_mouse_enter(move || {
        resting.set(true);
        rest.restart();
      })
      .on_mouse_leave(move || {
        leave.cancel();
        left.set(false);
        hide.set(false);
      });
    let mut anchor = match &tooltip.cut {
      Some(cut) => self.cut_text(anchor, &tooltip, cut),
      None => anchor.with_children(ctx.children().iter().cloned()),
    };
    if self.shown.get() {
      anchor = anchor.child(
        Popup::new(self.anchor.clone(), tooltip.panel())
          .open_when(true)
          .placement(Placement::BottomStart)
          .offset(0.0, ExtraSpacing::Snug.value())
          .hit_test(HitTestBehavior::None)
          .dismiss_on_outside_click(false)
          .dismiss_on_escape(false),
      );
    }
    anchor
  }
}

/// How much wider than its room a text must be to count as cut, for rounding.
const CUT_TOLERANCE: f32 = 0.5;

impl TooltipHost {
  /// The cut text in `anchor`, which gives up room when its row runs out of it, and, while the pointer
  /// rests on it, an unseen whole copy in an overlay that measures it without moving the row.
  fn cut_text(&self, anchor: Row, tooltip: &Tooltip, cut: &CutText) -> Row {
    let text = |text: Text| match cut.color.clone() {
      Some(color) => text.variant(cut.typography).color(color).nowrap(),
      None => text.variant(cut.typography).nowrap(),
    };
    anchor
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .child(
        text(Text::new(&tooltip.text))
          .id(tooltip.id.to_string())
          .describe("a11y_name", tooltip.text.clone())
          .ref_element(self.cut.clone())
          .max_width(Dimension::Pct(100.0))
          .text_overflow(TextOverflow::Elipsis),
      )
      // The copy is an overlay, out of the row's flow and out of its name: laid out whole, unseen.
      .with_children(self.resting.get().then(|| {
        let whole = text(Text::new(&tooltip.text))
          .ref_element(self.whole.clone())
          .opacity(0.0);
        Popup::new(self.anchor.clone(), whole)
          .open_when(true)
          .placement(Placement::BottomStart)
          .collision(CollisionStrategy::None)
          .hit_test(HitTestBehavior::None)
          .dismiss_on_outside_click(false)
          .dismiss_on_escape(false)
      }))
  }
}

/// Text on one line, cut with an ellipsis where its row runs out of room; while it is cut, its full
/// value shows in a tooltip.
pub struct Truncated {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  color: Option<PaletteColor>,
}

impl Truncated {
  /// The text's element id is `id`, its tooltip's `<id>-tooltip`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>, typography: TypographyStyle) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography,
      color: None,
    }
  }

  /// The text's color, where it is not its style's.
  pub fn color(mut self, color: impl Into<PaletteColor>) -> Self {
    self.color = Some(color.into());
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("tooltip-{}", self.id);
    let tooltip = Tooltip {
      cut: Some(CutText {
        typography: self.typography,
        color: self.color,
      }),
      ..Tooltip::new(self.id, self.text)
    };
    ctx.mount_keyed::<TooltipHost>(&key, tooltip)
  }
}
