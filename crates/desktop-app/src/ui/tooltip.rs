//! Tooltip (`Tooltip` `b6q3OZ`): lurq's `ui::Tooltip` in the theme's recipe (`theme::kit::tooltip`), a short label
//! in 11/500 `text_inverse` on `text_primary` with an optional shortcut; a long one wraps. It shows while the pointer
//! rests on its anchor and lets the pointer through, so the kit hangs it (`Tooltip::hang`) beside its anchor while the
//! pointer rests there: a mounted lurq tooltip takes the pointer that moves onto it, and covers what is under it.
//! [`Truncated`] is text cut to one line with an ellipsis that keeps its full value in a tooltip, shown only while the
//! text is cut (lurq's `ElementRef::text_cut`, handover §5.3), or that has a hint of its own, worded for whether it is
//! cut. A path or a command in a tooltip is in Geist Mono 11/500 (`C9Phsz`).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Placement, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
  ui::{self, TooltipPart},
};

use crate::theme::ExtraSpacing;

/// How long the pointer rests on the anchor before the tooltip shows.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
/// The widest a tooltip's words run before they wrap.
const TEXT_MAX_WIDTH: f32 = 400.0;
/// The widest the words of a tooltip with a note run before they wrap (`Tooltip · account switch` `oL5mO`: 236 less 8
/// at each side).
const NOTE_TEXT_WIDTH: f32 = 220.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
  id: Arc<str>,
  text: Arc<str>,
  shortcut: Option<Arc<str>>,
  /// The words are a path or a command.
  mono: bool,
  /// The anchor is this text, cut to its room: the tooltip shows only while it is cut.
  cut: Option<CutText>,
  /// It shows nothing now.
  silent: bool,
  /// Its right edge lines up with its anchor's, for an anchor at the end of a row (`Tooltip · Resets at …` in
  /// `uuaob`); else its left edge does.
  end: bool,
  /// It opens over its anchor, clear of the top of the box the anchor sits in; else under the anchor.
  over: Option<Over>,
  /// The widest its words run before they wrap.
  wrap_at: f32,
  /// What it says under its first line, in regular weight (`Tooltip · account switch` `oL5mO`).
  note: Option<Arc<str>>,
}

/// The box a tooltip that opens over its anchor keeps clear of: its bottom 6 over the box's top edge.
#[derive(Clone)]
struct Over(ElementRef);

/// The box is its anchor's, laid out by the same component for as long as the tooltip lives: two compare equal.
impl PartialEq for Over {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl std::fmt::Debug for Over {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Over").finish_non_exhaustive()
  }
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
      silent: false,
      end: false,
      over: None,
      wrap_at: TEXT_MAX_WIDTH,
      note: None,
    }
  }

  /// A second line under its words: they become its title, 2 over the note, and the panel is 6 over and under them,
  /// both wrapping at 220.
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self.wrap_at = NOTE_TEXT_WIDTH;
    self
  }

  /// It lines up with its anchor's right edge.
  pub fn end(mut self) -> Self {
    self.end = true;
    self
  }

  /// It opens over its anchor, its bottom 6 clear of the top edge of `frame`, the box the anchor sits in (`Tooltip ·
  /// skill chip on Codex` `mhanX` in `07` `QnPRC`).
  pub fn over(mut self, frame: &ElementRef) -> Self {
    self.over = Some(Over(frame.clone()));
    self
  }

  /// Its words wrap at `width` rather than at the widest a tooltip's run.
  pub fn wrap_at(mut self, width: f32) -> Self {
    self.wrap_at = width;
    self
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

  /// While `silent` it shows nothing, e.g. while what its anchor opened is open under it.
  pub fn silent(mut self, silent: bool) -> Self {
    self.silent = silent;
    self
  }

  /// `anchor` with this tooltip.
  pub fn wrap(self, ctx: &mut Ctx, anchor: impl Into<Element>) -> Element {
    let key = format!("tooltip-{}", self.id);
    ctx.mount_keyed_with::<TooltipHost>(&key, self, vec![anchor.into()])
  }

  /// Its panel hung `placement` of `anchor` while `shown`, for an anchor that shows its tooltip by itself rather than
  /// once the pointer rested on it (a message's actions, `Tooltip` in `06 · Message actions` `XPDAX`). It takes no
  /// pointer, so a press that arrives with the pointer reaches the anchor.
  pub fn hung(&self, anchor: &ElementRef, placement: Placement, shown: bool) -> Element {
    self.lurq(&self.text).placement(placement).hang(anchor, shown)
  }

  /// lurq's tooltip saying this one's words, for a lurq component that shows it itself (a page tab's).
  pub fn to_ui(&self) -> ui::Tooltip {
    self.lurq(&self.text)
  }

  /// What the tooltip says once the pointer rested on its anchor, if it shows at all: a cut text's own
  /// words only while it is cut; its hint's words either way.
  fn words(&self, fits: bool) -> Option<&Arc<str>> {
    match &self.cut {
      _ if self.silent => None,
      None => Some(&self.text),
      Some(CutText { hint: Some(hint), .. }) => Some(if fits { &hint.fits } else { &hint.cut }),
      Some(_) if fits => None,
      Some(_) => Some(&self.text),
    }
  }

  /// lurq's tooltip saying `words`, 6 from its anchor, its words wrapping at [`Self::wrap_at`]'s width; one with a note
  /// is that wide whatever its words (`oL5mO`).
  fn lurq(&self, words: &str) -> ui::Tooltip {
    let side = ui::current_theme().spacing_value(SpacingSize::Sm).to_px();
    let widest = self.wrap_at + 2.0 * side;
    let mut tooltip = ui::Tooltip::new(words)
      .id(format!("{}-tooltip", self.id))
      .offset(ExtraSpacing::Snug.value())
      .max_width(widest)
      .mono(self.mono);
    if let Some(note) = &self.note {
      tooltip = tooltip
        .note(note)
        .style(TooltipPart::Panel, |style| style.width(widest));
    }
    if let Some(keys) = &self.shortcut {
      tooltip = tooltip.shortcut(keys);
    }
    tooltip
  }
}

struct TooltipHost {
  shown: Signal<bool>,
  /// The pointer rests on the anchor: a framed text's control shows it.
  resting: Signal<bool>,
  timer: Timeout,
  anchor: ElementRef,
  /// A cut text as it was last laid out.
  cut: ElementRef,
}

impl Component for TooltipHost {
  type Props = Tooltip;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    Self {
      shown,
      resting: ctx.signal(false),
      timer: ctx.create_timeout(TOOLTIP_DELAY, move || show.set(true)),
      anchor: ctx.element_ref(),
      cut: ctx.element_ref(),
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
    // What the last layout drew of a cut text: whole, or ended by its ellipsis.
    let fits = !self.cut.text_cut();
    if let Some(words) = tooltip.words(fits).filter(|_| self.shown.get()) {
      let hung = tooltip.lurq(words).placement(placement(&tooltip));
      let hung = match &tooltip.over {
        Some(Over(frame)) => hung.over(frame),
        None => hung,
      };
      anchor = anchor.child(hung.hang(&self.anchor, true));
    }
    anchor
  }
}

/// Where `tooltip` opens from its anchor: under it, or over it clear of its frame, from its start or its end.
const fn placement(tooltip: &Tooltip) -> Placement {
  match (tooltip.over.is_some(), tooltip.end) {
    (true, true) => Placement::TopEnd,
    (true, false) => Placement::TopStart,
    (false, true) => Placement::BottomEnd,
    (false, false) => Placement::BottomStart,
  }
}

impl TooltipHost {
  /// The cut text in `anchor`, which gives up room when its row runs out of it. A framed text's anchor is the control
  /// it names.
  fn cut_text(&self, anchor: Row, tooltip: &Tooltip, cut: &CutText) -> Row {
    let resting = self.resting.get();
    let color = match &cut.frame {
      Some(frame) if resting => Some(frame.hovered.clone()),
      _ => cut.color.clone(),
    };
    let text = Text::new(&tooltip.text).variant(cut.typography).nowrap();
    let text = match color {
      Some(color) => text.color(color),
      None => text,
    };
    let anchor = match &cut.frame {
      Some(frame) => (frame.lay_out)(anchor, resting),
      None => anchor,
    };
    anchor.flex_shrink(1.0).min_width(Dimension::Px(0.0)).child(
      text
        .id(tooltip.id.to_string())
        .describe("a11y_name", tooltip.text.clone())
        .ref_element(self.cut.clone())
        .max_width(Dimension::Pct(100.0))
        .text_overflow(TextOverflow::Elipsis),
    )
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
