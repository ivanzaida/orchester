//! The service cards' own type roles in a run's Conversation (`tz1d8` in `FwHrv`, `TienU` in `hvQDS`, `oXVuM`).
use orchester_app::theme::ServiceCardsText;

use super::*;

#[test]
fn service_cards_roles_follow_the_cards() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, (size, weight, height), family, color) in [
    // What happened and what Orchester did (`JNqgZ`, `jb1yu`).
    (
      ServiceCardsText::Body,
      (12.5, FontWeight::Normal, 1.5),
      "Geist",
      PaletteColor::TextSecondary,
    ),
    // A service's program and arguments (`GZxEp`).
    (
      ServiceCardsText::Command,
      (12.0, FontWeight::Normal, 1.3),
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
    // Whether a start is approved, the slots line, the macOS line and what a slots holder runs (`jBOKa`, `XR7xh`,
    // `s3ehm`, `VXOA1`).
    (
      ServiceCardsText::Detail,
      (12.0, FontWeight::Normal, 1.3),
      "Geist",
      PaletteColor::TextMuted,
    ),
    // A slots holder's task key (`Otz6B`).
    (
      ServiceCardsText::HolderKey,
      (12.0, FontWeight::Normal, 1.3),
      "Geist Mono",
      PaletteColor::TextPrimary,
    ),
    // *Open ↗* at a holder's end (`psrsV`).
    (
      ServiceCardsText::Open,
      (12.0, FontWeight::Medium, 1.3),
      "Geist",
      PaletteColor::TextSecondary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.weight,
        style.line_height,
        &*style.font_family,
        style.color
      ),
      (size, weight, height, family, theme.palette_color(color)),
      "{role:?}"
    );
  }
}
