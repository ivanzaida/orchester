//! Values of a team task's state on its page (`Task - v2` `jDN0r` rows 14–17: the rail's `State` `VjLQq` in T11a
//! `ujJ6U`).
use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamStateText {
  /// 15/600 `text_primary`, 1.35: the state's title (`State title` `POt4L`).
  Title,
  /// 12 `text_secondary`, 1.5: what the state means (`State detail` `MDVE2`).
  Detail,
  /// 12 `text_primary`, 1.5: the blocked reason, read as the person wrote it.
  Reason,
}

impl TeamStateText {
  pub const ALL: [Self; 3] = [Self::Title, Self::Detail, Self::Reason];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "team_state_title",
      Self::Detail => "team_state_detail",
      Self::Reason => "team_state_reason",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(15.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::Detail => style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
      Self::Reason => style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  TeamStateText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
