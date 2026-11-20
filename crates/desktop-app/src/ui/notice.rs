//! Notice (`Notice` `R3Pyx`): lurq's `ui::Alert` in the kit's recipe (`theme::kit::notice`); a 16 icon, a title in
//! 13/500 `text_primary` with its explanation 2 under it in 12/1.45 `text_secondary`, and actions at the end, 10 apart
//! and 8 between actions; radius 10, 12 over and under and 14 at its sides, inside a 1 stroke. Its tone colors the
//! fill, the stroke and the icon: information on `info_muted` in `info`, as the redesigned screens draw it (`Notice ·
//! info` `byr8E` in `cC55E`, `HfwWH` in `P21bwh`), a warning on `warning_muted` in `warning` (`iw9rS`), a
//! failure on `danger_muted` in `danger` (`kXWCj`, `fRrDA`), a success on `success_muted` in `success`
//! (`nqp68`). The text wraps and the icon stays with the first line.
//!
//! A flat notice ([`Notice::flat`], `Notice · warning` `Mxt7k` in the redesigned Pipelines `evQ4U`): its tone's
//! fill without a stroke, radius 6, 10 over and under and 12 at its sides, a 14 icon in its tone's color 10 from
//! the text, the title, its explanation and its actions 6 apart.
//!
//! A raised notice ([`Notice::raised`], `Notice · danger` `b3htnP` in `Tasks cannot be read` `DNeq9`):
//! `surface_raised` in its tone's stroke, radius 6, 12 over and under and 14 at its sides; a 15 icon in its
//! tone's color 10 from the title, the explanation 4 under the title, and its actions 10 under the words at
//! the notice's end.
//!
//! Handling notice (`Handling notice` `mERxM`): how something is handled, in one paragraph: a 15
//! `text_secondary` icon and the words in 12/1.45 `text_secondary`, 10 apart; radius 8, 10 over and under and
//! 12 at its sides, `surface_raised` in a `border` stroke.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
  ui::{Alert, AlertPart, AlertVariant, Styled},
};

use super::icon::{Icon, icon};
use crate::theme::{
  ExtraSpacing, IconSize,
  kit::notice::{FLAT, HANDLING, RAISED, STROKED},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
  /// A fact about the thing the notice stands in (an edit not saved yet, an archived resource): `surface_raised` in a
  /// `border` stroke, its glyph `text_muted` (`Edit bar` `Z4FS4` in L8, `Notice · archived` `E4N09` in L14).
  Neutral,
  Info,
  Warning,
  Danger,
  Success,
}

impl NoticeTone {
  /// The tone's name, as tools read it from an element's `tone`.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Neutral => "neutral",
      Self::Info => "info",
      Self::Warning => "warning",
      Self::Danger => "danger",
      Self::Success => "success",
    }
  }

  /// The fill, the stroke and the icon's color.
  pub(crate) const fn colors(self) -> (PaletteColor, PaletteColor, PaletteColor) {
    match self {
      Self::Neutral => (PaletteColor::Card, PaletteColor::Border, PaletteColor::MutedForeground),
      Self::Info => (PaletteColor::InfoMuted, PaletteColor::Info, PaletteColor::Info),
      Self::Warning => (PaletteColor::WarningMuted, PaletteColor::Warning, PaletteColor::Warning),
      Self::Danger => (
        PaletteColor::DestructiveMuted,
        PaletteColor::Destructive,
        PaletteColor::Destructive,
      ),
      Self::Success => (PaletteColor::SuccessMuted, PaletteColor::Success, PaletteColor::Success),
    }
  }

  /// The variant of the notice's recipe (`theme::kit::notice`) that draws this tone.
  pub(crate) const fn variant(self) -> AlertVariant {
    match self {
      Self::Neutral => AlertVariant::Neutral,
      Self::Info => AlertVariant::Info,
      Self::Warning => AlertVariant::Warning,
      Self::Danger => AlertVariant::Destructive,
      Self::Success => AlertVariant::Success,
    }
  }

  /// The icon a notice of this tone shows unless it names its own.
  pub(crate) const fn icon(self) -> Icon {
    match self {
      Self::Neutral | Self::Info => Icon::Info,
      Self::Warning => Icon::TriangleAlert,
      Self::Danger => Icon::CircleAlert,
      Self::Success => Icon::CircleCheck,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoticeLayout {
  Inline,
  Inspector,
  Stacked,
  Raised,
}

pub struct Notice {
  id: Arc<str>,
  tone: NoticeTone,
  icon: Icon,
  /// The icon's color where it is not its tone's.
  mark: Option<PaletteColor>,
  title: Arc<str>,
  body: Option<Arc<str>>,
  /// What follows the explanation, e.g. a command to run.
  parts: Vec<Element>,
  actions: Vec<Element>,
  layout: NoticeLayout,
  /// Drawn as the redesigned screens draw it: no stroke, a smaller icon, its parts closer.
  flat: bool,
  /// The title's style where it is not the notice's own.
  title_style: Option<TypographyStyle>,
}

impl Notice {
  pub fn new(id: impl Into<Arc<str>>, tone: NoticeTone, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      tone,
      icon: tone.icon(),
      mark: None,
      title: title.into(),
      body: None,
      parts: Vec::new(),
      actions: Vec::new(),
      layout: NoticeLayout::Inline,
      flat: false,
      title_style: None,
    }
  }

  /// The icon that says what the notice is about, in place of its tone's.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = icon;
    self
  }

  /// The icon in `color` instead of its tone's: a failure that leaves what is shown in place reads as
  /// information with a `danger` icon (`Notice · Couldn’t reach GitHub` in `PEBSm`).
  pub fn mark(mut self, color: PaletteColor) -> Self {
    self.mark = Some(color);
    self
  }

  /// The title in `style` instead of the notice's own, where a design draws it otherwise.
  pub fn title_style(mut self, style: TypographyStyle) -> Self {
    self.title_style = Some(style);
    self
  }

  /// What happened and what can be done, under the title.
  pub fn body(mut self, body: impl Into<Arc<str>>) -> Self {
    self.body = Some(body.into());
    self
  }

  /// Compact vertical notice in a 340px inspector (adK4e / A0a2p).
  pub fn inspector(mut self) -> Self {
    self.layout = NoticeLayout::Inspector;
    self
  }

  /// Actions below the explanation, aligned with its text (SSbRV in GxmNs).
  pub fn stacked(mut self) -> Self {
    self.layout = NoticeLayout::Stacked;
    self
  }

  /// The redesigned screens' notice (`Notice · warning` `Mxt7k` in `evQ4U`): its tone's fill without a stroke, a 14
  /// icon, and its actions under its words.
  pub fn flat(mut self) -> Self {
    self.flat = true;
    self
  }

  /// Drawn on `surface_raised` with its actions under its words at its end, as a failure to read is
  /// (`Notice · danger` `b3htnP` in `DNeq9`).
  pub fn raised(mut self) -> Self {
    self.layout = NoticeLayout::Raised;
    self
  }

  /// `part` under the explanation, after the ones added before it: a command to run in a terminal
  /// (`Command` in `Account notice` `CWJuh`). Stacked actions follow the parts.
  pub fn part(mut self, part: impl Into<Element>) -> Self {
    self.parts.push(part.into());
    self
  }

  /// An action at the notice's end, after the ones added before it.
  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }
}

impl From<Notice> for Element {
  fn from(notice: Notice) -> Self {
    let layout = notice.layout;
    let flat = notice.flat && matches!(layout, NoticeLayout::Inline | NoticeLayout::Stacked);
    // A flat notice has a stroke only where it is neutral: the others are their tone's fill alone.
    let stroked = !flat || notice.tone == NoticeTone::Neutral;
    let mut alert = Alert::new(&notice.title)
      .id(notice.id.clone())
      .variant(notice.tone.variant())
      .icon(notice.icon.name().as_str())
      .children(notice.parts)
      .actions(notice.actions)
      .align_top(layout == NoticeLayout::Inline && !flat)
      .stacked(flat || matches!(layout, NoticeLayout::Stacked | NoticeLayout::Raised))
      .actions_end(layout == NoticeLayout::Raised)
      .compact(layout == NoticeLayout::Inspector)
      .state(FLAT, flat)
      .state(RAISED, layout == NoticeLayout::Raised)
      .state(STROKED, stroked)
      .describe("tone", notice.tone.name());
    if let Some(body) = &notice.body {
      alert = alert.description(body);
    }
    if let Some(mark) = notice.mark {
      alert = alert.style(AlertPart::Icon, |style| style.foreground(mark));
    }
    if let Some(title) = notice.title_style {
      alert = alert.style(AlertPart::Title, |style| style.typography(title));
    }
    alert.into()
  }
}

/// A handling notice: `text` beside `glyph` (`<id>-description`).
pub fn handling_notice(id: &str, glyph: Icon, text: &str) -> Alert {
  handling_notice_content(id, glyph, text)
}

/// The in-notice link of mERxM/AuGQr, used to open an existing credential (Gam8g), under the words.
pub fn handling_notice_action(
  id: &str,
  action_id: &str,
  glyph: Icon,
  text: &str,
  label: &str,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Alert {
  let action = super::pressable::pressable(on_click)
    .id(action_id.to_owned())
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(Text::new(label).variant(TypographyStyle::Label))
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::Foreground));
  handling_notice_content(id, glyph, text).stacked(true).action(action)
}

fn handling_notice_content(id: &str, glyph: Icon, text: &str) -> Alert {
  Alert::untitled()
    .id(id.to_owned())
    .variant(AlertVariant::Neutral)
    .icon(glyph.name().as_str())
    .description(text)
    .state(HANDLING, true)
    .state(STROKED, true)
}

/// Inline publication result below the editor header (GxmNs, state 7).
pub fn result_notice(id: &str, text: &str, dismiss: Element) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Tabs.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::CircleCheck, IconSize::Sm, PaletteColor::Success))
    .child(Text::new(text).variant(TypographyStyle::Description).flex(1.0))
    .child(dismiss)
}
