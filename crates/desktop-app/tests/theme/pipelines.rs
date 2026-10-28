//! The redesigned Pipelines' type roles and spacing, and grouped rows' (`Pipelines · redesign` `evQ4U`).
use orchester_app::theme::{GroupText, PipelinesSpacing, PipelinesText};

use super::*;

#[test]
fn theme_pipelines_and_grouped_rows_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, family) in [
    // A group's title (`m9gK0`), a row's hint (`XkuLQ`), a path as a row's label (`xKL70`).
    (GroupText::Title.role(), 12.0, FontWeight::SemiBold, "Geist"),
    (GroupText::Hint.role(), 12.0, FontWeight::Normal, "Geist"),
    (GroupText::MonoLabel.role(), 13.0, FontWeight::Medium, "Geist Mono"),
    // A row's value (`VI3kR`), a slug or a model (`cNIa0`), *Changed* (`ci2Mr`).
    (GroupText::Value.role(), 12.5, FontWeight::Normal, "Geist"),
    (GroupText::MonoValue.role(), 12.5, FontWeight::Normal, "Geist Mono"),
    (GroupText::Tag.role(), 10.5, FontWeight::Medium, "Geist"),
    // *ON TASK START* (`zPT6x`), a card's number (`V1JR5`), the chosen step's (`o06df`), the version line (`J4YQR`).
    (PipelinesText::FlowLabel.role(), 10.5, FontWeight::Normal, "Geist Mono"),
    (PipelinesText::CardNumber.role(), 11.5, FontWeight::Normal, "Geist Mono"),
    (PipelinesText::StepNumber.role(), 13.0, FontWeight::Normal, "Geist Mono"),
    (PipelinesText::Status.role(), 12.0, FontWeight::Medium, "Geist"),
  ] {
    let style = theme.typography_style(role);
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family),
      (size, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(
    theme.typography_style(GroupText::Hint.role()).color,
    theme.palette_color(PaletteColor::TextMuted)
  );
  assert_eq!(
    theme.typography_style(GroupText::Tag.role()).color,
    theme.palette_color(ExtraColor::AccentText.role())
  );
  for (spacing, value) in [(PipelinesSpacing::PageTop, 20.0), (PipelinesSpacing::EdgeIndent, 21.0)] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
