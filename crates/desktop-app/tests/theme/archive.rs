//! The archive confirmation's type roles (`A23` `Pnpbe`, ORC-838).
use orchester_app::theme::ArchiveText;

use super::*;

#[test]
fn archive_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family, color) in [
    (
      ArchiveText::Title,
      14.0,
      1.35,
      FontWeight::SemiBold,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      ArchiveText::Body,
      12.5,
      1.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      ArchiveText::Heading,
      11.5,
      1.45,
      FontWeight::SemiBold,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      ArchiveText::Name,
      12.5,
      1.45,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      ArchiveText::Holder,
      12.5,
      1.45,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
    (
      ArchiveText::Version,
      12.0,
      1.45,
      FontWeight::Normal,
      "Geist Mono",
      PaletteColor::TextMuted,
    ),
    (
      ArchiveText::Note,
      12.0,
      1.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextMuted,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
    assert_eq!(style.color, theme.palette_color(color), "{role:?}");
  }
}
