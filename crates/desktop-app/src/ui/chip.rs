//! Chips. A tag (`Chip` `jI8af`) is its name in 11/500 `text_secondary` on a `surface_raised` pill
//! inside a `border` stroke, 2 over and under it and 8 at its sides; a removable one ends in an 11
//! `text_muted` cross, 4 after the name and 4 from its end (`Chip/removable` `qj3Ss`). A badge (`Badge`
//! `Fm7lc`) is a 6 dot and a word in 11/500 in its tone, 6 apart, on the tone's muted fill, 3 over and
//! under them and 8 at their sides (`Saved` in `success`, `New` in `info`); a cut one keeps to its room.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  tooltip::Truncated,
};
use crate::theme::{ControlHeight, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// A badge's dot (`Vydkx`).
const DOT_SIZE: f32 = 6.0;

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
}

impl TagChip {
  pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      remove: None,
      mono: false,
      pill: false,
    }
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
    let padding = Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role());
    let mut row = Row::new()
      .id(chip.id.clone())
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Full.role())
      .describe("tag", chip.name.clone())
      .child(
        Text::new(&chip.name)
          .id(format!("{}-label", chip.id))
          .variant(if chip.mono {
            ExtraTypography::MonoLabel.role()
          } else {
            TypographyStyle::FieldLabel
          })
          .nowrap(),
      );
    if chip.pill {
      row = row.height(Dimension::Px(ControlHeight::Sm.value()));
    }
    let Some(Remove { label, action }) = chip.remove else {
      return row.padding(padding).into();
    };
    // A press removes the tag only: it does not also press a row or a picker around it.
    let cross = Button::empty()
      .id(format!("{}-remove", chip.id))
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", label)
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        action();
      })
      .child(icon(Icon::X, IconSize::Xs, PaletteColor::TextMuted));
    row.padding(padding.right(SpacingSize::Xs)).child(cross).into()
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
  const fn colors(self) -> (PaletteColor, PaletteColor) {
    match self {
      Self::Success => (PaletteColor::Success, PaletteColor::SuccessMuted),
      Self::Info => (PaletteColor::Info, PaletteColor::InfoMuted),
      Self::Warning => (PaletteColor::Warning, PaletteColor::WarningMuted),
      Self::Danger => (PaletteColor::Danger, PaletteColor::DangerMuted),
      Self::Neutral => (PaletteColor::TextSecondary, PaletteColor::AccentMuted),
    }
  }
}

/// A badge saying `word` in `tone`.
pub fn badge(id: &str, tone: BadgeTone, word: &str) -> Row {
  let color = tone.colors().0;
  badge_frame(id, tone).child(
    Text::new(word)
      .id(format!("{id}-label"))
      .variant(TypographyStyle::FieldLabel)
      .color(color)
      .nowrap(),
  )
}

/// A badge saying `word` in `tone` that keeps to the room its row gives it: the word is cut with an ellipsis, and
/// while it is cut its whole words show in a tooltip. A word the badge does not choose, such as a team's name, may
/// be any length.
pub fn cut_badge(ctx: &mut Ctx, id: &str, tone: BadgeTone, word: &str) -> Row {
  let color = tone.colors().0;
  badge_frame(id, tone)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      Truncated::new(format!("{id}-label"), word.to_owned(), TypographyStyle::FieldLabel)
        .color(color)
        .render(ctx),
    )
}

/// A badge's pill and its dot, before its word.
fn badge_frame(id: &str, tone: BadgeTone) -> Row {
  let (color, fill) = tone.colors();
  Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Fine.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .background(fill)
    .rounded(ExtraRadius::Full.role())
    .child(
      Row::new()
        .id(format!("{id}-dot"))
        .width(Dimension::Px(DOT_SIZE))
        .height(Dimension::Px(DOT_SIZE))
        .flex_shrink(0.0)
        .rounded(ExtraRadius::Full.role())
        .background(color),
    )
}
