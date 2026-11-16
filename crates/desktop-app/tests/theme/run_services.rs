//! A task's Services tab's own type roles (`06 · Task · Services` `cjvRr`).
use orchester_app::theme::RunServicesText;

use super::*;

#[test]
fn run_services_roles_follow_the_services_tab() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, (size, weight), family, color) in [
    // A row's name, port and uptime (`an59e`, `bBPsV`, `pPX2f` in `qWnt1`).
    (
      RunServicesText::Name,
      (12.5, FontWeight::Medium),
      "Geist Mono",
      PaletteColor::TextPrimary,
    ),
    (
      RunServicesText::Port,
      (12.0, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
    (
      RunServicesText::Uptime,
      (12.0, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::TextMuted,
    ),
    // The live log's title, a line's time and its words (`ftDRR`, `TPtll`, `FCQe1` in `iw8mT`).
    (
      RunServicesText::LogTitle,
      (12.0, FontWeight::Medium),
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      RunServicesText::LogTime,
      (11.5, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::TextMuted,
    ),
    (
      RunServicesText::LogLine,
      (11.5, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (size, weight, family, theme.palette_color(color)),
      "{role:?}"
    );
  }
}
