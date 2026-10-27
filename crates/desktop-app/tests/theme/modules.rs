//! The date picker's type roles and the Modules pages' spacing (`Modules · redesign` `oEELJ` in the new
//! `orchester.pen`: `14 · Date picker` `QF4a2`, `Properties` `ihSv0`).
use orchester_app::theme::{DatePickerText, ModulesSpacing, UNAVAILABLE_DAY_OPACITY};

use super::*;

#[test]
fn date_picker_and_modules_roles_follow_the_redesign() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight) in [
    // The month (`a0NvZy`), a day (`n8tnk`), the chosen day and today (`F2bKq`, `SzjgV`).
    (DatePickerText::Month, 13.0, FontWeight::SemiBold),
    (DatePickerText::Day, 12.0, FontWeight::Normal),
    (DatePickerText::DayMarked, 12.0, FontWeight::SemiBold),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!((style.font_size, style.weight), (size, weight), "{role:?}");
  }
  assert!(
    (UNAVAILABLE_DAY_OPACITY - 0.45).abs() < f32::EPSILON,
    "a ruled-out day is faded (DOdZp)"
  );
  assert_eq!(
    theme.spacing_value(ModulesSpacing::Properties.role()),
    Dimension::Px(18.0),
    "the properties sit 18 inside their hairline (ihSv0)"
  );
}
