//! The design values of the title bar's Needs you panel (layer `Needs you` `eBhFg`: the panel `BrP6i`, `Needs
//! you group` `o3xekF`, `Needs you item` `bAeM6`) that the UI kit's roles do not carry, registered as lurq
//! `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{RadiusSize, ShadowStyle, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::box_shadow::BoxShadow,
};

use super::{ExtraColor, typography::style};

/// Text of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedsYouText {
  /// 13/600 `text_primary`: the panel's title (`xAOTs`).
  Title,
  /// 12/500 `warning`: how many wait, beside the title (`Zny5d`); `text_muted` at 0 (`FAN4a`).
  Count,
  /// 12/600 `text_primary`: a project's name over its items (`ityot` in `o3xekF`).
  Project,
  /// 10.5/600 `text_secondary`: a project's initial in its tile (`u3TYV`).
  Initial,
  /// 11.5/500 `warning`: how many wait in a project (`znWZH`).
  Tally,
  /// 11.5/600 `warning`: what an item is (`pdnV7` in `bAeM6`).
  Kind,
  /// 13/500 `text_primary`, 1.4: what an item asks (`NczSN`).
  ItemTitle,
  /// 11.5 `text_secondary`: an item's task (`uTGip`).
  Task,
  /// 12 `text_muted`, 1.5: what the empty panel explains (`RAYK9`).
  Empty,
  /// 12/500 `text_primary`: how a request that no longer waits went (`J8bQb` in `Needs you note` `FETWe`).
  NoteLine,
  /// 11 `text_muted`, 1.4: what followed, and where it is recorded (`sLi8n` in `FETWe`).
  NoteDetail,
}

impl NeedsYouText {
  pub const ALL: [Self; 11] = [
    Self::Title,
    Self::Count,
    Self::Project,
    Self::Initial,
    Self::Tally,
    Self::Kind,
    Self::ItemTitle,
    Self::Task,
    Self::Empty,
    Self::NoteLine,
    Self::NoteDetail,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "needs_you_title",
      Self::Count => "needs_you_count",
      Self::Project => "needs_you_project",
      Self::Initial => "needs_you_initial",
      Self::Tally => "needs_you_tally",
      Self::Kind => "needs_you_kind",
      Self::ItemTitle => "needs_you_item_title",
      Self::Task => "needs_you_task",
      Self::Empty => "needs_you_empty",
      Self::NoteLine => "needs_you_note_line",
      Self::NoteDetail => "needs_you_note_detail",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Count => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.warning),
      Self::Project => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Initial => style(10.5, FontWeight::SemiBold, 1.0, 0.0, palette.text_secondary),
      Self::Tally => style(11.5, FontWeight::Medium, 1.3, 0.0, palette.warning),
      Self::Kind => style(11.5, FontWeight::SemiBold, 1.3, 0.0, palette.warning),
      Self::ItemTitle => style(13.0, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
      Self::Task => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Empty => style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_muted),
      Self::NoteLine => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::NoteDetail => style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
    }
  }
}

/// Gaps and paddings of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedsYouSpacing {
  /// 7: the title row's hand, title, count and close (`kBUGK`).
  TitleRow,
  /// 24: the empty panel's sides (`oMseR`).
  EmptySides,
}

impl NeedsYouSpacing {
  pub const ALL: [Self; 2] = [Self::TitleRow, Self::EmptySides];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TitleRow => "needs_you_title_row",
      Self::EmptySides => "needs_you_empty_sides",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::TitleRow => 7.0,
      Self::EmptySides => 24.0,
    }
  }
}

/// Radii of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedsYouRadius {
  /// 5: a project's initial tile (`n4MhD`).
  Initial,
}

impl NeedsYouRadius {
  pub const ALL: [Self; 1] = [Self::Initial];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Initial => "needs_you_initial",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Initial => 5.0,
    }
  }
}

/// The panel's lift: 0 8 24 in `shadow` (`BrP6i`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedsYouShadow {
  Panel,
}

impl NeedsYouShadow {
  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Panel => "needs_you_panel",
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  NeedsYouText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn shadows() -> impl Iterator<Item = (Arc<str>, Arc<[BoxShadow]>)> {
  let panel = BoxShadow::new(0.0, 8.0, 24.0, ExtraColor::Shadow.role());
  std::iter::once((Arc::from(NeedsYouShadow::Panel.name()), Arc::from([panel])))
}

pub(super) fn install(theme: &Theme) {
  for spacing in NeedsYouSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
  for radius in NeedsYouRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
