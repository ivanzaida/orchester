//! The design values of Connections (`Connections · All connections · dark` `z4QquH` and the boards of the
//! layer `Connections` `mKKPM`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of Connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionsText {
  /// 13 `text_secondary`, 1.4: what the screen holds, under its title (`l1SiGn` in `SUpbd`).
  Lead,
  /// 11/500 `text_muted`: a column's heading over the connections (`CONNECTION` `Qn42W` in `Sznby`).
  ColumnHeading,
  /// 14/600 `text_primary`: the inspected connection's name (`keVXJ` in `Connection inspector header`
  /// `GOSxB`).
  InspectorTitle,
  /// 12.5/500 `text_primary`: what a confirmation in the inspector asks (`Turn off kontur-docs?` `qi5Cr`).
  ConfirmTitle,
}

impl ConnectionsText {
  pub const ALL: [Self; 4] = [
    Self::Lead,
    Self::ColumnHeading,
    Self::InspectorTitle,
    Self::ConfirmTitle,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Lead => "connections_lead",
      Self::ColumnHeading => "connections_column_heading",
      Self::InspectorTitle => "connections_inspector_title",
      Self::ConfirmTitle => "connections_confirm_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Lead => style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::ColumnHeading => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
      Self::InspectorTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::ConfirmTitle => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of Connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionsSpacing {
  /// 40: inside the content panel at its sides (`Content · Connections` `wRmOQ`).
  Page,
  /// 14: between the list, its notice and the note under them (`List column` `QNpoj`).
  List,
}

impl ConnectionsSpacing {
  pub const ALL: [Self; 2] = [Self::Page, Self::List];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "connections_page",
      Self::List => "connections_list",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::List => 14.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ConnectionsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in ConnectionsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
