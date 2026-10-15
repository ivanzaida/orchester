//! The Orchestrator's own type roles and spacing (`TrG83`, `Xir64`), and its replies' links (`agv8r`).
use super::*;

#[test]
fn orchestrator_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family) in [
    (OrchestratorText::PanelTitle, 13.0, 1.3, FontWeight::SemiBold, "Geist"),
    (OrchestratorText::Emphasis, 12.5, 1.35, FontWeight::Medium, "Geist"),
    (OrchestratorText::Eyebrow, 11.0, 1.3, FontWeight::Normal, "Geist Mono"),
    (OrchestratorText::Suggestion, 12.0, 1.4, FontWeight::Normal, "Geist"),
    (OrchestratorText::ContextLink, 11.0, 1.4, FontWeight::Medium, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(
    theme.typography_style(OrchestratorText::Eyebrow.role()).letter_spacing,
    0.6
  );
  for (spacing, value) in [
    (OrchestratorSpacing::Start, 20.0),
    (OrchestratorSpacing::SettingsPage, 40.0),
    (OrchestratorSpacing::HistoryDay, 18.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}

#[test]
fn orchestrator_link_roles_follow_the_link_states() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, family) in [
    (OrchestratorText::LinkHost, "Geist"),
    (OrchestratorText::LinkAddress, "Geist Mono"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (11.5, FontWeight::Normal, family, Color::from_hex("#8A8A92")),
      "{role:?} in `text_muted`"
    );
  }
  assert_eq!(theme.radius_value(OrchestratorRadius::Link.role()), 3.0);
  let ring = theme.shadow_style(OrchestratorShadow::LinkFocus.role());
  assert_eq!(
    ring
      .iter()
      .map(|shadow| (shadow.spread, shadow.inset, shadow.color.clone()))
      .collect::<Vec<_>>(),
    [(
      2.0,
      false,
      lurq::node::BackgroundColor::Palette(PaletteColor::BorderFocus)
    )],
    "2 of `border_focus` outside the link's edge"
  );
  let link = theme
    .markdown()
    .link
    .apply(&theme.typography_style(TypographyStyle::Body));
  assert_eq!(
    (link.weight, link.color),
    (FontWeight::Medium, Color::from_hex("#EDEDF0")),
    "a link's words in `accent_text` 500"
  );
  assert_eq!(
    (theme.palette().accent_hover, theme.palette().text_muted),
    (Color::from_hex("#FFFFFF"), Color::from_hex("#8A8A92")),
    "under the pointer `accent_hover`; the underline `text_muted`"
  );
}
