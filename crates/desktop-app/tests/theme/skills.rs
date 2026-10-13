//! Import typography and page rhythm from TmfIL/OLgEc and F8mJNd/NKAjl.
use super::*;

#[test]
fn skills_import_roles_follow_the_live_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, family) in [
    (theme::skills::import_caption(), 10.5, 1.3, "Geist Mono"),
    (theme::skills::import_path(), 12.0, 1.3, "Geist Mono"),
    (theme::skills::library_lead(), 13.0, 1.45, "Geist"),
  ] {
    let style = theme.typography_style(role);
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, FontWeight::Normal, family)
    );
  }
}
