use lurq::{
  app::{
    App,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  layout::text_style::{FontFeature, FontFeatures, FontWeight},
  node::{color::Color, dimension::Dimension},
};
use orchester_desktop::theme::{self, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography};

mod screens;

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
  assert_eq!(theme.palette_color(PaletteColor::Danger), Color::from_hex("#F87171"));
}

#[test]
fn window_chrome_follows_pencil() {
  // Window composition `cpoal`: 32 title bar + 960 screen + 28 status bar (`pmOpt`).
  assert_eq!(theme::TITLE_BAR_HEIGHT, 32.0);
  assert_eq!(theme::STATUS_BAR_HEIGHT, 28.0);
}

#[test]
fn disabled_buttons_follow_pencil() {
  // A button that cannot be pressed, primary or secondary, at 40 % (`kLJJb`, `HN7cO`).
  assert_eq!(theme::DISABLED_OPACITY, 0.4);
  let app = themed_app();
  let form = app.theme().form().clone();
  // It keeps its variant's colors (`u1XBP`, `HN7cO`): only the opacity marks it.
  assert_eq!(
    (form.button.primary.background, form.button.primary.text.color),
    (PaletteColor::Accent, PaletteColor::TextInverse)
  );
  assert_eq!(
    (form.button.secondary.background, form.button.secondary.border),
    (PaletteColor::SurfaceRaised, PaletteColor::Border)
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
  let wrapped = theme.typography_style(ExtraTypography::WrappingProperty.role());
  assert_eq!((wrapped.font_size, wrapped.line_height), (11.0, 1.6));
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
  // Summary column beside its 12px scroll gutter: content still ends 32 from the edge (`JV8S8`).
  assert_eq!(
    theme.spacing_value(ExtraSpacing::BesideGutter.role()),
    Dimension::Px(20.0)
  );
  // The uncommitted files under a check row's title, which starts after its icon (`gzztk`).
  assert_eq!(
    theme.spacing_value(ExtraSpacing::CheckIndent.role()),
    Dimension::Px(28.0)
  );
  // Why an option cannot be chosen, past a compact radio and its gap (`J5RohL`, `L1B0m`).
  assert_eq!(
    theme.spacing_value(ExtraSpacing::ReasonIndent.role()),
    Dimension::Px(22.0)
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
  // The environment's `NAME=value` lines are the same mono 12 at 1.6 (`f0e2Hv` in `BGMgO`).
  let area = app.theme().typography_style(ExtraTypography::MonoArea.role());
  assert_eq!(
    (&*area.font_family, area.font_size, area.line_height),
    ("Geist Mono", 12.0, 1.6)
  );
}

#[test]
fn agent_enforcement_row_text_matches_pencil_line_boxes() {
  let app = themed_app();
  let setting = app.theme().typography_style(ExtraTypography::EnforcementSetting.role());
  assert_eq!((setting.font_size, setting.line_height), (12.0, 1.25));
  let mechanism = app
    .theme()
    .typography_style(ExtraTypography::EnforcementMechanism.role());
  assert_eq!(
    (&*mechanism.font_family, mechanism.font_size, mechanism.line_height),
    ("Geist Mono", 10.0, 1.3)
  );
}

#[test]
fn version_history_change_lines_follow_pencil() {
  let app = themed_app();
  // A change of a version, 12 at 1.4, beside a 17px icon slot: one line of it (`o6to5`, `Q1Geah`).
  let line = app.theme().typography_style(ExtraTypography::ChangeLine.role());
  assert_eq!((line.font_size, line.line_height), (12.0, 1.4));
}

#[test]
fn mono_text_keeps_the_space_before_double_hyphens() {
  let app = themed_app();
  let off = FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]);
  for role in [
    TypographyStyle::Mono,
    ExtraTypography::MonoValue.role(),
    ExtraTypography::MonoField.role(),
    ExtraTypography::MonoArea.role(),
    ExtraTypography::Metric.role(),
  ] {
    assert_eq!(app.theme().typography_style(role).font_features, off);
  }
}
