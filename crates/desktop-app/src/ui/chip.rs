//! Chips: lurq's `ui::Tag` and `ui::Badge` in the theme's recipes (`theme::kit::chip`, which says how they look). A
//! tag (`Chip` `jI8af`) is a name, removable or led by a glyph; a badge (`Badge` `Fm7lc`) a dot or a glyph and a word
//! in its tone (`Saved` in `success`, `New` in `info`); a cut one keeps to its room.

use std::sync::Arc;

use lurq::{
  app::theme::TypographyStyle,
  node::Element,
  ui::{Badge, BadgePart, Styled, Tag},
};

use super::icon::Icon;
use crate::theme::kit::chip as variant;

/// The cross of a removable tag: what tools call it, and what a press does.
struct Remove {
  label: Arc<str>,
  action: Arc<dyn Fn() + Send + Sync>,
}

pub struct TagChip {
  id: Arc<str>,
  name: Arc<str>,
  remove: Option<Remove>,
  /// The name is an identifier, in Geist Mono (`Chip · search_docs` in `XRCed`).
  mono: bool,
  /// A form's pill under the field that adds it, 24 tall (`Pill · deepseek-v4-pro` `TnP0h`).
  pill: bool,
  /// The glyph before the name.
  glyph: Option<Icon>,
}

impl TagChip {
  pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      remove: None,
      mono: false,
      pill: false,
      glyph: None,
    }
  }

  /// `glyph` before the name: what the chip says something is, e.g. a memory record's type.
  #[must_use]
  pub const fn glyph(mut self, glyph: Icon) -> Self {
    self.glyph = Some(glyph);
    self
  }

  /// The name in Geist Mono: an identifier such as a tool's name.
  #[must_use]
  pub const fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// A form's pill: 24 tall with its name centred in it, as the values a field adds (`Pills` `TQ7md`).
  #[must_use]
  pub const fn pill(mut self) -> Self {
    self.pill = true;
    self
  }

  /// The cross that removes the tag; `label` names it for tools (`Remove webhooks`).
  pub fn removable(mut self, label: impl Into<Arc<str>>, remove: impl Fn() + Send + Sync + 'static) -> Self {
    self.remove = Some(Remove {
      label: label.into(),
      action: Arc::new(remove),
    });
    self
  }
}

impl From<TagChip> for Element {
  fn from(chip: TagChip) -> Self {
    let mut tag = Tag::new(&chip.name)
      .state(variant::MONO, chip.mono)
      .state(variant::FIELD, chip.pill)
      .describe("tag", chip.name.clone())
      .id(chip.id);
    if let Some(glyph) = chip.glyph {
      tag = tag.icon(glyph.name());
    }
    if let Some(Remove { label, action }) = chip.remove {
      // Pointer-only, as the kit's other controls: Tab passes the cross by. The press is the cross's alone: a row a
      // press opens around the tag hears nothing of it.
      tag = tag.remove_label(&label).tab_stop(false).on_remove_event(move |event| {
        event.stop_propagation();
        action();
      });
    }
    tag.into()
  }
}

/// The tones of a badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
  Success,
  Info,
  Warning,
  Danger,
  /// A state without a tone, e.g. a task kept *Not shared* (`R4elt`): `text_secondary` on the `hover` fill.
  Neutral,
}

impl BadgeTone {
  const fn variant(self) -> &'static str {
    match self {
      Self::Success => variant::SUCCESS,
      Self::Info => variant::INFO,
      Self::Warning => variant::WARNING,
      Self::Danger => variant::DANGER,
      Self::Neutral => variant::NEUTRAL,
    }
  }
}

/// A badge saying `word` in `tone`: its dot is `<id>-dot` and its word `<id>-label`.
pub fn badge(id: &str, tone: BadgeTone, word: &str) -> Badge {
  Badge::new(word).variant(tone.variant()).dot(true).id(id.to_owned())
}

/// [`badge`] with its word in `style`, e.g. 11/500 where a panel draws it 19 high (`Chip · Active` `cwhcK` in Y2).
pub fn badge_in(id: &str, tone: BadgeTone, word: &str, style: TypographyStyle) -> Badge {
  badge(id, tone, word).style(BadgePart::Label, |part| part.typography(style))
}

/// A badge saying `word` in `tone` that keeps to the room its row gives it: the word is cut with an ellipsis, and
/// while it is cut its whole words show in a tooltip. A word the badge does not choose, such as a team's name, may
/// be any length.
pub fn cut_badge(id: &str, tone: BadgeTone, word: &str) -> Badge {
  badge(id, tone, word).truncate(true).tooltip_when_cut(true)
}

/// A state's badge: a 12 `glyph` (`<id>-icon`) and `word` (`<id>-label`), 6 apart, in `tone` (`Badge` `sNWem` in T11a
/// `ujJ6U`).
pub fn state_badge(id: &str, tone: BadgeTone, glyph: Icon, word: &str) -> Badge {
  Badge::new(word)
    .variant(tone.variant())
    .icon(glyph.name())
    .id(id.to_owned())
}
