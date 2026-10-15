//! Local integration's text roles follow `Dirty checkout notice` `t6PHFE`, `Prepare checkout` `hULqu` and
//! `Changed file row` `dAwTi` of section `zMOBO`, and the recovery record's values (`xBGxu`).
use super::*;

#[test]
fn local_integration_roles_follow_the_live_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line_box, family, weight) in [
    (theme::LocalText::Heading, 14.0, 21.0, "Geist", FontWeight::Normal),
    (theme::LocalText::Explanation, 11.5, 17.0, "Geist", FontWeight::Normal),
    (theme::LocalText::Note, 11.5, 17.0, "Geist", FontWeight::Normal),
    (theme::LocalText::Value, 11.5, 17.0, "Geist Mono", FontWeight::Normal),
    (theme::LocalText::Letter, 12.0, 16.0, "Geist Mono", FontWeight::Medium),
    (theme::LocalText::Path, 12.0, 16.0, "Geist Mono", FontWeight::Normal),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, weight, family),
      "{role:?}"
    );
    assert!(
      (style.font_size * style.line_height - line_box).abs() < 0.01,
      "{role:?}"
    );
  }
}
