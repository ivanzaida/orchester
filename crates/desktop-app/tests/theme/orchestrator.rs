//! The Orchestrator's own type roles and spacing (`TrG83`, `Xir64`), and its replies' links (`agv8r`).
use super::*;

#[test]
fn orchestrator_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, size, line, weight, family) in [
    (OrchestratorText::PanelTitle, 13.0, 1.3, FontWeight::SemiBold, "Geist"),
    (OrchestratorText::Emphasis, 12.5, 1.35, FontWeight::Medium, "Geist"),
    (OrchestratorText::Eyebrow, 11.0, 1.3, FontWeight::Normal, "Geist Mono"),
    (OrchestratorText::Suggestion, 12.0, 1.4, FontWeight::Normal, "Geist"),
    (OrchestratorText::ContextLink, 11.0, 1.4, FontWeight::Medium, "Geist"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line, weight, family),
      "{role:?}"
    );
  }
  assert_eq!(
    theme.typography_style(OrchestratorText::Eyebrow.role()).letter_spacing,
    0.6
  );
  for (spacing, value) in [
    (OrchestratorSpacing::Start, 20.0),
    (OrchestratorSpacing::SettingsPage, 40.0),
    (OrchestratorSpacing::HistoryDay, 18.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}

#[test]
fn orchestrator_link_roles_follow_the_link_states() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (role, family) in [
    (OrchestratorText::LinkHost, "Geist"),
    (OrchestratorText::LinkAddress, "Geist Mono"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, &*style.font_family, style.color),
      (11.5, FontWeight::Normal, family, Color::from_hex("#8A8A92")),
      "{role:?} in `text_muted`"
    );
  }
  let link = theme
    .markdown()
    .link
    .apply(&theme.typography_style(TypographyStyle::Body));
  assert_eq!(
    (link.weight, link.color),
    (FontWeight::Medium, Color::from_hex("#EDEDF0")),
    "a link's words in `accent_text` 500"
  );
  let links = theme.markdown();
  let ring = links.link_focus_ring.expect("a link's focus ring");
  assert_eq!(
    (ring.width, ring.color, ring.radius),
    (2.0, Color::from_hex("#A3A3AB"), 3.0),
    "keyboard focus draws 2 of `border_focus` around a link, radius 3"
  );
  assert_eq!(
    (links.link_tooltip_box.background, links.link_tooltip_box.radius),
    (Some(Color::from_hex("#EDEDF0")), Some(4.0)),
    "a link's tooltip as the kit's, on `text_primary`"
  );
}

#[test]
fn orchestrator_chat_link_looks_follow_the_link_states() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let looks = ChatLinkLooks::of(theme, &theme.typography_style(TypographyStyle::Body));
  let underline = looks.link.text.underline.expect("a reply's link is underlined");
  assert_eq!(
    (underline.color, underline.thickness),
    (Some(Color::from_hex("#8A8A92")), Some(1.0)),
    "a 1 `text_muted` underline"
  );
  let body = theme.typography_style(TypographyStyle::Body);
  let box_below_baseline = (body.line_height - 1.3) / 2.0 * body.font_size + 0.295 * body.font_size;
  assert!(
    underline
      .offset
      .is_some_and(|offset| (offset + 1.0 - box_below_baseline).abs() < 0.001),
    "the underline ends where the line box, and a hovered link's fill, ends: {underline:?}"
  );
  assert_eq!(
    looks.focused.underline_color,
    Some(Color::from_hex("#00000000")),
    "with keyboard focus the ring stands for the underline"
  );
  assert_eq!(
    (
      looks.hovered.color,
      looks.hovered.underline_color,
      looks.hovered.background,
      looks.hovered.trailing_color
    ),
    (
      Some(Color::from_hex("#FFFFFF")),
      Some(Color::from_hex("#FFFFFF")),
      Some(Color::from_hex("#FFFFFF0F")),
      Some(Color::from_hex("#B3B3BA"))
    ),
    "under the pointer `accent_hover` on `hover`, the host `text_secondary`"
  );
  assert_eq!(
    looks.radius.to_array(),
    [3.0, 3.0, 0.0, 0.0],
    "square at the underline's side"
  );
  for (gap, width) in [(&looks.host_gap, 6.0), (&looks.icon_gap, 3.0)] {
    let size = gap.text.font_size.expect("a gap's size");
    let spacing = gap.text.letter_spacing.expect("a gap's spacing");
    assert!(
      (size * 0.25 + spacing - width).abs() < 0.01,
      "a space widened to {width}"
    );
  }
}

#[test]
fn orchestrator_labelled_link_roles_follow_the_grid() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let label = theme.typography_style(OrchestratorText::LinkLabel.role());
  assert_eq!((label.font_size, label.color), (12.0, Color::from_hex("#8A8A92")));
  let detail = theme.typography_style(OrchestratorText::LinkDetail.role());
  assert_eq!(
    (detail.font_size, detail.line_height, detail.color),
    (12.5, 1.5, Color::from_hex("#B3B3BA"))
  );
  assert_eq!(
    theme.spacing_value(OrchestratorSpacing::LinkColumns.role()),
    Dimension::Px(12.0)
  );
}
