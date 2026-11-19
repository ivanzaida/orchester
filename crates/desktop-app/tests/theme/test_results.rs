//! The Tests card's own type roles (`07 · Summary · Tests` `qN2FK`).
use orchester_app::theme::TestsText;

use super::*;

#[test]
fn test_results_roles_follow_the_tests_card() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, (size, weight), family, color) in [
    // A layer's and a failed test's name, a flaky test's name, a file:line (`yHyXs`, `NC2ol`, `desMD`, `fV869`).
    (
      TestsText::Name,
      (12.5, FontWeight::Medium),
      "Geist Mono",
      PaletteColor::Foreground,
    ),
    (
      TestsText::FlakyName,
      (12.5, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::Foreground,
    ),
    (
      TestsText::Detail,
      (12.0, FontWeight::Normal),
      "Geist Mono",
      PaletteColor::MutedForeground,
    ),
    // A message's line (`VolEf`) and the headings over the card's rows (`XjDnk`).
    (
      TestsText::Message,
      (11.5, FontWeight::Normal),
      "Geist Mono",
      ExtraColor::TextSecondary.role(),
    ),
    (
      TestsText::Heading,
      (12.0, FontWeight::SemiBold),
      "Geist",
      ExtraColor::TextSecondary.role(),
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (size, weight, family, theme.palette_color(color)),
      "{role:?}"
    );
  }
}
