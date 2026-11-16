//! The type roles of the steps drawn as the flow they run in (`Steps` `uSoll` on `Pipelines · v2` P1 `s7LMyQ`).

use lurq::app::theme::PaletteColor;
use orchester_app::theme::FlowText;

use super::*;

#[test]
fn flow_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line_height, weight, color, family) in [
    // *ON TASK START* (`r7IIY5`), what it says after it (`RexHV`), *Waits for your approval* (`OptVp`).
    (
      FlowText::Label,
      10.5,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist Mono",
    ),
    (
      FlowText::Hint,
      12.0,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      FlowText::Gate,
      12.0,
      1.2,
      FontWeight::Medium,
      PaletteColor::Warning,
      "Geist",
    ),
    // A node's number (`aRdE6`), a card's number in a parallel group (`XLjJT`), its name (`Jg3Pb`) and its line
    // (`I9Z0s2`).
    (
      FlowText::Number,
      10.5,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
      "Geist Mono",
    ),
    (
      FlowText::CardNumber,
      11.5,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist Mono",
    ),
    (
      FlowText::Name,
      12.5,
      1.3,
      FontWeight::Medium,
      PaletteColor::TextPrimary,
      "Geist",
    ),
    (
      FlowText::Line,
      12.0,
      1.3,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
      "Geist",
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.line_height,
        style.weight,
        style.color,
        &*style.font_family
      ),
      (size, line_height, weight, theme.palette_color(color), family),
      "{role:?}"
    );
  }
}

/// What a dragged card's drop line says first: 12/500 `text` (`XHcfs` in `Drop here` `y0SHa`, P3).
#[test]
fn the_landing_line_is_12_medium_text() {
  let app = themed(Mode::Dark);
  let style = app.theme().typography_style(FlowText::Landing.role());
  assert_eq!(
    (style.font_size, style.line_height, style.weight, style.color),
    (
      12.0,
      1.2,
      FontWeight::Medium,
      app.theme().palette_color(PaletteColor::TextPrimary)
    )
  );
}

/// The label is spaced 0.6 (`r7IIY5`, `UVGsy`, `W5F8Hk`).
#[test]
fn the_flow_label_is_spaced() {
  let app = themed(Mode::Dark);
  assert_eq!(app.theme().typography_style(FlowText::Label.role()).letter_spacing, 0.6);
}
