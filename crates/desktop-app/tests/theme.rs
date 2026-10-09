//! The theme's roles resolve to the variables and type roles of `data/orchester2.pen` (accent graphite),
//! in dark and light mode, and the mode switches while the app runs.

use lurq::{
  app::{
    App,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  },
  layout::text_style::FontWeight,
  node::{color::Color, dimension::Dimension},
};
use orchester_app::{
  appearance::{
    Appearance, AppearanceError, ThemeSetting,
    system::{macos_mode, windows_mode},
  },
  settings::AppSettings,
  theme::{self, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, Mode},
};

fn themed(mode: Mode) -> App {
  let mut app = App::new();
  theme::install(&mut app, mode);
  app
}

fn assert_colors(theme: &Theme, mode: Mode, expected: &[(PaletteColor, &str)]) {
  for (role, hex) in expected {
    assert_eq!(
      theme.palette_color(role.clone()),
      Color::from_hex(hex),
      "{role:?} in {}",
      mode.name()
    );
  }
}

#[test]
fn palette_roles_follow_the_design_in_both_modes() {
  use PaletteColor as P;
  let dark = [
    (P::Accent, "#EDEDF0"),
    (P::AccentMuted, "#FFFFFF14"),
    (P::SurfaceBase, "#111113"),
    (P::SurfacePanel, "#18181A"),
    (P::SurfaceRaised, "#202023"),
    (P::SurfaceInput, "#1B1B1E"),
    (P::Border, "#2A2A2E"),
    (P::BorderStrong, "#3B3B40"),
    (P::BorderFocus, "#A3A3AB"),
    (P::TextPrimary, "#EDEDF0"),
    (P::TextSecondary, "#B3B3BA"),
    (P::TextMuted, "#8A8A92"),
    (P::Warning, "#E2A33C"),
    (P::DangerMuted, "#F0605A24"),
  ];
  let light = [
    (P::Accent, "#18181B"),
    (P::SurfaceBase, "#E4E4E8"),
    (P::SurfacePanel, "#FFFFFF"),
    (P::Border, "#E2E2E6"),
    (P::BorderStrong, "#C7C7CE"),
    (P::TextPrimary, "#18181B"),
    (P::TextMuted, "#6B6B74"),
    (P::Success, "#158354"),
  ];
  assert_colors(themed(Mode::Dark).theme(), Mode::Dark, &dark);
  assert_colors(themed(Mode::Light).theme(), Mode::Light, &light);
}

#[test]
fn every_extra_color_is_registered_with_its_design_value() {
  let expected = |extra: ExtraColor| match extra {
    ExtraColor::AccentText => ("#EDEDF0", "#18181B"),
    ExtraColor::OnAccent => ("#111113", "#FFFFFF"),
    ExtraColor::PanelBorder => ("#2A2A2E", "#C7C7CE"),
    ExtraColor::Hover => ("#FFFFFF0F", "#0000000A"),
    ExtraColor::Pressed => ("#FFFFFF17", "#00000012"),
    ExtraColor::Selection => ("#2F6FEB", "#2F6FEB"),
    ExtraColor::Scrim => ("#000000A3", "#18181B59"),
    ExtraColor::Shadow => ("#00000099", "#0F0F1A1F"),
    ExtraColor::OwnMessage => ("#232326", "#F1F1F4"),
    ExtraColor::CodeBackground => ("#0E0E10", "#F6F6F8"),
    ExtraColor::CodeKeyword => ("#8AB4F8", "#2B5FC7"),
    ExtraColor::CodeString => ("#7DCFBF", "#0F7A6A"),
    ExtraColor::CodeNumber => ("#C3A6F7", "#7A4CC2"),
    ExtraColor::DiffAdded => ("#3DB07A1F", "#15835414"),
    ExtraColor::DiffRemoved => ("#F0605A1F", "#D4342F14"),
  };
  let (dark, light) = (themed(Mode::Dark), themed(Mode::Light));
  for extra in ExtraColor::ALL {
    let (dark_hex, light_hex) = expected(extra);
    assert_eq!(
      dark.theme().palette_color(extra.role()),
      Color::from_hex(dark_hex),
      "{}",
      extra.name()
    );
    assert_eq!(
      light.theme().palette_color(extra.role()),
      Color::from_hex(light_hex),
      "{}",
      extra.name()
    );
  }
}

#[test]
fn type_roles_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let check = |role: TypographyStyle, size: f32, weight: FontWeight, tracking: f32, family: &str| {
    let style = theme.typography_style(role);
    assert_eq!(
      (style.font_size, style.weight, style.letter_spacing, &*style.font_family),
      (size, weight, tracking, family),
      "{role:?}"
    );
  };
  check(
    ExtraTypography::Display.role(),
    28.0,
    FontWeight::SemiBold,
    -0.5,
    "Geist",
  );
  check(TypographyStyle::Heading, 20.0, FontWeight::SemiBold, -0.2, "Geist");
  check(TypographyStyle::Title, 15.0, FontWeight::SemiBold, -0.1, "Geist");
  check(TypographyStyle::Body, 13.0, FontWeight::Normal, 0.0, "Geist");
  check(TypographyStyle::Description, 12.0, FontWeight::Normal, 0.0, "Geist");
  check(TypographyStyle::Caption, 11.0, FontWeight::Normal, 0.0, "Geist");
  check(TypographyStyle::Label, 12.0, FontWeight::Medium, 0.0, "Geist");
  check(TypographyStyle::FieldLabel, 11.0, FontWeight::Medium, 0.0, "Geist");
  check(TypographyStyle::Button, 12.0, FontWeight::Medium, 0.0, "Geist");
  check(TypographyStyle::Link, 13.0, FontWeight::Normal, 0.0, "Geist");
  check(TypographyStyle::Mono, 12.0, FontWeight::Normal, 0.0, "Geist Mono");
  check(
    ExtraTypography::Initial.role(),
    12.0,
    FontWeight::SemiBold,
    0.0,
    "Geist",
  );
  let color = |role: TypographyStyle| theme.typography_style(role).color;
  assert_eq!(color(TypographyStyle::Description), Color::from_hex("#B3B3BA"));
  assert_eq!(color(TypographyStyle::Caption), Color::from_hex("#8A8A92"));
  assert_eq!(color(TypographyStyle::FieldLabel), Color::from_hex("#B3B3BA"));
  assert_eq!(color(TypographyStyle::Link), Color::from_hex("#EDEDF0"));
  for size in IconSize::ALL {
    let glyph = theme.typography_style(size.role());
    assert_eq!(
      (&*glyph.font_family, glyph.font_size),
      ("lucide", size.size()),
      "{size:?}"
    );
  }
}

#[test]
fn spacing_radii_and_borders_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (size, value) in [
    (SpacingSize::Xs, 4.0),
    (SpacingSize::Sm, 8.0),
    (SpacingSize::Md, 12.0),
    (SpacingSize::Lg, 16.0),
    (SpacingSize::Xl, 24.0),
    (SpacingSize::Section, 32.0),
  ] {
    assert_eq!(theme.spacing_value(size), Dimension::Px(value), "{size:?}");
  }
  for extra in ExtraSpacing::ALL {
    assert_eq!(
      theme.spacing_value(extra.role()),
      Dimension::Px(extra.value()),
      "{extra:?}"
    );
  }
  assert_eq!(
    [RadiusSize::Sm, RadiusSize::Md, RadiusSize::Lg].map(|size| theme.radius_value(size)),
    [4.0, 6.0, 10.0]
  );
  for extra in ExtraRadius::ALL {
    assert_eq!(theme.radius_value(extra.role()), extra.value(), "{extra:?}");
  }
  assert_eq!(ExtraRadius::Full.value(), 999.0);
  assert_eq!(
    [BorderSize::Sm, BorderSize::Md, BorderSize::Lg].map(|size| theme.border_size_value(size)),
    [1.0, 2.0, 3.0]
  );
}

#[test]
fn switching_the_mode_recolors_the_palette_and_the_text() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  theme::apply(theme, Mode::Light);
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#E4E4E8")
  );
  assert_eq!(
    theme.palette_color(ExtraColor::Hover.role()),
    Color::from_hex("#0000000A")
  );
  assert_eq!(
    theme.typography_style(TypographyStyle::Body).color,
    Color::from_hex("#18181B")
  );
  assert_eq!(theme.scrollbar().thumb_color, Color::from_hex("#C7C7CE"));
  theme::apply(theme, Mode::Dark);
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#111113")
  );
}

#[test]
fn the_appearance_setting_switches_the_theme_and_is_kept() {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let database = dir.path().join("orchester.db");
  let settings = AppSettings::open(&database).expect("the settings open");
  let appearance = Appearance::new(ThemeSetting::Dark, Some(Mode::Light), Some(settings));
  let app = themed(appearance.mode());
  let mode = appearance
    .set(app.theme(), ThemeSetting::System)
    .expect("the choice is kept");
  assert_eq!(
    mode,
    Mode::Light,
    "system is the operating system's appearance at start"
  );
  assert_eq!(
    (appearance.setting(), appearance.mode()),
    (ThemeSetting::System, Mode::Light)
  );
  assert_eq!(
    app.theme().palette_color(PaletteColor::TextPrimary),
    Color::from_hex("#18181B")
  );
  let reopened = AppSettings::open(&database).expect("the settings open again");
  assert_eq!(reopened.theme().expect("the theme is stored"), ThemeSetting::System);
}

#[test]
fn without_settings_the_theme_does_not_change() {
  let appearance = Appearance::new(ThemeSetting::Dark, None, None);
  let app = themed(appearance.mode());
  assert!(matches!(
    appearance.set(app.theme(), ThemeSetting::Light),
    Err(AppearanceError::NoSettings)
  ));
  assert_eq!(
    (appearance.setting(), appearance.mode()),
    (ThemeSetting::Dark, Mode::Dark)
  );
  assert_eq!(
    app.theme().palette_color(PaletteColor::TextPrimary),
    Color::from_hex("#EDEDF0")
  );
  let unknown_system = Appearance::new(ThemeSetting::System, None, None);
  assert_eq!(
    unknown_system.mode(),
    Mode::Dark,
    "an unknown system appearance shows dark"
  );
}

#[test]
fn the_system_appearance_reads_from_each_platforms_answer() {
  let windows = |value: &str| {
    format!(
      "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\r\n    AppsUseLightTheme    REG_DWORD    {value}\r\n\r\n"
    )
  };
  assert_eq!(windows_mode(&windows("0x0")), Some(Mode::Dark));
  assert_eq!(windows_mode(&windows("0x1")), Some(Mode::Light));
  assert_eq!(
    windows_mode("ERROR: The system was unable to find the specified registry key"),
    None
  );
  assert_eq!(macos_mode(true, "Dark\n"), Mode::Dark);
  assert_eq!(macos_mode(false, ""), Mode::Light);
}
