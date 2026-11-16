//! The Agent picker's type roles and scrollbar (`Agent picker` `Zn8BM` in K4 `CwLfL`; the field `rMu3f` in P10
//! `HjgZ8`).

use lurq::app::theme::PaletteColor;
use orchester_app::theme::{AgentPickerText, agent_picker_scrollbar};

use super::*;

#[test]
fn agent_picker_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line_height, weight, color) in [
    // *Agent profiles* (`M27vF`), a name (`uLVIH`), what it runs on (`NOSNq`), the foot (`tP9tR`).
    (
      AgentPickerText::Group,
      11.0,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextMuted,
    ),
    (
      AgentPickerText::Name,
      12.0,
      1.2,
      FontWeight::Medium,
      PaletteColor::TextPrimary,
    ),
    (
      AgentPickerText::Runs,
      11.5,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextMuted,
    ),
    (
      AgentPickerText::Foot,
      12.0,
      1.2,
      FontWeight::Normal,
      PaletteColor::TextSecondary,
    ),
    // The field's name (`GyeX6`) and what it runs on (`X7UJ9`).
    (
      AgentPickerText::FieldName,
      12.5,
      1.3,
      FontWeight::Medium,
      PaletteColor::TextPrimary,
    ),
    (
      AgentPickerText::FieldRuns,
      12.0,
      1.3,
      FontWeight::Normal,
      PaletteColor::TextMuted,
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
      (size, line_height, weight, theme.palette_color(color), "Geist"),
      "{role:?}"
    );
  }
}

/// The list's scrollbar is 3 wide, radius 2, 4 from the list's side and 6 from its top and bottom (`f8jPq`).
#[test]
fn agent_picker_scrollbar_follows_the_design() {
  let app = themed(Mode::Dark);
  let bar = agent_picker_scrollbar(&app.theme().scrollbar());
  assert_eq!((bar.width, bar.thumb_radius), (3.0, 2.0));
  assert_eq!((bar.edge_inset, bar.end_inset), (Some(4.0), Some(6.0)));
}
