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
    // The head's kind, a hint and a section's label (`Where 0` `O1TUu`, `Time` `TAV5u`, `Label` `k4ivft`), the title
    // (`Title` `x5AOBx`).
    (HubText::PanelWhere, 12.0, 1.2, FontWeight::Normal, "Geist"),
    (HubText::PanelHint, 12.0, 1.2, FontWeight::Normal, "Geist"),
    (HubText::PanelLabel, 12.0, 1.2, FontWeight::Medium, "Geist"),
    (HubText::PanelTitle, 16.0, 1.3, FontWeight::SemiBold, "Geist"),
    // What a banner of an edit says first (`Title` `n1Nv7` in A14, `w6McZ` in A16, `rHijY` in A17).
    (HubText::NoticeTitle, 12.5, 1.4, FontWeight::SemiBold, "Geist"),
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
  for (role, color) in [
    (HubText::PanelWhere, PaletteColor::TextSecondary),
    (HubText::PanelHint, PaletteColor::TextMuted),
    (HubText::PanelLabel, PaletteColor::TextMuted),
    (HubText::PanelTitle, PaletteColor::TextPrimary),
    (HubText::NoticeTitle, PaletteColor::TextPrimary),
  ] {
    assert_eq!(theme.typography_style(role.role()).color, palette(color), "{role:?}");
  }
}
