//! The design values of the Conversation tab (`Task · Conversation · working` `D1PYAe`, its states board
//! `B67XdS` and the `Chat` components `wtGg8`) that the UI kit's roles do not carry, registered as lurq
//! `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{RadiusSize, ShadowStyle, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::box_shadow::BoxShadow,
};

use super::{ExtraColor, typography::style};

/// Text of the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationText {
  /// 12/600 `text_primary`: who wrote an agent's message (`Name` `WSVQW` in `Agent message` `dcd0m`).
  Name,
  /// 11/500 `text_muted`: a step divider's label (`Label` `qoj6F` in `Step divider` `qfdli`).
  Divider,
  /// 11.5 `text_secondary`, 1.45: what a problem row explains (`Detail` `jsURR` in `Problem row` `IxIMv`).
  ProblemDetail,
  /// 12.5/600 `text_primary`: a result card's title (`Title` `KOwOt` in `Result card` `T0uKNU`).
  ResultTitle,
  /// 13.5/600 `text_primary`: what an empty, loading or failed chat says (`AGhHg`, `SmQgm`, `cqjZ2` in
  /// `B67XdS`).
  StateTitle,
}

impl ConversationText {
  pub const ALL: [Self; 5] = [
    Self::Name,
    Self::Divider,
    Self::ProblemDetail,
    Self::ResultTitle,
    Self::StateTitle,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Name => "conversation_name",
      Self::Divider => "conversation_divider",
      Self::ProblemDetail => "conversation_problem_detail",
      Self::ResultTitle => "conversation_result_title",
      Self::StateTitle => "conversation_state_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Name => style(12.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::Divider => style(11.0, FontWeight::Medium, 1.4, 0.0, palette.text_muted),
      Self::ProblemDetail => style(11.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::ResultTitle => style(12.5, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::StateTitle => style(13.5, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationSpacing {
  /// 20: an open tool group's rows, in from the chat's edge (`Tool rows` `G8Q14`).
  ToolRows,
  /// 29: a decision mark under its tool row, in to the row's label (`Mark row` `hVbqn`).
  Mark,
  /// 9: a problem row's top and bottom (`IxIMv`).
  ProblemRow,
  /// 9: the run state chip's sides (`Run state · Following live` `AQbG3`).
  StateChip,
}

impl ConversationSpacing {
  pub const ALL: [Self; 4] = [Self::ToolRows, Self::Mark, Self::ProblemRow, Self::StateChip];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ToolRows => "conversation_tool_rows",
      Self::Mark => "conversation_mark",
      Self::ProblemRow => "conversation_problem_row",
      Self::StateChip => "conversation_state_chip",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ToolRows => 20.0,
      Self::Mark => 29.0,
      Self::ProblemRow | Self::StateChip => 9.0,
    }
  }
}

/// Radii of the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationRadius {
  /// 8: a problem row (`IxIMv`).
  ProblemRow,
}

impl ConversationRadius {
  pub const ALL: [Self; 1] = [Self::ProblemRow];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ProblemRow => "conversation_problem_row",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ProblemRow => 8.0,
    }
  }
}

/// Shadows of the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationShadow {
  /// 0 4 16 of `shadow`: under *Jump to latest* (`tvvRD`).
  Jump,
}

impl ConversationShadow {
  pub fn role(self) -> ShadowStyle {
    ShadowStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Jump => "conversation_jump",
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ConversationText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn shadows() -> impl Iterator<Item = (Arc<str>, Arc<[BoxShadow]>)> {
  let jump = BoxShadow::new(0.0, 4.0, 16.0, ExtraColor::Shadow.role());
  std::iter::once((Arc::from(ConversationShadow::Jump.name()), Arc::from([jump])))
}

pub(super) fn install(theme: &Theme) {
  for spacing in ConversationSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
  for radius in ConversationRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
