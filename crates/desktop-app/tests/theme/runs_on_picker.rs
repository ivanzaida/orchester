//! The compact *Runs on* picker's type roles and scrollbar (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`).

use lurq::app::theme::PaletteColor;
use orchester_app::theme::{RUNS_ON_UNAVAILABLE_OPACITY, RunsOnPickerText, runs_on_picker_scrollbar};

use super::*;

#[test]
fn runs_on_picker_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, color, family) in [
    // The connection's name (`hyudy`), *Claude Code · Max plan* (`f2fcT`), *5h 41%* (`JLRnE`).
    (
      RunsOnPickerText::Name,
      12.0,
      FontWeight::Medium,
      PaletteColor::TextPrimary,
      "Geist",
    ),
    (
      RunsOnPickerText::Caption,
      11.5,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      RunsOnPickerText::Use,
      10.5,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist Mono",
    ),
    // An effort's segment (`k2S91`) and the chosen one (`LH6Q5`).
    (
      RunsOnPickerText::Segment,
      11.5,
      FontWeight::Normal,
      PaletteColor::TextMuted,
      "Geist",
    ),
    (
      RunsOnPickerText::SegmentChosen,
      11.5,
      FontWeight::Medium,
      PaletteColor::TextPrimary,
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
      (size, 1.2, weight, theme.palette_color(color), family),
      "{role:?}"
    );
  }
}

/// A connection that cannot be chosen is at 40 % (`w58WS`), and the model list's scrollbar is 3 wide, radius 2, 2 clear
/// of the list's end, top and bottom (`KbY6m`).
#[test]
fn runs_on_picker_unavailable_opacity_and_scrollbar_follow_the_design() {
  assert_eq!(RUNS_ON_UNAVAILABLE_OPACITY, 0.4);
  let app = themed(Mode::Dark);
  let bar = runs_on_picker_scrollbar(&app.theme().scrollbar());
  assert_eq!((bar.width, bar.thumb_radius), (3.0, 2.0));
  assert_eq!((bar.edge_inset, bar.end_inset), (Some(2.0), Some(2.0)));
}
