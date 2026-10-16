//! Settings › Orchester's own type roles and spacing (`Cu20b`, `DUWM5`, `zxe2P`, `Aowlk`).
use super::*;

#[test]
fn orchester_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, family) in [
    (OrchesterText::FormTitle, 18.0, FontWeight::SemiBold, "Geist"),
    (OrchesterText::PageTitle, 22.0, FontWeight::SemiBold, "Geist"),
    (OrchesterText::StepTitle, 14.0, FontWeight::SemiBold, "Geist"),
    (OrchesterText::StepNumber, 10.5, FontWeight::SemiBold, "Geist"),
    (OrchesterText::Code, 24.0, FontWeight::Medium, "Geist Mono"),
    (OrchesterText::InviteTitle, 13.0, FontWeight::Medium, "Geist"),
    (OrchesterText::Address, 11.5, FontWeight::Normal, "Geist Mono"),
    (OrchesterText::CardTitle, 12.5, FontWeight::SemiBold, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(theme.typography_style(OrchesterText::Code.role()).letter_spacing, 3.0);
  assert_eq!(
    theme.typography_style(OrchesterText::PageTitle.role()).letter_spacing,
    -0.3
  );
  for (spacing, value) in [
    (OrchesterSpacing::FormSide, 40.0),
    (OrchesterSpacing::FormTop, 22.0),
    (OrchesterSpacing::FormGap, 14.0),
    (OrchesterSpacing::PageSide, 96.0),
    (OrchesterSpacing::CodeSide, 18.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}

#[test]
fn gate_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, family, tracking) in [
    (GateText::Eyebrow, 11.0, FontWeight::Normal, "Geist Mono", 0.6),
    (GateText::Title, 26.0, FontWeight::SemiBold, "Geist", -0.5),
    (GateText::Lead, 13.5, FontWeight::Normal, "Geist", 0.0),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.letter_spacing),
      (size, weight, family, tracking),
      "{role:?}"
    );
  }
  assert_eq!(theme.typography_style(GateText::Lead.role()).line_height, 1.55);
  for (spacing, value) in [(GateSpacing::Column, 20.0), (GateSpacing::Side, 56.0)] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
