//! Values of the Orchestrator chat (`TrG83`) and its settings (`Xir64`) that no other screen uses; its replies'
//! links (`Orchestrator · links · states` `agv8r`).

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, ShadowStyle, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::box_shadow::BoxShadow,
};

use super::{
  palette::ExtraColor,
  typography::{mono, style},
};

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
  /// 11.5 `text_muted`, 1.3: what a fact of a proposal card is (`Key` `u5vGv` in `b55NqO`).
  FactKey,
  /// 12.5 `text_secondary`, 1.3: a fact's value (`PX8jm` in `b55NqO`).
  FactValue,
  /// 12.5/500 `accent_text`, 1.3: a fact that opens its place, over a 1 `text_muted` line (`d0wPU8` in `keqvu`).
  FactLink,
  /// 12 `text_muted`, 1.3: what a fact adds after its links (*2 runs under way keep v3*, `KmSjB`).
  FactAside,
  /// Geist Mono 11.5 `text_secondary`, 1.3: a branch or command as a fact (`O4epE` in `Ee2JE`).
  FactMono,
  /// 11 `text_muted`, 1.3: *What changes* over a draft's diff (`BlMyH`).
  DiffLabel,
  /// Geist Mono 11 `text_muted`: a line's numbers in a draft's diff (`ey7iM` in `kzMBc`).
  DiffNumber,
  /// Geist Mono 12 `text_primary`: a line of a draft's diff, and its sign (`VX2mn`, `OtSVu` in `kzMBc`).
  DiffCode,
  /// 11.5 `text_muted`: the host after a link to the web (`zKxtr` in `agv8r`).
  LinkHost,
  /// Geist Mono 11.5 `text_muted`: the address after the words of a link the chat does not follow (`Suuac` in
  /// `agv8r`).
  LinkAddress,
}

impl OrchestratorText {
  pub const ALL: [Self; 15] = [
    Self::PanelTitle,
    Self::Emphasis,
    Self::Eyebrow,
    Self::Suggestion,
    Self::ContextLink,
    Self::FactKey,
    Self::FactValue,
    Self::FactLink,
    Self::FactAside,
    Self::FactMono,
    Self::DiffLabel,
    Self::DiffNumber,
    Self::DiffCode,
    Self::LinkHost,
    Self::LinkAddress,
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
      Self::FactKey => "orchestrator_fact_key",
      Self::FactValue => "orchestrator_fact_value",
      Self::FactLink => "orchestrator_fact_link",
      Self::FactAside => "orchestrator_fact_aside",
      Self::FactMono => "orchestrator_fact_mono",
      Self::DiffLabel => "orchestrator_diff_label",
      Self::DiffNumber => "orchestrator_diff_number",
      Self::DiffCode => "orchestrator_diff_code",
      Self::LinkHost => "orchestrator_link_host",
      Self::LinkAddress => "orchestrator_link_address",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::PanelTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Emphasis => style(12.5, FontWeight::Medium, 1.35, 0.0, palette.text_primary),
      Self::Eyebrow => mono(style(11.0, FontWeight::Normal, 1.3, 0.6, palette.text_muted)),
      Self::Suggestion => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::ContextLink => style(11.0, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
      Self::FactKey => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::FactValue => style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::FactLink => style(
        12.5,
        FontWeight::Medium,
        1.3,
        0.0,
        palette.get(ExtraColor::AccentText.role()),
      ),
      Self::FactAside => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::FactMono => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::DiffLabel => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::DiffNumber => mono(style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::DiffCode => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::LinkHost => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::LinkAddress => mono(style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
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

/// Radii of the Orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorRadius {
  /// 3: a link's hover fill (top corners) and focus ring (`K52XFs`, `Mjhop` in `agv8r`).
  Link,
}

impl OrchestratorRadius {
  pub const ALL: [Self; 1] = [Self::Link];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Link => "orchestrator_link",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Link => 3.0,
    }
  }
}

/// Shadows of the Orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrchestratorShadow {
  /// A focused link's ring: 2 of `border_focus` outside its edge (`Mjhop` in `agv8r`), where the chrome's focus
  /// ring sits inside a control's edge.
  LinkFocus,
}

impl OrchestratorShadow {
  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::LinkFocus => "orchestrator_link_focus",
    }
  }
}

/// The line height of a link's words in a reply, so its underline sits 1 under them (`qgLX8` in `agv8r`); the
/// words keep the size of the text around them.
pub const LINK_LINE_HEIGHT: f32 = 1.3;

/// The width of a focused link's ring.
const LINK_FOCUS_WIDTH: f32 = 2.0;

pub(super) fn shadows() -> impl Iterator<Item = (Arc<str>, Arc<[BoxShadow]>)> {
  let ring = BoxShadow::new(0.0, 0.0, 0.0, PaletteColor::BorderFocus).spread(LINK_FOCUS_WIDTH);
  std::iter::once((Arc::from(OrchestratorShadow::LinkFocus.name()), Arc::from([ring])))
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
  for radius in OrchestratorRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
