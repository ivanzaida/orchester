//! Values of the `/watching` answer (ORC-775, `06 · /watching` `Ff7AD` in `Orchestrator watch · redesign`): its list
//! of what the conversation leads (`Watching` `SuoBc`), one head row per subscription and a row per task under it.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchingText {
  /// 13 `text_primary`, 1.6: what the answer says over its list, the one-line total (`E1LN7W` in `SJcW9`).
  Answer,
  /// 12.5/500 `text_primary`, 1.3: a subscription's name on its head row (`DY5UD` in `mRauZ`).
  Name,
  /// 11.5 `text_muted`, 1.3: a head row's kind, since and order (`dB4Vx`), a task's note (`M4Ftl`), the rest counted
  /// (`mZ4Wv` in `DMq1Y`).
  Meta,
  /// Geist Mono 11.5 `text_secondary`, 1.3: a task's key and run (`n9WR4` in `fzdsr`).
  Key,
  /// 12.5 `text_primary`, 1.3: a task's title (`ugPXE` in `fzdsr`).
  Title,
  /// 12 `text_secondary`, 1.3: *Stop watching* (`Q8wFp4`) and a task's link, *Decide ↗* (`MJxAI` in `rUNnk`).
  Action,
}

impl WatchingText {
  pub const ALL: [Self; 6] = [
    Self::Answer,
    Self::Name,
    Self::Meta,
    Self::Key,
    Self::Title,
    Self::Action,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Answer => "watching_answer",
      Self::Name => "watching_name",
      Self::Meta => "watching_meta",
      Self::Key => "watching_key",
      Self::Title => "watching_title",
      Self::Action => "watching_action",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Answer => style(13.0, FontWeight::Normal, 1.6, 0.0, palette.text_primary),
      Self::Name => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::Meta => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Key => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::Title => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Action => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
    }
  }
}

/// Gaps and paddings of the `/watching` list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchingSpacing {
  /// 12: a row's sides (`mRauZ`, `fzdsr`).
  Side,
  /// 34: a task row's start, past its subscription's icon (`fzdsr`, `DMq1Y`).
  Indent,
}

impl WatchingSpacing {
  pub const ALL: [Self; 2] = [Self::Side, Self::Indent];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Side => "watching_side",
      Self::Indent => "watching_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Side => 12.0,
      Self::Indent => 34.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  WatchingText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in WatchingSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
