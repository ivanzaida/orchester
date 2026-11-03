//! Values of the side question pane beside a run's Conversation (ORC-337; `Side question` `q4Nz4i` in 12b `gC8a7`,
//! `Gm19K` in 12 `j8r9z`, `tqdDd` in 12c `Y595T`, the states in 12d `GVKQD`) that the UI kit's roles do not carry.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideText {
  /// 13/600 `text_primary`, 1.3: *Side question* in the pane's head (`g857c`).
  Title,
  /// 11.5 `text_muted`, 1.3: *KON-12 · run 9* (`Dwxw5`), the answer's model and time (`IYQmr`), *Not sent* (`l2sSz`),
  /// *Reads only · changes nothing* (`GfcJb`).
  Quiet,
  /// 12 `text_secondary`, 1.45: the scope line (`MtduG`).
  Scope,
  /// 13 `text_primary`, 1.55: the question in its bubble (`Vc0ev`) and the answer (`B6mJAw`).
  Body,
  /// 11 `text_muted`, 1.3: *Asked 15:26 · only you see it* (`F1n7O6`).
  Meta,
  /// 12/500 `text_primary`, 1.3: *Side question* over the answer (`nIJns`).
  Who,
  /// Geist Mono 12 `text_primary`, 1.3: a source's path or command (`V9y1sX`, `mJTSd`).
  SourceMono,
  /// 12 `text_primary`, 1.3: a message's author as a source (`z6OHs`).
  Source,
  /// 12 `text_muted`, 1.3: what a source is, *+12 −0*, *failed* (`pbIsM`).
  Detail,
  /// 12 `text_secondary`, 1.3: *Open ↗* at a source's end (`r9mNx`).
  Open,
  /// 12 `danger`, 1.45: *Couldn't answer: …* (12d).
  Failed,
}

impl SideText {
  pub const ALL: [Self; 11] = [
    Self::Title,
    Self::Quiet,
    Self::Scope,
    Self::Body,
    Self::Meta,
    Self::Who,
    Self::SourceMono,
    Self::Source,
    Self::Detail,
    Self::Open,
    Self::Failed,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "side_title",
      Self::Quiet => "side_quiet",
      Self::Scope => "side_scope",
      Self::Body => "side_body",
      Self::Meta => "side_meta",
      Self::Who => "side_who",
      Self::SourceMono => "side_source_mono",
      Self::Source => "side_source",
      Self::Detail => "side_detail",
      Self::Open => "side_open",
      Self::Failed => "side_failed",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Quiet => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Scope => style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::Body => style(13.0, FontWeight::Normal, 1.55, 0.0, palette.text_primary),
      Self::Meta => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Who => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::SourceMono => mono(style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary)),
      Self::Source => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Detail => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Open => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Failed => style(12.0, FontWeight::Normal, 1.45, 0.0, palette.danger),
    }
  }
}

/// Gaps and paddings of the pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideSpacing {
  /// 12: the answer's avatar to its text (`rVqTf`), over the pane's composer (`v3gD7f`).
  Gap,
  /// 4: a source row's sides (`c3fRuu`), over the answer's actions (`U6rRBx`).
  Edge,
}

impl SideSpacing {
  pub const ALL: [Self; 2] = [Self::Gap, Self::Edge];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Gap => "side_gap",
      Self::Edge => "side_edge",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Gap => 12.0,
      Self::Edge => 4.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SideText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in SideSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
