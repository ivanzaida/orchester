//! The Orchestrator's own type roles and spacing (`TrG83`, `Xir64`).
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
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
