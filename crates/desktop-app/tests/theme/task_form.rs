//! The task form composer's own type roles and spacing (`Task form · redesign` `HFXxX` in the new `orchester.pen`).
use orchester_app::theme::{TaskFormSpacing, TaskFormText, sources_scrollbar};

use super::*;

#[test]
fn task_form_roles_follow_the_composer() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, line_height, color) in [
    // The title and the outcome typed on the page (`klxTy`, `KNpPG`), a criterion (`cScHP`).
    (
      TaskFormText::Title,
      24.0,
      FontWeight::SemiBold,
      1.3,
      PaletteColor::TextPrimary,
    ),
    (
      TaskFormText::Outcome,
      14.0,
      FontWeight::Normal,
      1.6,
      PaletteColor::TextSecondary,
    ),
    (
      TaskFormText::Criterion,
      13.0,
      FontWeight::Normal,
      1.3,
      PaletteColor::TextPrimary,
    ),
    // The criteria head's count and hint (`KrVl9`, `ZndIB`), a pipeline's steps in its menu (`H1NsU`).
    (
      TaskFormText::CriteriaHint,
      12.0,
      FontWeight::Normal,
      1.3,
      PaletteColor::TextMuted,
    ),
    (
      TaskFormText::OptionMeta,
      11.5,
      FontWeight::Normal,
      1.3,
      PaletteColor::TextMuted,
    ),
    // A branch's name in the Base branch menu (`f9wHj` in `fSsas`), in Geist Mono.
    (
      TaskFormText::OptionMono,
      12.5,
      FontWeight::Medium,
      1.3,
      PaletteColor::TextPrimary,
    ),
    // The words of a note under the head, one 16 line in its 32 (`vpy3p` in `CvMGI`).
    (
      TaskFormText::Note,
      12.0,
      FontWeight::Normal,
      1.3,
      PaletteColor::TextSecondary,
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height, style.color),
      (size, weight, line_height, theme.palette_color(color)),
      "{role:?}"
    );
  }
  assert_eq!(theme.typography_style(TaskFormText::Title.role()).letter_spacing, -0.4);
  for (spacing, value) in [
    (TaskFormSpacing::Side, 28.0),
    (TaskFormSpacing::Composer, 20.0),
    (TaskFormSpacing::MenuOption, 7.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
  // The Sources popover's 4 wide scrollbar runs 4 in from its end, inside its 12, clear of the rows.
  let scrollbar = sources_scrollbar(&theme.scrollbar());
  assert_eq!((scrollbar.width, scrollbar.resolved_edge_inset()), (4.0, 4.0));
}
