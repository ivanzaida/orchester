//! Tooltip (`Tooltip` `b6q3OZ`): a short label in 11/500 `text_inverse` on `text_primary`, radius 4, 4
//! over and under it and 8 at its sides, an optional shortcut 8 after it in Geist Mono 10 at 60 %. It
//! shows while the pointer rests on its anchor and lets the pointer through. [`Truncated`] is text cut
//! to one line with an ellipsis that keeps its full value in a tooltip (handover §5.3).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraSpacing, ExtraTypography, SHORTCUT_OPACITY};

/// How long the pointer rests on the anchor before the tooltip shows.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
/// The widest a tooltip gets before its text wraps.
const MAX_WIDTH: f32 = 420.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
  id: Arc<str>,
  text: Arc<str>,
  shortcut: Option<Arc<str>>,
  placement: Placement,
  shrink: bool,
}

impl DevtoolsInspectable for Tooltip {}

impl Tooltip {
  /// A tooltip saying `text`; its panel's element id is `<id>-tooltip`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      shortcut: None,
      placement: Placement::BottomStart,
      shrink: false,
    }
  }

  /// The keys that do the same, after the label.
  pub fn shortcut(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.shortcut = Some(keys.into());
    self
  }

  /// Shown above the anchor instead of under it, e.g. for an anchor at the window's bottom.
  pub fn above(mut self) -> Self {
    self.placement = Placement::TopStart;
    self
  }

  /// The anchor gives up room when its row runs out of it, as text cut to its room does.
  pub fn shrink(mut self) -> Self {
    self.shrink = true;
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
      .max_width(Dimension::Px(MAX_WIDTH))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::TextPrimary)
      .rounded(RadiusSize::Sm)
      .child(
        Text::new(&self.text)
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextInverse),
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
  timer: Timeout,
  anchor: ElementRef,
}

impl Component for TooltipHost {
  type Props = Tooltip;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    let timer = ctx.create_timeout(TOOLTIP_DELAY, move || show.set(true));
    Self {
      shown,
      timer,
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tooltip = ctx.props::<Tooltip>().clone();
    let (rest, leave, hide) = (self.timer.clone(), self.timer.clone(), self.shown.clone());
    let mut anchor = Row::new()
      .ref_element(self.anchor.clone())
      .align_items(Alignment::Center)
      .on_mouse_enter(move || rest.restart())
      .on_mouse_leave(move || {
        leave.cancel();
        hide.set(false);
      })
      .with_children(ctx.children().iter().cloned());
    if tooltip.shrink {
      anchor = anchor.flex_shrink(1.0).min_width(Dimension::Px(0.0));
    }
    if !self.shown.get() {
      return anchor;
    }
    let popup = Popup::new(self.anchor.clone(), tooltip.panel())
      .open_when(true)
      .placement(tooltip.placement)
      .offset(0.0, ExtraSpacing::Snug.value())
      .hit_test(HitTestBehavior::None)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false);
    anchor.child(popup)
  }
}

/// Text on one line, cut with an ellipsis where its room ends, its full value in a tooltip.
pub struct Truncated {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  color: Option<PaletteColor>,
  width: Option<Dimension>,
}

impl Truncated {
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>, typography: TypographyStyle) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography,
      color: None,
      width: None,
    }
  }

  pub fn color(mut self, color: impl Into<PaletteColor>) -> Self {
    self.color = Some(color.into());
    self
  }

  /// A fixed room, e.g. a table column's width; without one the text is as wide as it is, and cut where
  /// its row runs out of room.
  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = Some(width.into());
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let mut text = Text::new(&self.text)
      .id(self.id.to_string())
      .variant(self.typography)
      .nowrap()
      .max_width(Dimension::Pct(100.0))
      .text_overflow(TextOverflow::Elipsis);
    if let Some(color) = self.color {
      text = text.color(color);
    }
    let tooltip = Tooltip::new(self.id.clone(), self.text.clone());
    match self.width {
      Some(width) => Row::new().width(width).child(tooltip.shrink().wrap(ctx, text)).into(),
      None => tooltip.shrink().wrap(ctx, text),
    }
  }
}
