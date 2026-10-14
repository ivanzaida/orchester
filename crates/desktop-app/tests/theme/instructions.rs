//! U69ob: source lines start at their line boxes even with blank source lines.
use lurq::{
  app::{App, theme::PaletteColor},
  layout::text_style::{FontWeight, VerticalAlign},
};
use orchester_app::theme::{self, Mode, instructions};

#[test]
fn instructions_source_typography_preserves_source_line_positions() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let style = app.theme().typography_style(instructions::source_text());
  assert_eq!(
    (style.font_size, style.weight, &*style.font_family),
    (12.0, FontWeight::Normal, "Geist Mono")
  );
  assert!(style.vertical_align == VerticalAlign::LineBox);
  assert_eq!(style.color, app.theme().palette_color(PaletteColor::TextPrimary));
}

#[test]
fn instructions_detail_rhythm_matches_x5we6i() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let name = app.theme().typography_style(instructions::variable_name());
  let value = app.theme().typography_style(instructions::detail_value());
  let description = app.theme().typography_style(instructions::detail_description());
  assert_eq!(name.font_size, 12.0);
  assert!((name.line_height - 4.0 / 3.0).abs() < f32::EPSILON);
  assert_eq!(value.line_height, 1.45);
  assert_eq!(description.line_height, 1.45);
  assert_eq!(
    app.theme().spacing_value(instructions::prompt_top()),
    lurq::node::dimension::Dimension::Px(16.0)
  );
}
