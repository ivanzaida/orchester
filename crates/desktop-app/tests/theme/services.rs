//! A service's page's own type roles (`02 · Service editor` `TGRTt`).
use orchester_app::theme::ServicesText;

use super::*;

#[test]
fn services_roles_follow_the_service_editor() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, color) in [
    // A chain chip's place and service (`UbGRP`, `zd2y5` in `R3grk`).
    (ServicesText::ChainNumber, 11.0, PaletteColor::TextMuted),
    (ServicesText::ChainName, 12.0, PaletteColor::TextSecondary),
    // A start order row's place in its mark and its service (`tSqix`, `q3WMp` in `Sow5a`).
    (ServicesText::OrderNumber, 10.0, PaletteColor::TextSecondary),
    (ServicesText::OrderName, 12.0, PaletteColor::TextSecondary),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (size, FontWeight::Normal, "Geist Mono", theme.palette_color(color)),
      "{role:?}"
    );
  }
}
