//! The date picker's calendar (`ui::date_picker`, `14 · Date picker` `QF4a2` in `Modules · redesign` `oEELJ`): lurq's
//! `ui::Calendar`. 260 wide on `surface_raised` inside a `border_strong` stroke, radius 10, 10 inside, its parts 6
//! apart, lifted by the `lg` shadow: the month in 13/600 at the start of a 28 high heading, 4 in, with ‹ and › at its
//! end (26 square, radius 6, a 15 `text_secondary` glyph, the `hover` fill under the pointer), 2 apart; the weekdays in
//! 11 `text_muted`, 22 high; six weeks of days 2 apart, each 30 high with radius 6 and as wide as the panel leaves a
//! seventh of it. The chosen day is filled `accent` in `on_accent` 12/600, today outlined in `border_strong` in
//! 12/600, the other end of a range tinted `accent_muted`; the months around are `text_muted`, and days the range rules
//! out are faded and take no press. The day the keys are on is ringed once the keys moved it (state [`KEYED`]; the
//! design draws no ring). Today and Clear are under a `border` line, 4 under it. The recipe is the app's whole.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border, color::Color},
  ui::{Calendar, CalendarPart, CalendarState, Recipe, Selector},
};

use crate::theme::{
  ControlHeight, DatePickerText, ExtraColor, ExtraShadow, ExtraSpacing, IconSize, UNAVAILABLE_DAY_OPACITY,
};

/// The keys moved the day they are on: it is ringed.
pub const KEYED: &str = "keyed";

/// The panel's width (`Target date picker` `INGIj`).
pub const PANEL_WIDTH: f32 = 260.0;
/// The heading's height (`Month` `uii7i`), a weekday's (`Mo` `ZOcUr`) and a day's (`Week 1` `buHTX`).
const HEADING_HEIGHT: f32 = 28.0;
const WEEKDAY_HEIGHT: f32 = 22.0;
const DAY_HEIGHT: f32 = 30.0;

pub(super) fn install(theme: &Theme) {
  let clear = Color::new(0, 0, 0, 0);
  theme.set_recipe::<Calendar>(
    frame(Recipe::new())
      .base(CalendarPart::Day, |style| {
        style
          .width(day_width())
          .height(DAY_HEIGHT)
          .radius(RadiusSize::Md)
          .typography(DatePickerText::Day.role())
          .foreground(PaletteColor::Foreground)
          .cursor(CursorIcon::Pointer)
      })
      .hovered(CalendarPart::Day, |style| style.background(ExtraColor::Hover.role()))
      .state(CalendarState::Outside, CalendarPart::Day, |style| {
        style.foreground(PaletteColor::MutedForeground)
      })
      .state(CalendarState::Current, CalendarPart::Day, |style| {
        style
          .typography(DatePickerText::DayMarked.role())
          .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
      })
      .state(CalendarState::OtherEnd, CalendarPart::Day, |style| {
        style.background(PaletteColor::Highlight)
      })
      .state(CalendarState::Selected, CalendarPart::Day, |style| {
        style
          .background(PaletteColor::Primary)
          .foreground(ExtraColor::OnAccent.role())
          .typography(DatePickerText::DayMarked.role())
      })
      .when(
        Selector::new().state(CalendarState::Selected).hovered(),
        CalendarPart::Day,
        |style| style.background(PaletteColor::Primary),
      )
      .state(CalendarState::Unavailable, CalendarPart::Day, |style| {
        style.opacity(UNAVAILABLE_DAY_OPACITY).cursor(CursorIcon::Default)
      })
      .when(
        Selector::new().state(CalendarState::Unavailable).hovered(),
        CalendarPart::Day,
        |style| style.background(clear),
      )
      .when(
        Selector::new().state(KEYED).state(CalendarState::Cursor),
        CalendarPart::Day,
        |style| style.shadow(ExtraShadow::FocusRing.role()),
      ),
  );
}

/// The panel, its heading, buttons and weekdays, and the footer's line.
fn frame(recipe: Recipe) -> Recipe {
  recipe
    .base(CalendarPart::Root, |style| {
      style
        .width(PANEL_WIDTH)
        .padding_x(ExtraSpacing::Loose.role())
        .padding_y(ExtraSpacing::Loose.role())
        .gap(ExtraSpacing::Snug.role())
        .radius(RadiusSize::Lg)
        .background(PaletteColor::Card)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
        .shadow(ShadowStyle::Lg)
    })
    .base(CalendarPart::Header, |style| {
      style
        .height(HEADING_HEIGHT)
        .padding_start(SpacingSize::Xs)
        .gap(ExtraSpacing::Tight.role())
    })
    .base(CalendarPart::Title, |style| {
      style.typography(DatePickerText::Month.role())
    })
    .base(CalendarPart::Nav, |style| {
      style
        .square(ControlHeight::Inspector.value())
        .radius(RadiusSize::Md)
        .cursor(CursorIcon::Pointer)
    })
    .hovered(CalendarPart::Nav, |style| style.background(ExtraColor::Hover.role()))
    .base(CalendarPart::NavIcon, |style| {
      style
        .square(IconSize::Nav.size())
        .foreground(ExtraColor::TextSecondary.role())
    })
    .base(CalendarPart::Grid, |style| style.gap(ExtraSpacing::Tight.role()))
    // 6 under the weekdays, as between the panel's parts: the grid's own gap is the 2 between weeks.
    .base(CalendarPart::Weekdays, |style| style.padding_bottom(SpacingSize::Xs))
    .base(CalendarPart::Weekday, |style| {
      style
        .width(day_width())
        .height(WEEKDAY_HEIGHT)
        .typography(TypographyStyle::Caption)
    })
    .base(CalendarPart::Footer, |style| {
      style
        .padding_top(SpacingSize::Xs)
        .top_line(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
}

/// A seventh of the room inside the panel: the seven columns fill it.
fn day_width() -> f32 {
  (PANEL_WIDTH - 2.0 * ExtraSpacing::Loose.value()) / 7.0
}
