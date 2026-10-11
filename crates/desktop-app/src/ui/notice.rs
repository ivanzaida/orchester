//! Notice (`Notice` `R3Pyx`): a 16 icon, a title in 13/500 `text_primary` with its explanation 2 under
//! it in 12/1.45 `text_secondary`, and actions at the end, 10 apart and 8 between actions; radius 10,
//! 12 over and under and 14 at its sides, inside a 1 stroke. Its tone colors the fill, the stroke and
//! the icon: information on `surface_raised` in a `border` stroke with a `text_secondary` icon (`IJCZY`,
//! `uqbUH`), a warning on `warning_muted` in `warning` (`iw9rS`), a failure on `danger_muted` in
//! `danger` (`kXWCj`, `fRrDA`), a success on `success_muted` in `success` (`nqp68`). The text wraps and
//! the icon stays with the first line.
//!
//! Handling notice (`Handling notice` `mERxM`): how something is handled, in one paragraph: a 15
//! `text_secondary` icon and the words in 12/1.45 `text_secondary`, 10 apart; radius 8, 10 over and under and
//! 12 at its sides, `surface_raised` in a `border` stroke.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
  Info,
  Warning,
  Danger,
  Success,
}

impl NoticeTone {
  const fn name(self) -> &'static str {
    match self {
      Self::Info => "info",
      Self::Warning => "warning",
      Self::Danger => "danger",
      Self::Success => "success",
    }
  }

  /// The fill, the stroke and the icon's color.
  const fn colors(self) -> (PaletteColor, PaletteColor, PaletteColor) {
    match self {
      Self::Info => (
        PaletteColor::SurfaceRaised,
        PaletteColor::Border,
        PaletteColor::TextSecondary,
      ),
      Self::Warning => (PaletteColor::WarningMuted, PaletteColor::Warning, PaletteColor::Warning),
      Self::Danger => (PaletteColor::DangerMuted, PaletteColor::Danger, PaletteColor::Danger),
      Self::Success => (PaletteColor::SuccessMuted, PaletteColor::Success, PaletteColor::Success),
    }
  }

  /// The icon a notice of this tone shows unless it names its own.
  const fn icon(self) -> Icon {
    match self {
      Self::Info => Icon::Info,
      Self::Warning => Icon::TriangleAlert,
      Self::Danger => Icon::CircleAlert,
      Self::Success => Icon::CircleCheck,
    }
  }
}

pub struct Notice {
  id: Arc<str>,
  tone: NoticeTone,
  icon: Icon,
  /// The icon's color where it is not its tone's.
  mark: Option<PaletteColor>,
  title: Arc<str>,
  body: Option<Arc<str>>,
  actions: Vec<Element>,
  inspector: bool,
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
      actions: Vec::new(),
      inspector: false,
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

  /// What happened and what can be done, under the title.
  pub fn body(mut self, body: impl Into<Arc<str>>) -> Self {
    self.body = Some(body.into());
    self
  }

  /// Compact vertical notice in a 340px inspector (adK4e / A0a2p).
  pub fn inspector(mut self) -> Self {
    self.inspector = true;
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
    if notice.inspector {
      return inspector_notice(notice);
    }
    let (fill, stroke, mark) = notice.tone.colors();
    let mark = notice.mark.unwrap_or(mark);
    let text = Column::new()
      .id(format!("{}-text", notice.id))
      .flex(1.0)
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(&notice.title)
          .id(format!("{}-title", notice.id))
          .variant(ExtraTypography::Strong.role()),
      )
      .with_children(
        notice
          .body
          .as_deref()
          .map(|body| Text::new(body).variant(ExtraTypography::Prose.role())),
      );
    let actions = (!notice.actions.is_empty()).then(|| {
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(notice.actions)
    });
    Row::new()
      .id(notice.id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Lg)
      .describe("tone", notice.tone.name())
      .child(icon(notice.icon, IconSize::Lg, mark).id(format!("{}-icon", notice.id)))
      .child(text)
      .with_children(actions)
      .into()
  }
}

fn inspector_notice(notice: Notice) -> Element {
  let (fill, stroke, mark) = notice.tone.colors();
  Column::new()
    .id(notice.id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(ExtraRadius::Tile.role())
    .describe("tone", notice.tone.name())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Start)
        .child(icon(notice.icon, IconSize::Base, mark))
        .child(
          Text::new(&notice.title)
            .variant(lurq::app::theme::TypographyStyle::Label)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        ),
    )
    .with_children(notice.body.map(|body| {
      Text::new(&body)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Pct(100.0))
    }))
    .with_children(
      (!notice.actions.is_empty()).then(|| Row::new().wrap().spacing(SpacingSize::Sm).with_children(notice.actions)),
    )
    .into()
}

/// A handling notice: `text` beside `glyph`.
pub fn handling_notice(id: &str, glyph: Icon, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Prose.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
