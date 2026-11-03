//! A run's pull request CI's own type role (ORC-335, `J0SbA` in 10b `mxt1S` of `Task page · redesign`).
use orchester_app::theme::CiText;

use super::*;

#[test]
fn ci_log_tail_follows_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  // A failing check's log tail on `code_bg`: Geist Mono 11.5 in `text_secondary`.
  let style = theme.typography_style(CiText::LogTail.role());
  assert_eq!(
    (style.font_size, style.line_height, style.weight, &*style.font_family),
    (11.5, 1.6, FontWeight::Normal, "Geist Mono")
  );
  assert_eq!(style.color, theme.palette_color(PaletteColor::TextSecondary));
}
