//! Tooltip (`Tooltip` `b6q3OZ`): a short label in 11/500 `text_inverse` on `text_primary`, radius 4, 4
//! over and under it and 8 at its sides, an optional shortcut 8 after it in Geist Mono 10 at 60 %. It
//! shows while the pointer rests on its anchor and lets the pointer through; a long one wraps. [`Truncated`]
//! is text cut to one line with an ellipsis that keeps its full value in a tooltip, shown only while the
//! text is cut (handover §5.3), or that has a hint of its own, worded for whether it is cut. A path or a
//! command in a tooltip is in Geist Mono 11/500 (`C9Phsz`).

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
  /// The words are a path or a command.
  mono: bool,
  /// The anchor is this text, cut to its room: the tooltip shows only while it is cut.
  cut: Option<CutText>,
}

/// How a cut text is set, and what its tooltip says.
#[derive(Clone, Debug, PartialEq)]
struct CutText {
  typography: TypographyStyle,
  color: Option<PaletteColor>,
  hint: Option<Hint>,
  frame: Option<Frame>,
}

/// Lays out the control a cut text names from the anchor's row, given whether the pointer rests on it.
type LayOut = dyn Fn(Row, bool) -> Row + Send + Sync;

/// The control a cut text names, laid out around it: the anchor is the whole control.
#[derive(Clone)]
struct Frame {
  lay_out: Arc<LayOut>,
  /// The text's color while the pointer rests on the control.
  hovered: PaletteColor,
}

/// A frame compares by its colors: its layout depends only on whether the pointer rests, and closures do
/// not compare.
impl PartialEq for Frame {
  fn eq(&self, other: &Self) -> bool {
    self.hovered == other.hovered
  }
}

impl std::fmt::Debug for Frame {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Frame")
      .field("hovered", &self.hovered)
      .finish_non_exhaustive()
  }
}

/// A cut text's tooltip that shows whether or not the text is cut.
#[derive(Clone, Debug, PartialEq)]
struct Hint {
  fits: Arc<str>,
  cut: Arc<str>,
}

impl DevtoolsInspectable for Tooltip {}

impl Tooltip {
  /// A tooltip saying `text`; its panel's element id is `<id>-tooltip`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      shortcut: None,
      mono: false,
      cut: None,
    }
  }

  /// The words are a path or a command: in Geist Mono.
  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
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

  /// What the tooltip says once the pointer rested on its anchor, if it shows at all: a cut text's own
  /// words only while it is cut; its hint's words either way.
  fn words(&self, fits: bool) -> Option<&Arc<str>> {
    match &self.cut {
      None => Some(&self.text),
      Some(CutText { hint: Some(hint), .. }) => Some(if fits { &hint.fits } else { &hint.cut }),
      Some(_) if fits => None,
      Some(_) => Some(&self.text),
    }
  }

  fn panel(&self, words: &str) -> Row {
    let typography = if self.mono {
      ExtraTypography::MonoLabel.role()
    } else {
      TypographyStyle::FieldLabel
    };
    let mut panel = Row::new()
      .id(format!("{}-tooltip", self.id))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::TextPrimary)
      .rounded(RadiusSize::Sm)
      .child(
        Text::new(words)
          .variant(typography)
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
  /// When it showed, the cut text was whole: it fits its room.
  fits: Signal<bool>,
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
    let (shown, fits) = (ctx.signal(false), ctx.signal(false));
    let (cut, whole) = (ctx.element_ref(), ctx.element_ref());
    let (show, fit, measured, unmeasured) = (shown.clone(), fits.clone(), cut.clone(), whole.clone());
    let timer = ctx.create_timeout(TOOLTIP_DELAY, move || {
      // Without a cut text the copy that measures it is not laid out.
      fit.set(unmeasured.is_attached() && unmeasured.width() <= measured.width() + CUT_TOLERANCE);
      show.set(true);
    });
    Self {
      shown,
      fits,
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
    if let Some(words) = tooltip.words(self.fits.get()).filter(|_| self.shown.get()) {
      anchor = anchor.child(
        Popup::new(self.anchor.clone(), tooltip.panel(words))
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
  /// rests on it, an unseen whole copy in an overlay that measures it without moving the row. A framed
  /// text's anchor is the control it names.
  fn cut_text(&self, anchor: Row, tooltip: &Tooltip, cut: &CutText) -> Row {
    let resting = self.resting.get();
    let color = match &cut.frame {
      Some(frame) if resting => Some(frame.hovered.clone()),
      _ => cut.color.clone(),
    };
    let text = |text: Text| match color.clone() {
      Some(color) => text.variant(cut.typography).color(color).nowrap(),
      None => text.variant(cut.typography).nowrap(),
    };
    let anchor = match &cut.frame {
      Some(frame) => (frame.lay_out)(anchor, resting),
      None => anchor,
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
      .with_children(resting.then(|| {
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
  hint: Option<Hint>,
  mono: bool,
  frame: Option<Frame>,
}

impl Truncated {
  /// The text's element id is `id`, its tooltip's `<id>-tooltip`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>, typography: TypographyStyle) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography,
      color: None,
      hint: None,
      mono: false,
      frame: None,
    }
  }

  /// The text's color, where it is not its style's.
  pub fn color(mut self, color: impl Into<PaletteColor>) -> Self {
    self.color = Some(color.into());
    self
  }

  /// A tooltip whenever the pointer rests on the text, not only while it is cut: `fits` while the text
  /// fits its room, `cut` while it is cut (which says the text's own words too).
  pub fn hint(mut self, fits: impl Into<Arc<str>>, cut: impl Into<Arc<str>>) -> Self {
    self.hint = Some(Hint {
      fits: fits.into(),
      cut: cut.into(),
    });
    self
  }

  /// The tooltip's words are a path or a command: in Geist Mono.
  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// The text names a control that `lay_out` builds around it from the anchor's row, given whether the
  /// pointer rests on it: the parts before the text, the control's room and its fill. The text is in
  /// `hovered` while the pointer rests. The whole control is the anchor: the tooltip hangs from its start.
  pub fn framed(
    mut self,
    hovered: impl Into<PaletteColor>,
    lay_out: impl Fn(Row, bool) -> Row + Send + Sync + 'static,
  ) -> Self {
    self.frame = Some(Frame {
      lay_out: Arc::new(lay_out),
      hovered: hovered.into(),
    });
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("tooltip-{}", self.id);
    let tooltip = Tooltip {
      mono: self.mono,
      cut: Some(CutText {
        typography: self.typography,
        color: self.color,
        hint: self.hint,
        frame: self.frame,
      }),
      ..Tooltip::new(self.id, self.text)
    };
    ctx.mount_keyed::<TooltipHost>(&key, tooltip)
  }
}
