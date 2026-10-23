//! Import resources' own type roles and spacing (`Dzzks`).
use orchester_app::theme::{ImportSpacing, ImportText};

use super::*;

#[test]
fn import_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, family) in [
    // The current step's number on its dot, a step to come (`k5ymI`, `rKOXf`).
    (ImportText::StepCurrent, 10.5, FontWeight::SemiBold, "Geist"),
    (ImportText::StepNext, 10.5, FontWeight::Normal, "Geist"),
    // The destination's initial (`gtFlr`), a group's eyebrow (`KRlEH`), a secret's name (`oOtgw` in `ItSwZ`).
    (ImportText::Initial, 10.0, FontWeight::SemiBold, "Geist"),
    (ImportText::Eyebrow, 10.5, FontWeight::Normal, "Geist Mono"),
    (ImportText::SecretName, 12.5, FontWeight::Medium, "Geist Mono"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(theme.typography_style(ImportText::Eyebrow.role()).letter_spacing, 0.6);
  assert_eq!(
    theme.typography_style(ImportText::StepCurrent.role()).color,
    theme.palette_color(PaletteColor::TextInverse)
  );
  for (spacing, value) in [
    (ImportSpacing::Ready, 18.0),
    (ImportSpacing::Sections, 20.0),
    (ImportSpacing::CheckColumn, 36.0),
    (ImportSpacing::MissingRow, 9.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
