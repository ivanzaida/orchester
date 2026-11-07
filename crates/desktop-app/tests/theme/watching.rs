//! The `/watching` answer's own type roles and spacing (ORC-775, `SuoBc` in 06 `Ff7AD` of `Orchestrator watch ·
//! redesign`).
use orchester_app::theme::{WatchingSpacing, WatchingText};

use super::*;

#[test]
fn watching_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, family, color) in [
    // The total (`E1LN7W`), a subscription's name (`DY5UD`), its meta and a task's note (`dB4Vx`, `M4Ftl`), a task's
    // key and run (`n9WR4`), its title (`ugPXE`), *Stop watching* and *Decide ↗* (`Q8wFp4`, `MJxAI`).
    (
      WatchingText::Answer,
      13.0,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      WatchingText::Name,
      12.5,
      FontWeight::Medium,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      WatchingText::Meta,
      11.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextMuted,
    ),
    (
      WatchingText::Key,
      11.5,
      FontWeight::Normal,
      "Geist Mono",
      PaletteColor::TextSecondary,
    ),
    (
      WatchingText::Title,
      12.5,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextPrimary,
    ),
    (
      WatchingText::Action,
      12.0,
      FontWeight::Normal,
      "Geist",
      PaletteColor::TextSecondary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (size, weight, family, theme.palette_color(color)),
      "{role:?}"
    );
  }
  for (spacing, value) in [(WatchingSpacing::Side, 12.0), (WatchingSpacing::Indent, 34.0)] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
