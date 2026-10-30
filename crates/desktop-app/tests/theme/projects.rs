//! All projects' own type roles of the returning start (09 `i5G19A`).
use orchester_app::theme::ProjectsText;

use super::*;

#[test]
fn projects_roles_follow_the_returning_start() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  // The team project offered above the list (`G1tpa` in `ZyS3T`): 12.5 on 18 `text_secondary`.
  let offer = theme.typography_style(ProjectsText::Offer.role());
  assert_eq!(
    (offer.font_size, offer.line_height, offer.weight, &*offer.font_family),
    (12.5, 1.45, FontWeight::Normal, "Geist")
  );
  assert_eq!(offer.color, theme.palette_color(PaletteColor::TextSecondary));
  // The folders searched for an agent CLI (`qMnzd` in `U4QvYO`): mono 11.5 on 1.3 `text_muted`.
  let searched = theme.typography_style(ProjectsText::SearchedFolders.role());
  assert_eq!(
    (
      searched.font_size,
      searched.line_height,
      searched.weight,
      &*searched.font_family
    ),
    (11.5, 1.3, FontWeight::Normal, "Geist Mono")
  );
  assert_eq!(searched.color, theme.palette_color(PaletteColor::TextMuted));
}
