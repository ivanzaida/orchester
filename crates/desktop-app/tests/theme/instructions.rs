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
