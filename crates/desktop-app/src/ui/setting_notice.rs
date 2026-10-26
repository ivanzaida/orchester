//! What Settings says in place (the states sheets `poD7g`, `shpsk` and `ZUuMT` of the new `orchester.pen`):
//! - a notice (`Notice · warning` `TPqRf`): a 14 icon in its tone's color and, 10 after it, an optional title in 13/500
//!   `text_primary`, what happened in 12 `text_secondary`, parts such as a code, actions 8 apart and a note in 11.5
//!   `text_muted`, 6 apart; 10 over and under and 12 at its sides, on its tone's muted fill, radius 6, no stroke. A
//!   question asked in place is a notice whose actions answer it (`Rotate the token?` `XG6M1`).
//! - a panel (`Sign in` `yKzFt`, `Remove` `CTa4g`): a title in 13/600, what it says and its parts and actions 10 apart,
//!   12 over and under and 14 at its sides inside a `border` stroke, radius 10; a destructive question is on
//!   `danger_muted` inside a `danger` stroke.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  notice::NoticeTone,
};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize, SettingsText};

const fn colors(tone: NoticeTone) -> (PaletteColor, PaletteColor, Icon) {
  match tone {
    NoticeTone::Info => (PaletteColor::InfoMuted, PaletteColor::Info, Icon::Info),
    NoticeTone::Warning => (PaletteColor::WarningMuted, PaletteColor::Warning, Icon::TriangleAlert),
    NoticeTone::Danger => (PaletteColor::DangerMuted, PaletteColor::Danger, Icon::CircleAlert),
    NoticeTone::Success => (PaletteColor::SuccessMuted, PaletteColor::Success, Icon::CircleCheck),
  }
}

const fn tone_name(tone: NoticeTone) -> &'static str {
  match tone {
    NoticeTone::Info => "info",
    NoticeTone::Warning => "warning",
    NoticeTone::Danger => "danger",
    NoticeTone::Success => "success",
  }
}

/// A notice in place.
pub struct SettingNotice {
  id: Arc<str>,
  tone: NoticeTone,
  icon: Option<Icon>,
  title: Option<Arc<str>>,
  body: Option<Arc<str>>,
  parts: Vec<Element>,
  actions: Vec<Element>,
  note: Option<Arc<str>>,
}

impl SettingNotice {
  /// The notice `id` (its title `<id>-title`, its words `<id>-body`, its note `<id>-note`).
  pub fn new(id: impl Into<Arc<str>>, tone: NoticeTone) -> Self {
    Self {
      id: id.into(),
      tone,
      icon: None,
      title: None,
      body: None,
      parts: Vec::new(),
      actions: Vec::new(),
      note: None,
    }
  }

  /// The icon that says what the notice is about, in place of its tone's.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn title(mut self, title: impl Into<Arc<str>>) -> Self {
    self.title = Some(title.into());
    self
  }

  pub fn body(mut self, body: impl Into<Arc<str>>) -> Self {
    self.body = Some(body.into());
    self
  }

  /// A part under the words, before the actions.
  pub fn part(mut self, part: impl Into<Element>) -> Self {
    self.parts.push(part.into());
    self
  }

  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }

  /// A quieter line after the actions.
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }
}

impl From<SettingNotice> for Element {
  fn from(notice: SettingNotice) -> Self {
    let (fill, mark, glyph) = colors(notice.tone);
    let actions = (!notice.actions.is_empty()).then(|| {
      Row::new()
        .id(format!("{}-actions", notice.id))
        .wrap()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(notice.actions)
    });
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(notice.title.map(|title| {
        Text::new(&title)
          .id(format!("{}-title", notice.id))
          .variant(ExtraTypography::Strong.role())
          .width(Dimension::Pct(100.0))
      }))
      .with_children(notice.body.map(|body| {
        Text::new(&body)
          .id(format!("{}-body", notice.id))
          .variant(ExtraTypography::Prose.role())
          .width(Dimension::Pct(100.0))
      }))
      .with_children(notice.parts)
      .with_children(actions)
      .with_children(notice.note.map(|note| {
        Text::new(&note)
          .id(format!("{}-note", notice.id))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0))
      }));
    Row::new()
      .id(notice.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .background(fill)
      .rounded(RadiusSize::Md)
      .describe("tone", tone_name(notice.tone))
      .child(icon(notice.icon.unwrap_or(glyph), IconSize::Md, mark).id(format!("{}-icon", notice.id)))
      .child(text)
      .into()
  }
}

/// A panel in place: a step to take or a question to answer.
pub struct SettingPanel {
  id: Arc<str>,
  title: Arc<str>,
  danger: bool,
  parts: Vec<Element>,
}

impl SettingPanel {
  /// The panel `id` titled `title` (`<id>-title`).
  pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      danger: false,
      parts: Vec::new(),
    }
  }

  /// A destructive question: on `danger_muted` inside a `danger` stroke.
  pub fn danger(mut self) -> Self {
    self.danger = true;
    self
  }

  /// What it says, as a paragraph in 12 `text_secondary`, after the parts before it.
  pub fn text(self, id: &str, text: &str) -> Self {
    self.part(
      Text::new(text)
        .id(id.to_owned())
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
  }

  /// A quieter line in 11.5 `text_muted`.
  pub fn note(self, id: &str, text: &str) -> Self {
    self.part(
      Text::new(text)
        .id(id.to_owned())
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
  }

  pub fn part(mut self, part: impl Into<Element>) -> Self {
    self.parts.push(part.into());
    self
  }

  /// Its actions on one line, 8 apart.
  pub fn actions(self, actions: impl IntoIterator<Item = Element>) -> Self {
    self.part(
      Row::new()
        .wrap()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(actions),
    )
  }
}

impl From<SettingPanel> for Element {
  fn from(panel: SettingPanel) -> Self {
    let frame = Column::new()
      .id(panel.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .rounded(RadiusSize::Lg)
      .child(
        Text::new(&panel.title)
          .id(format!("{}-title", panel.id))
          .variant(SettingsText::SectionTitle.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(panel.parts);
    let frame = if panel.danger {
      frame
        .background(PaletteColor::DangerMuted)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
        .describe("tone", "danger")
    } else {
      frame.border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    };
    frame.into()
  }
}

/// A line of what is going on: a 13 icon and its words in 12 `text_secondary`, 8 apart (`Line` `wJefD`).
pub fn status_line(id: &str, glyph: Icon, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Prose.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
