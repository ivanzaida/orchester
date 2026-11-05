//! The Services pages' own roles (`02 · Service editor` `TGRTt`, `03 · Stack editor` `iNxTM`).
use orchester_app::theme::{ServicesSpacing, ServicesText};

use super::*;

#[test]
fn services_roles_follow_the_service_editor() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, color) in [
    // A chain chip's place and service (`UbGRP`, `zd2y5` in `R3grk`).
    (
      ServicesText::ChainNumber,
      11.0,
      FontWeight::Normal,
      PaletteColor::TextMuted,
    ),
    (
      ServicesText::ChainName,
      12.0,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    // A start order row's place in its mark and its service (`tSqix`, `q3WMp` in `Sow5a`).
    (
      ServicesText::OrderNumber,
      10.0,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    (
      ServicesText::OrderName,
      12.0,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    // A UI driver card's name (`Uk9us` in `AHbLE`).
    (
      ServicesText::DriverName,
      12.5,
      FontWeight::Medium,
      PaletteColor::TextSecondary,
    ),
    // A placeholder of *Values in a run* and a tool of *Agents use it with* (`UMC4g`, `xCTPs`).
    (
      ServicesText::SideLine,
      11.5,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    // A stack's service's step in its round mark (`L5UCWh` in `iNxTM`).
    (
      ServicesText::StepNumber,
      10.5,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (size, weight, "Geist Mono", theme.palette_color(color)),
      "{role:?}"
    );
  }
}

/// The tree of needs steps 18 in per level (`ePZt6`); the slots' bar keeps 4 between its slots (`QXz4w`).
#[test]
fn services_spacing_follows_the_stack_editor() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, value) in [(ServicesSpacing::TreeIndent, 18.0), (ServicesSpacing::SlotGap, 4.0)] {
    assert_eq!(theme.spacing_value(role.role()), Dimension::Px(value), "{role:?}");
  }
}
