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

/// WMT1W's found sources: header title `Lqx9K`, count `NYVFc`, footer explanation `NaIBV`.
#[test]
fn skills_found_roles_follow_the_live_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let palette = theme.palette();
  for (role, size, line, weight, color) in [
    (
      theme::skills::found_title(),
      12.5,
      1.3,
      FontWeight::SemiBold,
      palette.text_primary,
    ),
    (
      theme::skills::found_count(),
      12.0,
      1.3,
      FontWeight::Normal,
      palette.text_muted,
    ),
    (
      theme::skills::found_note(),
      11.5,
      1.45,
      FontWeight::Normal,
      palette.text_muted,
    ),
  ] {
    let style = theme.typography_style(role);
    assert_eq!(
      (
        style.font_size,
        style.line_height,
        style.weight,
        &*style.font_family,
        style.color
      ),
      (size, line, weight, "Geist", color)
    );
  }
}
