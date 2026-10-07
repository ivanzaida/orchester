//! The roles of the project screens (tasks, pipelines, agents, secrets, import, pull requests,
//! activity, instructions, plan review, tabs) and of the controls they share: popovers, the focus
//! ring, select menus and the scroll gutter.

use lurq::{
  app::theme::{BorderSize, TypographyStyle},
  layout::{
    scrollbar::{ScrollBarPlacement, compute_vertical_scrollbar},
    text_style::FontWeight,
  },
  node::{BackgroundColor, BoxShadow, color::Color, dimension::Dimension},
};
use orchester_desktop::theme::{self, ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography};

use super::themed_app;

#[test]
fn plan_limits_emphasis_follows_pencil() {
  let app = themed_app();
  let percent = app.theme().typography_style(ExtraTypography::MonoLimit.role());
  assert_eq!(percent.font_size, 11.0);
  assert_eq!(percent.weight, FontWeight::Medium);
  assert_eq!(&*percent.font_family, "Geist Mono");
  assert_eq!(
    app.theme().spacing_value(ExtraSpacing::PlanRows.role()),
    Dimension::Px(6.0)
  );
  assert_eq!(theme::PLAN_HINT_WIDTH, 360.0);
  assert_eq!(theme::PLAN_WINDOW_LABEL, 44.0);
  assert_eq!((theme::PLAN_METER_WIDTH, theme::PLAN_METER_HEIGHT), (100.0, 4.0));
  assert_eq!(theme::PLAN_PERCENT_WIDTH, 32.0);
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
  // An acceptance criterion on the Summary, 12 at 1.5 (`j0Ye4t` in `tqn5P`).
  let criterion = theme.typography_style(ExtraTypography::Criterion.role());
  assert_eq!((criterion.font_size, criterion.line_height), (12.0, 1.5));
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
  assert_eq!(theme.typography_style(ExtraTypography::IconTag.role()).font_size, 10.0);
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
  // A locked switch fades with its label as one row (`lW7Sp` in 04.B `W0Owo`).
  assert_eq!(theme::LOCKED_SWITCH_OPACITY, 0.45);
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
fn import_roles_follow_pencil() {
  let app = themed_app();
  // `gHiGc` in the mapping headings `WcIvG` of 06.05 `QqnDd`: Inter 9 in `muted`.
  let heading = app.theme().typography_style(ExtraTypography::ColumnHeading.role());
  assert_eq!(
    (heading.font_size, &*heading.font_family, heading.weight),
    (9.0, "Inter", FontWeight::Normal)
  );
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
  // The room the Open tab keeps for its count (`tidLB` in `DF8II`) counts digits of Geist Mono 10,
  // which all advance 0.6 em.
  let count = app.theme().typography_style(TypographyStyle::Mono);
  assert_eq!((&*count.font_family, count.font_size), ("Geist Mono", 10.0));
  assert_eq!(theme::COUNT_DIGIT_WIDTH, 0.6 * count.font_size);
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

/// Transcript command box (`uA3Sh` in `N4SlQq`): its lines 4 from the viewport's top and bottom;
/// the horizontal thumb (`hpzPu` in `G9skx`) 4px in `border`, radius 2, no track, overlaid 1 from
/// the box's bottom and 12 from the viewport's ends.
#[test]
fn transcript_command_box_follows_pencil() {
  let app = themed_app();
  let theme = app.theme();
  assert_eq!(
    theme.spacing_value(ExtraSpacing::CommandLines.role()),
    Dimension::Px(4.0)
  );
  let scrollbar = theme::command_scrollbar();
  assert!(scrollbar.placement == ScrollBarPlacement::Overlay);
  assert_eq!((scrollbar.width, scrollbar.thumb_radius), (4.0, 2.0));
  assert_eq!(
    (scrollbar.resolved_edge_inset(), scrollbar.resolved_end_inset()),
    (1.0, 12.0)
  );
  assert_eq!(scrollbar.thumb_color, Color::from_hex("#363636"));
  assert_eq!(scrollbar.track_color, Color::from_hex("#00000000"));
}

/// Capped value box of the approval dialog (`E5NBo`, `e0vyng` in `bjrgu`): the transcript command's
/// thumbs, each in a 6px lane of its own (4 wide, 1 from each side), so neither covers a glyph,
/// running from the viewport's start.
#[test]
fn approval_value_box_scrollbars_keep_lanes_of_their_own() {
  let scrollbar = theme::value_box_scrollbar();
  assert!(scrollbar.placement == ScrollBarPlacement::Reserved);
  assert_eq!((scrollbar.width, scrollbar.thumb_radius), (4.0, 2.0));
  assert_eq!(
    (scrollbar.resolved_edge_inset(), scrollbar.resolved_end_inset()),
    (1.0, 0.0)
  );
  assert_eq!(scrollbar.thumb_color, Color::from_hex("#363636"));
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

/// A field error that wraps beside its icon (`foNvP` in `glurU`, `eeksu` in `XIEn8`): Inter 11 in
/// the text color, line height 1.5.
#[test]
fn wrapping_error_follows_pencil() {
  let app = themed_app();
  let error = app.theme().typography_style(ExtraTypography::WrappingError.role());
  assert_eq!(
    (&*error.font_family, error.font_size, error.line_height),
    ("Inter", 11.0, 1.5)
  );
  assert_eq!(error.color, Color::from_hex("#EEEEEE"));
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
  // The lock before an unavailable option's reason (`OoLYh`, `RuJto`): 11.
  assert_eq!(size(ExtraTypography::IconReason), 11.0);
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

#[test]
fn plan_review_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // Plan title `J7irj`: 19/500.
  let title = theme.typography_style(ExtraTypography::PlanTitle.role());
  assert_eq!((title.font_size, title.weight), (19.0, FontWeight::Medium));
  // The note for the agent `DDXc5` in `NbNN0`: 12, line height 1.6, padding 14.
  let note = theme.typography_style(ExtraTypography::ReviewNote.role());
  assert_eq!((note.font_size, note.line_height), (12.0, 1.6));
  assert_eq!(theme.spacing_value(ExtraSpacing::TextArea.role()), Dimension::Px(14.0));
  // Agent-written text `FP6BD`: 12, line height 1.6, in the text color.
  let agent = theme.typography_style(ExtraTypography::AgentText.role());
  assert_eq!((agent.font_size, agent.line_height), (12.0, 1.6));
  assert_eq!(agent.color, Color::from_hex("#EEEEEE"));
  // Agent-written Markdown reads in the grayscale chrome; code keeps the diff's code colors; a
  // first-level heading is 14 on the 12px body.
  let markdown = theme.markdown();
  assert_eq!(
    markdown.heading_1.text.font_size_scale.map(|scale| scale * 12.0),
    Some(14.0)
  );
  assert_eq!(markdown.code_block_box.background, Some(Color::from_hex("#0D1117")));
  assert_eq!(markdown.code_block.text.color, Some(Color::from_hex("#E6EDF3")));
  assert_eq!(markdown.link.text.color, Some(Color::from_hex("#EEEEEE")));
  assert_eq!(markdown.list_marker.text.color, Some(Color::from_hex("#A3A3A3")));
  assert_eq!(markdown.code_block.text.font_family.as_deref(), Some("Geist Mono"));
  // Its tables' columns share the text's width, however narrow (a chat message, ORC-78).
  assert_eq!(markdown.table_cell_min_width, 0.0);
}

/// Title bar tabs (`nAKyb`, states `UZF1Q`): the avatar's initial is Inter 9 semibold in `text`,
/// avatar and close button have radius 4, the edge fade of a scrolled strip ends in transparent
/// `canvas` (`xpSC0`), the switcher's *Open in new tab* has radius 5 (`NcJQ0`), and the macOS
/// traffic lights keep the platform's colors (`R3RtGs`).
#[test]
fn tab_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  let initial = theme.typography_style(ExtraTypography::TabInitial.role());
  assert_eq!(
    (&*initial.font_family, initial.font_size, initial.weight),
    ("Inter", 9.0, FontWeight::SemiBold)
  );
  assert_eq!(initial.color, Color::from_hex("#EEEEEE"));
  assert_eq!(theme.radius_value(ExtraRadius::TabControl.role()), 4.0);
  assert_eq!(theme.radius_value(ExtraRadius::RowAction.role()), 5.0);
  assert_eq!(
    theme.palette_color(ExtraColor::CanvasClear.role()),
    Color::from_hex("#10101000")
  );
  for (light, hex) in [
    (ExtraColor::TrafficClose, "#FF5F57"),
    (ExtraColor::TrafficMinimize, "#FEBC2E"),
    (ExtraColor::TrafficZoom, "#28C840"),
  ] {
    assert_eq!(theme.palette_color(light.role()), Color::from_hex(hex));
  }
}

#[test]
fn conversation_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // A timeline message's text (`V17jfc`): 13 at 1.6 in `$text`.
  let message = theme.typography_style(ExtraTypography::TimelineMessage.role());
  assert_eq!((message.font_size, message.line_height), (13.0, 1.6));
  assert_eq!(message.color, Color::from_hex("#EEEEEE"));
  // The working line starts where a chat message's text does: past the 28 avatar and its 12 gap
  // (`VCcdr` in `kpnmP`).
  assert_eq!(
    theme.spacing_value(ExtraSpacing::TimelineIndent.role()),
    Dimension::Px(40.0)
  );
  // A segment of the Show control, in the usage period style.
  assert_eq!(theme.radius_value(ExtraRadius::Segment.role()), 5.0);
  // Round ends up to 32 high (the 28 avatar `qU0XW`, the composer's pills and send button), a tool
  // chip's padding over and under its line (`a7zbJ`) and the gap between the badges of the
  // collapsed rail (`fFDrw`).
  assert_eq!(theme.radius_value(ExtraRadius::Round.role()), 16.0);
  assert_eq!(theme.spacing_value(ExtraSpacing::Chip.role()), Dimension::Px(5.0));
  assert_eq!(theme.spacing_value(ExtraSpacing::Snug.role()), Dimension::Px(4.0));
  // Inside an inset focus ring: the ring and its gap (`Oz9EN`).
  assert_eq!(theme.spacing_value(ExtraSpacing::FocusInset.role()), Dimension::Px(4.0));
  // The composer (`n0wdPC` in `BRMpj`): its box's radius and its dock's room above and under it.
  assert_eq!(theme.radius_value(ExtraRadius::Composer.role()), 18.0);
  assert_eq!(
    theme.spacing_value(ExtraSpacing::ComposerTop.role()),
    Dimension::Px(4.0)
  );
  assert_eq!(
    theme.spacing_value(ExtraSpacing::ComposerEdge.role()),
    Dimension::Px(18.0)
  );
}

#[test]
fn server_roles_follow_pencil() {
  let app = themed_app();
  let theme = app.theme();
  // What answered at an address in Add server (`q2uPd` in 10.02): 12 at 1.5 in `$text`.
  let heading = theme.typography_style(ExtraTypography::ResultHeading.role());
  assert_eq!((heading.font_size, heading.line_height), (12.0, 1.5));
  assert_eq!(heading.color, Color::from_hex("#EEEEEE"));
  // A first password shown once (`pUTsa` in 10.08 B): mono 14, spaced 0.5.
  let password = theme.typography_style(ExtraTypography::FirstPassword.role());
  assert_eq!((password.font_size, password.letter_spacing), (14.0, 0.5));
  // That it is shown only once (`nSpO0`): 11 at 1.5 in `$text`.
  let caution = theme.typography_style(ExtraTypography::Caution.role());
  assert_eq!((caution.font_size, caution.line_height), (11.0, 1.5));
  assert_eq!(caution.color, Color::from_hex("#EEEEEE"));
  // A server's name in the status bar's hint (`e8ZUC` in 10.43 A): 11/500.
  let name = theme.typography_style(ExtraTypography::HintHeading.role());
  assert_eq!(name.font_size, 11.0);
  assert_eq!(name.weight, FontWeight::Medium);
}
