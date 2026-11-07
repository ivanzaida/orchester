//! The Agents hub's own type roles of the profile panel (`Agents · Profile` `NDZXC`, ORC-832).
use orchester_app::theme::HubText;

use super::*;

#[test]
fn hub_panel_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family) in [
    // What a row's label says under it (`Sub` `aOsQu`), a model (`Value` `Jw1FA`), *Set path →* (`Link` `rfGmw`).
    (HubText::PanelSub, 11.5, 1.3, FontWeight::Normal, "Geist"),
    (HubText::PanelMono, 12.5, 1.3, FontWeight::Normal, "Geist Mono"),
    (HubText::PanelLink, 12.0, 1.2, FontWeight::Medium, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
  }
  let palette = |color| theme.palette_color(color);
  assert_eq!(
    theme.typography_style(HubText::PanelSub.role()).color,
    palette(PaletteColor::TextMuted)
  );
  assert_eq!(
    theme.typography_style(HubText::PanelLink.role()).color,
    palette(PaletteColor::TextPrimary)
  );
}
