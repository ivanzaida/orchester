//! The side question pane's own type roles and spacing (`q4Nz4i` in 12b `gC8a7` of `Task page · redesign`).
use orchester_app::theme::{SideSpacing, SideText};

use super::*;

#[test]
fn side_question_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family) in [
    // *Side question* (`g857c`), *KON-12 · run 9* (`Dwxw5`), the scope line (`MtduG`), the question and the answer
    // (`Vc0ev`, `B6mJAw`), *Asked 15:26 · only you see it* (`F1n7O6`), *Side question* over the answer (`nIJns`).
    (SideText::Title, 13.0, 1.3, FontWeight::SemiBold, "Geist"),
    (SideText::Quiet, 11.5, 1.3, FontWeight::Normal, "Geist"),
    (SideText::Scope, 12.0, 1.45, FontWeight::Normal, "Geist"),
    (SideText::Body, 13.0, 1.55, FontWeight::Normal, "Geist"),
    (SideText::Meta, 11.0, 1.3, FontWeight::Normal, "Geist"),
    (SideText::Who, 12.0, 1.3, FontWeight::Medium, "Geist"),
    // A source's path (`V9y1sX`), a message's author (`z6OHs`), what it is (`pbIsM`), *Open ↗* (`r9mNx`).
    (SideText::SourceMono, 12.0, 1.3, FontWeight::Normal, "Geist Mono"),
    (SideText::Source, 12.0, 1.3, FontWeight::Normal, "Geist"),
    (SideText::Detail, 12.0, 1.3, FontWeight::Normal, "Geist"),
    (SideText::Open, 12.0, 1.3, FontWeight::Normal, "Geist"),
    (SideText::Failed, 12.0, 1.45, FontWeight::Normal, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
  }
  let palette = |color| theme.palette_color(color);
  assert_eq!(
    theme.typography_style(SideText::Scope.role()).color,
    palette(PaletteColor::TextSecondary)
  );
  assert_eq!(
    theme.typography_style(SideText::Failed.role()).color,
    palette(PaletteColor::Danger)
  );
  for (spacing, value) in [(SideSpacing::Gap, 12.0), (SideSpacing::Edge, 4.0)] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
