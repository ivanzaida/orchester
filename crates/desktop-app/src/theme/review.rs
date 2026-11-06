//! Values of a run's review page (ORC-697; 09i `wLyFt` of `Review · redesign`, and the thread and tray states of
//! `RBR9O`): its threads under their lines, their tags and messages.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The three dots while Implement starts, fading from the first (`Typing` `D231t`).
pub const REVIEW_TYPING_OPACITY: [f32; 3] = [1.0, 0.6, 0.3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewText {
  /// 11/500, 1.2: a tag in a file's head or a thread's (`NzXv8` in `GSgiW`, `wWavj` in `I1Fsj`); its color is its
  /// tag's.
  Tag,
  /// 13 `text_primary`, 1.45: what a message of a thread says, and the comment box's words (`tR5zv`, `IgR9h`).
  Message,
}

impl ReviewText {
  pub const ALL: [Self; 2] = [Self::Tag, Self::Message];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Tag => "review_tag",
      Self::Message => "review_message",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Tag => style(11.0, FontWeight::Medium, 1.2, 0.0, palette.text_muted),
      Self::Message => style(13.0, FontWeight::Normal, 1.45, 0.0, palette.text_primary),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewSpacing {
  /// 94: a thread or the comment box in from the diff's start, past the line numbers and the sign (`hxP69`).
  ThreadStart,
  /// 30: a message's words in from its avatar's start (`X97JJM`).
  BodyIndent,
}

impl ReviewSpacing {
  pub const ALL: [Self; 2] = [Self::ThreadStart, Self::BodyIndent];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ThreadStart => "review_thread_start",
      Self::BodyIndent => "review_body_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ThreadStart => 94.0,
      Self::BodyIndent => 30.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ReviewSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ReviewText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
