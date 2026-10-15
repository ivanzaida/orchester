//! Values of the Orchestrator chat (`TrG83`) and its settings (`Xir64`) that no other screen uses.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorText {
  /// 13/600 `text_primary`: History's title (`mms4o` in `Panel · History`).
  PanelTitle,
  /// 12.5/500 `text_primary`: a conversation's title in History (`M8nDR` in `Conversation row` `V37fU`), what a
  /// loading conversation or a failed send says (`pYcX8`, `Tq6wM` in `kDOn9`).
  Emphasis,
  /// Geist Mono 11 `text_muted`, tracking 0.6: the suggestions' heading (`E1Bgj`).
  Eyebrow,
  /// 12 `text_secondary`, 1.4: a suggestion (`lzBZf` in `Prompt chip` `gFvYI`).
  Suggestion,
  /// 11/500 `text_primary`: *Manage context* after the context line (`q7W1f`).
  ContextLink,
}

impl OrchestratorText {
  pub const ALL: [Self; 5] = [
    Self::PanelTitle,
    Self::Emphasis,
    Self::Eyebrow,
    Self::Suggestion,
    Self::ContextLink,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PanelTitle => "orchestrator_panel_title",
      Self::Emphasis => "orchestrator_emphasis",
      Self::Eyebrow => "orchestrator_eyebrow",
      Self::Suggestion => "orchestrator_suggestion",
      Self::ContextLink => "orchestrator_context_link",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::PanelTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Emphasis => style(12.5, FontWeight::Medium, 1.35, 0.0, palette.text_primary),
      Self::Eyebrow => mono(style(11.0, FontWeight::Normal, 1.3, 0.6, palette.text_muted)),
      Self::Suggestion => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::ContextLink => style(11.0, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the Orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorSpacing {
  /// 20: between the start's intro, composer and suggestions (`Start` `ktXkH`).
  Start,
  /// 40: the settings page's sides (`Head` `b8gGj`, `Page body` `WAGD4`).
  SettingsPage,
  /// 18: over a day's label in History after the first (`kPs8W`: about 30 from the row before).
  HistoryDay,
}

impl OrchestratorSpacing {
  pub const ALL: [Self; 3] = [Self::Start, Self::SettingsPage, Self::HistoryDay];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Start => "orchestrator_start",
      Self::SettingsPage => "orchestrator_settings_page",
      Self::HistoryDay => "orchestrator_history_day",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Start => 20.0,
      Self::SettingsPage => 40.0,
      Self::HistoryDay => 18.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  OrchestratorText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in OrchestratorSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
