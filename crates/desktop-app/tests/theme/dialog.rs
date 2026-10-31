//! A confirmation modal's roles (`Dialog` `LVCxf` in `04d · Bind an MCP server` `z0iFy`, `Shared configuration ·
//! redesign` `UC5xk`).
use orchester_app::theme::{DIALOG_TILE, DIALOG_WIDTH, DialogSpacing, DialogText};

use super::*;

#[test]
fn dialog_roles_follow_the_shared_configuration_modals() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight) in [
    // The question (`gqIuU`), the choice's label (`y2I3c`), a line (`RnZTC`), *Esc cancels* (`lESYn`).
    (DialogText::Title, 15.0, FontWeight::SemiBold),
    (DialogText::ChoiceLabel, 11.0, FontWeight::Medium),
    (DialogText::Line, 12.5, FontWeight::Normal),
    (DialogText::Keys, 11.5, FontWeight::Normal),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!((style.font_size, style.weight), (size, weight), "{role:?}");
  }
  assert_eq!(
    theme.spacing_value(DialogSpacing::Side.role()),
    Dimension::Px(20.0),
    "BuD1B, APcrC"
  );
  assert_eq!(
    theme.spacing_value(DialogSpacing::Parts.role()),
    Dimension::Px(14.0),
    "BuD1B"
  );
  assert!((DIALOG_WIDTH - 500.0).abs() < f32::EPSILON, "LVCxf");
  assert!((DIALOG_TILE - 32.0).abs() < f32::EPSILON, "jdSYt");
}
