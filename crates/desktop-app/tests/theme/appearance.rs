//! The mode switches while the app runs, follows the appearance setting and keeps it, and reads the system's
//! appearance on each platform.
use std::sync::{Arc, OnceLock};

use orchester_app::appearance::{
  Appearance, AppearanceError, ThemeSetting,
  system::{macos_mode, windows_mode},
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;

use super::*;

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
  let settings = Workspace::new(AppDb::open(&database).expect("the settings open"));
  let appearance = Appearance::new(
    ThemeSetting::Dark,
    Arc::new(OnceLock::from(Mode::Light)),
    Some(settings),
  );
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
  let reopened = AppDb::open(&database).expect("the settings open again");
  assert_eq!(reopened.theme().expect("the theme is stored"), ThemeSetting::System);
}

#[test]
fn without_settings_the_theme_does_not_change() {
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
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
  let unknown_system = Appearance::new(ThemeSetting::System, Arc::default(), None);
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
