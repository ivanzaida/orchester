//! Skills-specific Pencil roles.
use lurq::{app::App, node::dimension::Dimension};
use orchester_app::theme::{self, Mode};
#[test]
fn skills_theme_form_inset_matches_xf9b8() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  assert_eq!(
    app.theme().spacing_value(theme::skills::source_line()),
    Dimension::Px(20.0)
  );
  assert_eq!(app.theme().typography_style(theme::skills::source()).font_size, 12.0);
  assert_eq!(app.theme().typography_style(theme::skills::gutter()).font_size, 11.0);
  assert_eq!(
    app.theme().spacing_value(theme::skills::form_inset()),
    Dimension::Px(28.0)
  );
  assert_eq!(
    app.theme().spacing_value(theme::skills::cell_spacing()),
    Dimension::Px(12.0)
  );
  assert_eq!(app.theme().typography_style(theme::skills::name()).font_size, 13.0);
  assert_eq!(app.theme().typography_style(theme::skills::heading()).font_size, 11.0);
}
