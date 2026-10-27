//! The design values of the date picker (`14 · Date picker` `QF4a2` in `Modules · redesign` `oEELJ` of the new
//! `orchester.pen`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// A day the range rules out, faded with its words (`Oct 1 · unavailable` `DOdZp`).
pub const UNAVAILABLE_DAY_OPACITY: f32 = 0.45;

/// Text of the date picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatePickerText {
  /// 13/600 `text_primary`: the month on show (`October 2026` `a0NvZy`).
  Month,
  /// 12 `text_primary`: a day of the grid (`Day` `n8tnk`); a day of the months around in `text_muted`.
  Day,
  /// 12/600: the chosen day, on `accent` in `on_accent` (`F2bKq`), and today (`SzjgV`).
  DayMarked,
}

impl DatePickerText {
  pub const ALL: [Self; 3] = [Self::Month, Self::Day, Self::DayMarked];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Month => "date_picker_month",
      Self::Day => "date_picker_day",
      Self::DayMarked => "date_picker_day_marked",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Month => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Day => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::DayMarked => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  DatePickerText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
