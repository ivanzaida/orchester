//! Local integration's text roles follow `Dirty checkout notice` `t6PHFE`, `Prepare checkout` `hULqu` and
//! `Changed file row` `dAwTi` of section `zMOBO`.
use super::*;

#[test]
fn local_integration_roles_follow_the_live_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line_box, family) in [
    (theme::LocalText::Heading, 14.0, 21.0, "Geist"),
    (theme::LocalText::Explanation, 11.5, 17.0, "Geist"),
    (theme::LocalText::Note, 11.5, 17.0, "Geist"),
    (theme::LocalText::Path, 12.0, 16.0, "Geist Mono"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, FontWeight::Normal, family),
      "{role:?}"
    );
    assert!(
      (style.font_size * style.line_height - line_box).abs() < 0.01,
      "{role:?}"
    );
  }
}
