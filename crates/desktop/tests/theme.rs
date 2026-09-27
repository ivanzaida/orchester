use lurq::{
  app::{
    App,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  layout::{
    scrollbar::{ScrollBarPlacement, compute_vertical_scrollbar},
    text_style::{FontFeature, FontFeatures, FontWeight},
  },
  node::{BackgroundColor, BoxShadow, color::Color, dimension::Dimension},
};
use orchester_desktop::theme::{self, ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography};

fn themed_app() -> App {
  let mut app = App::new();
  theme::install(&mut app);
  app
}

#[test]
fn palette_roles_resolve_to_pencil_values() {
  let app = themed_app();
  let theme = app.theme();
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#101010")
  );
  assert_eq!(
    theme.palette_color(PaletteColor::TextPrimary),
    Color::from_hex("#EEEEEE")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Surface.role()),
    Color::from_hex("#1A1A1A")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Hairline.role()),
    Color::from_hex("#2B2B2B")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Scrim.role()),
    Color::from_hex("#000000A6")
  );
}

#[test]
fn typography_and_radii_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let heading = theme.typography_style(TypographyStyle::Heading);
  assert_eq!(heading.font_size, 30.0);
  assert_eq!(&*heading.font_family, "Inter");
  assert_eq!(theme.radius_value(RadiusSize::Md), 7.0);
  assert_eq!(theme.radius_value(RadiusSize::Lg), 12.0);
}

#[test]
fn extra_typography_follows_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let overline = theme.typography_style(ExtraTypography::Overline.role());
  assert_eq!(overline.font_size, 9.0);
  assert_eq!(overline.weight, FontWeight::SemiBold);
  assert_eq!(
    theme.typography_style(ExtraTypography::SectionHeading.role()).font_size,
    15.0
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::CardTitle.role()).font_size,
    14.0
  );
  assert_eq!(theme.typography_style(ExtraTypography::Metric.role()).font_size, 23.0);
}

#[test]
fn getting_started_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let display = theme.typography_style(ExtraTypography::Display.role());
  assert_eq!(display.font_size, 44.0);
  assert_eq!(display.letter_spacing, -1.2);
  assert_eq!(
    theme.typography_style(ExtraTypography::Eyebrow.role()).letter_spacing,
    1.6
  );
  assert_eq!(
    &*theme.typography_style(ExtraTypography::IconNav.role()).font_family,
    "lucide"
  );
  assert_eq!(theme.spacing_value(ExtraSpacing::Columns.role()), Dimension::Px(112.0));
  assert_eq!(theme.spacing_value(SpacingSize::Section), Dimension::Px(32.0));
  assert_eq!(theme.radius_value(ExtraRadius::Notice.role()), 8.0);
  assert_eq!(
    theme.typography_style(ExtraTypography::PageTitle.role()).font_size,
    27.0
  );
  assert_eq!(theme.typography_style(ExtraTypography::Detail.role()).line_height, 1.6);
  assert_eq!(theme.spacing_value(ExtraSpacing::Tight.role()), Dimension::Px(6.0));
  assert_eq!(theme.spacing_value(ExtraSpacing::Indicator.role()), Dimension::Px(7.0));
  // Rows of the status bar operations popover (`E1msJS`).
  assert_eq!(theme.spacing_value(ExtraSpacing::Hair.role()), Dimension::Px(2.0));
  assert_eq!(
    theme.spacing_value(ExtraSpacing::DialogMargin.role()),
    Dimension::Px(32.0)
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::DialogTitle.role()).font_size,
    20.0
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::Explanation.role()).line_height,
    1.6
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::IconNotice.role()).font_size,
    18.0
  );
}

#[test]
fn mono_text_uses_bundled_geist_mono() {
  let app = themed_app();
  let mono = app.theme().typography_style(TypographyStyle::Mono);
  assert_eq!(&*mono.font_family, "Geist Mono");
  assert_eq!(
    &*app.theme().typography_style(ExtraTypography::Metric.role()).font_family,
    "Geist Mono"
  );
}

#[test]
fn mono_field_path_follows_pencil() {
  let app = themed_app();
  let path = app.theme().typography_style(ExtraTypography::MonoField.role());
  assert_eq!((&*path.font_family, path.font_size), ("Geist Mono", 12.0));
}

#[test]
fn mono_text_keeps_the_space_before_double_hyphens() {
  let app = themed_app();
  let off = FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]);
  for role in [
    TypographyStyle::Mono,
    ExtraTypography::MonoValue.role(),
    ExtraTypography::MonoField.role(),
    ExtraTypography::Metric.role(),
  ] {
    assert_eq!(app.theme().typography_style(role).font_features, off);
  }
}

#[test]
fn task_screen_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let size = |role: ExtraTypography| theme.typography_style(role.role()).font_size;
  assert_eq!(size(ExtraTypography::ScreenTitle), 28.0);
  assert_eq!(size(ExtraTypography::FormLabel), 12.0);
  assert_eq!(size(ExtraTypography::StatusLabel), 11.0);
  assert_eq!(size(ExtraTypography::NavCount), 11.0);
  assert_eq!(size(ExtraTypography::IconMarker), 13.0);
  assert_eq!(
    &*theme.typography_style(ExtraTypography::NavCount.role()).font_family,
    "Geist Mono"
  );
  let brief = theme.typography_style(ExtraTypography::Brief.role());
  assert_eq!((brief.font_size, brief.line_height), (13.0, 1.7));
  assert_eq!(
    theme.spacing_value(ExtraSpacing::HeadingTop.role()),
    Dimension::Px(26.0)
  );
  assert_eq!(
    theme.spacing_value(ExtraSpacing::HeadingBottom.role()),
    Dimension::Px(22.0)
  );
  assert_eq!(theme.border_size_value(BorderSize::Lg), 2.0);
}

#[test]
fn pipeline_editor_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let label = theme.typography_style(ExtraTypography::CanvasLabel.role());
  assert_eq!((label.font_size, label.letter_spacing), (9.0, 1.0));
  assert_eq!(&*label.font_family, "Geist Mono");
  assert_eq!(
    theme.typography_style(ExtraTypography::MonoValue.role()).font_size,
    11.0
  );
  assert_eq!(
    theme.typography_style(ExtraTypography::Instructions.role()).line_height,
    1.65
  );
  assert_eq!(theme.typography_style(ExtraTypography::Hint.role()).font_size, 10.0);
  assert_eq!(
    theme.typography_style(ExtraTypography::VersionFigure.role()).font_size,
    18.0
  );
  assert_eq!(theme.typography_style(ExtraTypography::Summary.role()).line_height, 1.6);
  assert_eq!(
    theme.typography_style(ExtraTypography::IconLarge.role()).font_size,
    22.0
  );
  assert_eq!(theme.spacing_value(ExtraSpacing::Field.role()), Dimension::Px(9.0));
  assert_eq!(theme.spacing_value(ExtraSpacing::Property.role()), Dimension::Px(15.0));
  assert_eq!(theme.spacing_value(ExtraSpacing::Divider.role()), Dimension::Px(18.0));
}

#[test]
fn agents_and_connections_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let inspector_title = theme.typography_style(ExtraTypography::InspectorTitle.role());
  assert_eq!(
    (inspector_title.font_size, inspector_title.weight),
    (18.0, FontWeight::Medium)
  );
  let compact = theme.typography_style(ExtraTypography::CompactStatusLabel.role());
  assert_eq!((compact.font_size, compact.weight), (10.0, FontWeight::Medium));
  assert_eq!(theme.spacing_value(ExtraSpacing::Form.role()), Dimension::Px(22.0));
  // Permission options a connection cannot enforce (`BNliD` in 04.04 `T1rjv`).
  assert_eq!(theme::UNAVAILABLE_OPTION_OPACITY, 0.5);
}

#[test]
fn secrets_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // `B1zGN` in the secret row `vxAMD`: Geist Mono 12 in `text`.
  let name = theme.typography_style(ExtraTypography::MonoTitle.role());
  assert_eq!((name.font_size, &*name.font_family), (12.0, "Geist Mono"));
  // `FNlaR` in the secret inspector `CQxCb`: Geist Mono 13 in `muted`.
  let masked = theme.typography_style(ExtraTypography::MaskedValue.role());
  assert_eq!((masked.font_size, &*masked.font_family), (13.0, "Geist Mono"));
}

#[test]
fn pull_request_roles_follow_pencil() {
  let app = themed_app();
  let title = app.theme().typography_style(ExtraTypography::RowTitle.role());
  // `G2kdX` in the pull request list row `QHBNZ`: Inter 15/500 in `text`.
  assert_eq!((title.font_size, title.weight), (15.0, FontWeight::Medium));
  // `r0ZlXX`: the row's lucide icon at 20px.
  let icon = app.theme().typography_style(ExtraTypography::IconRow.role());
  assert_eq!(icon.font_size, 20.0);
}

#[test]
fn pull_request_diff_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // Pencil `code-*` and `diff-*` variables, used by the diff line `vEZ42`.
  for (role, hex) in [
    (ExtraColor::CodeBackground, "#0D1117"),
    (ExtraColor::CodeText, "#E6EDF3"),
    (ExtraColor::CodeKeyword, "#FF7B72"),
    (ExtraColor::CodeString, "#A5D6FF"),
    (ExtraColor::CodeFunction, "#D2A8FF"),
    (ExtraColor::CodeVariable, "#FFA657"),
    (ExtraColor::CodeConstant, "#79C0FF"),
    (ExtraColor::CodeComment, "#8B949E"),
    (ExtraColor::DiffAddedBackground, "#14261C"),
    (ExtraColor::DiffAddedMark, "#7EE787"),
    (ExtraColor::DiffRemovedBackground, "#301C20"),
    (ExtraColor::DiffRemovedMark, "#FFA198"),
  ] {
    assert_eq!(theme.palette_color(role.role()), Color::from_hex(hex), "{hex}");
  }
  // Code tokens: Geist Mono 12, line height 1.6 (`DQMrs`); line numbers Geist Mono 11 in
  // `code-comment` (`b8bSop`); the pull request title Inter 24/500 (`CNO65`); the context line
  // Geist Mono 11 (`ADELW`); the diff's file path Geist Mono 14 (`rxiPH`).
  let code = theme.typography_style(ExtraTypography::Code.role());
  assert_eq!(
    (&*code.font_family, code.font_size, code.line_height),
    ("Geist Mono", 12.0, 1.6)
  );
  assert_eq!(code.color, Color::from_hex("#E6EDF3"));
  let number = theme.typography_style(ExtraTypography::DiffLineNumber.role());
  assert_eq!((number.font_size, number.color), (11.0, Color::from_hex("#8B949E")));
  let title = theme.typography_style(ExtraTypography::PullRequestTitle.role());
  assert_eq!((title.font_size, title.weight), (24.0, FontWeight::Medium));
  let context = theme.typography_style(ExtraTypography::PullRequestContext.role());
  assert_eq!((&*context.font_family, context.font_size), ("Geist Mono", 11.0));
  assert_eq!(theme.typography_style(ExtraTypography::DiffPath.role()).font_size, 14.0);
}

#[test]
fn popover_shadow_follows_pencil() {
  let app = themed_app();
  let shadow = app.theme().shadow_style(ExtraShadow::Popover.role());
  // `E1msJS`: outer, x 0, y 8, blur 24, no spread, #00000066.
  assert_eq!(
    &*shadow,
    &[BoxShadow::new(
      0.0,
      8.0,
      24.0,
      BackgroundColor::from(Color::from_hex("#00000066"))
    )]
  );
}

#[test]
fn focus_ring_follows_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // `xURvJ`: `focus-ring` #EEEEEE, `focus-gap` #101010, `focus-width` 2, `focus-extent` 4.
  assert_eq!(
    theme.palette_color(ExtraColor::FocusRing.role()),
    Color::from_hex("#EEEEEE")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::FocusGap.role()),
    Color::from_hex("#101010")
  );
  let ring = |spread: f32, color: ExtraColor| BoxShadow::new(0.0, 0.0, 0.0, color.role()).spread(spread);
  assert_eq!(
    &*theme.shadow_style(ExtraShadow::FocusRing.role()),
    &[ring(2.0, ExtraColor::FocusGap), ring(4.0, ExtraColor::FocusRing)]
  );
  assert_eq!(
    &*theme.shadow_style(ExtraShadow::FocusRingInset.role()),
    &[
      ring(2.0, ExtraColor::FocusRing).inset(),
      ring(4.0, ExtraColor::FocusGap).inset()
    ]
  );
}

#[test]
fn select_menu_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // `u0ByM`: radius 11, padding 4.
  assert_eq!(theme.radius_value(ExtraRadius::SelectMenu.role()), 11.0);
  assert_eq!(theme.spacing_value(ExtraSpacing::SelectMenu.role()), Dimension::Px(4.0));
  // Chevron `eASPF` and check `ClxwV`: lucide 14, which is also the reserved check slot.
  let icon = theme.typography_style(ExtraTypography::IconSelect.role());
  assert_eq!((&*icon.font_family, icon.font_size), ("lucide", 14.0));
  assert_eq!(theme::SELECT_CHECK_SLOT, 14.0);
  // `YD8PV`: overlay thumb 4px in `border`, radius 2, 3px from the outer right edge, 11px from
  // the top and bottom, at least 24px long, no track.
  let scrollbar = theme::select_menu_scrollbar();
  assert_eq!((scrollbar.width, scrollbar.thumb_radius), (4.0, 2.0));
  assert_eq!(
    (
      scrollbar.resolved_edge_inset(),
      scrollbar.resolved_end_inset(),
      scrollbar.min_thumb_length
    ),
    (3.0, 11.0, 24.0)
  );
  assert_eq!(scrollbar.thumb_color, Color::from_hex("#363636"));
  assert_eq!(scrollbar.track_color, Color::from_hex("#00000000"));
}

/// Scroll gutter (`nf9DG` in `NwsEg`, 04.01 `JuVaA`): a 12px gutter of its own; the thumb is 4px
/// in `border`, radius 2, centred in the gutter, 4px from the viewport's top and bottom, at least
/// 24px long and as long as the visible share of the track; no track.
#[test]
fn scroll_gutter_follows_pencil() {
  let scrollbar = theme::gutter_scrollbar();
  assert!(scrollbar.placement == ScrollBarPlacement::Reserved);
  assert_eq!((scrollbar.width, scrollbar.thumb_radius), (4.0, 2.0));
  assert_eq!(scrollbar.thumb_color, Color::from_hex("#363636"));
  assert_eq!(scrollbar.track_color, Color::from_hex("#00000000"));
  assert_eq!(scrollbar.min_thumb_length, 24.0);
  // A 344px inspector with its 24 / 12 side padding: a 308px viewport, twice as tall content.
  let thumb = compute_vertical_scrollbar(&scrollbar, 24.0, 24.0, 308.0, 800.0, 1600.0, 0.0)
    .expect("the thumb shows while the content overflows");
  let gutter_start = 24.0 + 308.0 - 12.0;
  assert_eq!(
    (thumb.thumb_x - gutter_start, thumb.thumb_width),
    (4.0, 4.0),
    "centred in the 12px gutter"
  );
  assert_eq!((thumb.thumb_y, thumb.thumb_height), (24.0 + 4.0, (800.0 - 8.0) / 2.0));
  assert!(compute_vertical_scrollbar(&scrollbar, 24.0, 24.0, 308.0, 800.0, 800.0, 0.0).is_none());
}

/// Worktree state label (`VHSnL` in `Py65X`): Inter 11 muted, line height 1.5.
#[test]
fn cell_label_follows_pencil() {
  let app = themed_app();
  let label = app.theme().typography_style(ExtraTypography::CellLabel.role());
  assert_eq!(
    (&*label.font_family, label.font_size, label.line_height),
    ("Inter", 11.0, 1.5)
  );
  assert_eq!(label.color, Color::from_hex("#A3A3A3"));
}

/// Inspector notes (`xK9v9`, `Thdx2` in `JuVaA`): Inter 10 muted, line height 1.6.
#[test]
fn inspector_note_follows_pencil() {
  let app = themed_app();
  let note = app.theme().typography_style(ExtraTypography::InspectorNote.role());
  assert_eq!(
    (&*note.font_family, note.font_size, note.line_height),
    ("Inter", 10.0, 1.6)
  );
  assert_eq!(note.color, Color::from_hex("#A3A3A3"));
}

#[test]
fn activity_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // Command output `mMUyR`: Geist Mono 11 muted, line height 1.6.
  let output = theme.typography_style(ExtraTypography::Output.role());
  assert_eq!(
    (&*output.font_family, output.font_size, output.line_height),
    ("Geist Mono", 11.0, 1.6)
  );
  assert_eq!(output.color, Color::from_hex("#A3A3A3"));
  // Entry glyph `DQmo9` 13, event glyph `xQYdP` 14, state icon `qgZru` 20, all lucide.
  let size = |role: ExtraTypography| theme.typography_style(role.role()).font_size;
  assert_eq!(size(ExtraTypography::IconEntry), 13.0);
  assert_eq!(size(ExtraTypography::IconEvent), 14.0);
  assert_eq!(size(ExtraTypography::IconState), 20.0);
  // Switch `K8ItM`: radius 8.
  assert_eq!(theme.radius_value(ExtraRadius::Pill.role()), 8.0);
  // Unparsed lines `Ane9j`: 60 %.
  assert_eq!(theme::UNPARSED_OPACITY, 0.6);
}

#[test]
fn instruction_screen_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let style = |role: ExtraTypography| theme.typography_style(role.role());
  assert_eq!(style(ExtraTypography::RuleTitle).font_size, 20.0);
  let prompt = style(ExtraTypography::PromptText);
  assert_eq!((prompt.font_size, prompt.line_height), (12.0, 1.8));
  assert_eq!(&*prompt.font_family, "Geist Mono");
  let name = style(ExtraTypography::VariableName);
  assert_eq!((name.font_size, &*name.font_family), (20.0, "Geist Mono"));
  assert_eq!(style(ExtraTypography::ContextNote).font_size, 9.0);
  assert_eq!(style(ExtraTypography::IconState).font_size, 20.0);
}
