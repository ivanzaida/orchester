//! The theme's roles resolve to the variables and type roles of `data/orchester2.pen` (accent graphite),
//! in dark and light mode, and the mode switches while the app runs.

use std::sync::{Arc, OnceLock};

use lurq::{
  app::{
    App,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  },
  layout::text_style::{FontFeature, FontFeatures, FontWeight},
  node::{color::Color, dimension::Dimension},
};
use orchester_app::{
  appearance::{
    Appearance, AppearanceError, ThemeSetting,
    system::{macos_mode, windows_mode},
  },
  theme::{
    self, ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography,
    IconSize, Mode, SHORTCUT_OPACITY, ShellText, StartupSpacing, StartupText,
  },
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;

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
    ExtraColor::BrandTile => ("#18191D", "#18191D"),
    ExtraColor::BrandMark => ("#F1F1F4", "#F1F1F4"),
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
  let window = if cfg!(target_os = "macos") { 10.0 } else { 8.0 };
  assert_eq!(
    theme.radius_value(ExtraRadius::Window.role()),
    window,
    "lurq clips macOS windows at 10"
  );
  assert_eq!(
    [BorderSize::Sm, BorderSize::Md, BorderSize::Lg].map(|size| theme.border_size_value(size)),
    [1.0, 2.0, 3.0]
  );
}

#[test]
fn area_roles_follow_the_startup_screens_and_the_switcher() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let check = |role: TypographyStyle, (size, line_height): (f32, f32), weight: FontWeight, family: &str| {
    let style = theme.typography_style(role);
    assert_eq!(
      (style.font_size, style.line_height, style.weight, &*style.font_family),
      (size, line_height, weight, family),
      "{role:?}"
    );
  };
  check(StartupText::Reason.role(), (13.0, 1.45), FontWeight::Medium, "Geist");
  check(StartupText::Step.role(), (13.0, 1.35), FontWeight::SemiBold, "Geist");
  check(StartupText::Path.role(), (11.5, 1.5), FontWeight::Normal, "Geist Mono");
  check(StartupText::Log.role(), (11.0, 1.45), FontWeight::Normal, "Geist Mono");
  check(StartupText::TileIcon.role(), (20.0, 1.0), FontWeight::Normal, "lucide");
  check(
    ShellText::SwitcherInitial.role(),
    (12.0, 1.35),
    FontWeight::SemiBold,
    "Geist",
  );
  check(
    ShellText::SwitcherPath.role(),
    (10.5, 1.35),
    FontWeight::Normal,
    "Geist Mono",
  );
  let color = |role: TypographyStyle| theme.typography_style(role).color;
  assert_eq!(color(StartupText::Log.role()), Color::from_hex("#8A8A92"));
  assert_eq!(color(ShellText::SwitcherInitial.role()), Color::from_hex("#B3B3BA"));
  assert_eq!(color(ShellText::SwitcherPath.role()), Color::from_hex("#8A8A92"));
  for (spacing, value) in [
    (StartupSpacing::Block, 20.0),
    (StartupSpacing::Panel, 40.0),
    (StartupSpacing::Fact, 5.0),
  ] {
    assert_eq!(spacing.value(), value, "{spacing:?}");
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
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

/// The kit's type roles (`ASNYj`, `DMMLd`, the dogfood boards): size, weight, line height, family and
/// color in each mode.
#[test]
fn theme_kit_type_roles_follow_the_design_in_both_modes() {
  use ExtraTypography as T;
  type Expected = (f32, FontWeight, f32, &'static str, (&'static str, &'static str));
  let expected = |role: ExtraTypography| -> Option<Expected> {
    let (dark_primary, light_primary) = ("#EDEDF0", "#18181B");
    let (dark_secondary, light_secondary) = ("#B3B3BA", "#45454D");
    let (dark_muted, light_muted) = ("#8A8A92", "#6B6B74");
    Some(match role {
      T::Display => return None,
      T::Strong => (13.0, FontWeight::Medium, 1.3, "Geist", (dark_primary, light_primary)),
      T::Note => (11.5, FontWeight::Normal, 1.4, "Geist", (dark_muted, light_muted)),
      T::NoteStrong => (11.5, FontWeight::Medium, 1.4, "Geist", (dark_primary, light_primary)),
      T::Prose => (
        12.0,
        FontWeight::Normal,
        1.45,
        "Geist",
        (dark_secondary, light_secondary),
      ),
      T::Lead => (
        12.5,
        FontWeight::Normal,
        1.5,
        "Geist",
        (dark_secondary, light_secondary),
      ),
      T::ColumnHeading => (10.5, FontWeight::Medium, 1.35, "Geist", (dark_muted, light_muted)),
      T::MonoKey => (11.5, FontWeight::Normal, 1.4, "Geist Mono", (dark_muted, light_muted)),
      T::MonoNote => (11.0, FontWeight::Normal, 1.4, "Geist Mono", (dark_muted, light_muted)),
      T::MonoSmall => (
        10.0,
        FontWeight::Normal,
        1.4,
        "Geist Mono",
        (dark_secondary, light_secondary),
      ),
      T::Initials => (9.0, FontWeight::SemiBold, 1.0, "Geist", (dark_primary, light_primary)),
    })
  };
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    for role in ExtraTypography::ALL {
      let Some((size, weight, line_height, family, (dark, light))) = expected(role) else {
        continue;
      };
      let style = app.theme().typography_style(role.role());
      assert_eq!(
        (style.font_size, style.weight, style.line_height, &*style.font_family),
        (size, weight, line_height, family),
        "{role:?}"
      );
      let color = if mode == Mode::Dark { dark } else { light };
      assert_eq!(style.color, Color::from_hex(color), "{role:?} in {}", mode.name());
    }
    assert_eq!(
      app.theme().typography_style(T::ColumnHeading.role()).letter_spacing,
      0.4,
      "column headings are tracked"
    );
    let no_ligatures = FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]);
    for role in [
      TypographyStyle::Mono,
      T::MonoKey.role(),
      T::MonoNote.role(),
      T::MonoSmall.role(),
    ] {
      assert_eq!(
        app.theme().typography_style(role).font_features,
        no_ligatures,
        "{role:?} shows `--` as typed"
      );
    }
  }
}

/// The kit's gaps, paddings, radii, control heights and icon sizes.
#[test]
fn theme_kit_spacing_radii_and_sizes_follow_the_design() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  for (extra, value) in [
    (ExtraSpacing::Fine, 3.0),
    (ExtraSpacing::Pair, 5.0),
    (ExtraSpacing::TextArea, 7.0),
    (ExtraSpacing::Inset, 14.0),
    (ExtraSpacing::Tabs, 20.0),
    (ExtraSpacing::Roomy, 28.0),
    (ExtraSpacing::Loading, 56.0),
  ] {
    assert_eq!(theme.spacing_value(extra.role()), Dimension::Px(value), "{extra:?}");
  }
  assert_eq!(theme.radius_value(ExtraRadius::Tile.role()), 8.0, "menus and tiles");
  assert_eq!(
    [ControlHeight::Sm, ControlHeight::Md].map(ControlHeight::value),
    [24.0, 28.0],
    "control_height_sm and control_height_md"
  );
  assert_eq!(
    IconSize::ALL.map(IconSize::size),
    [11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0]
  );
  assert_eq!((DISABLED_OPACITY, SHORTCUT_OPACITY), (0.5, 0.6));
}

/// A field's value and placeholder are 12 (`JQkW2`, `X75zC`) and its error 11.5 `danger` (`rCoJS`), in
/// both modes.
#[test]
fn theme_fields_read_at_12_in_both_modes() {
  for (mode, text, muted, danger) in [
    (Mode::Dark, "#EDEDF0", "#8A8A92", "#F0605A"),
    (Mode::Light, "#18181B", "#6B6B74", "#D4342F"),
  ] {
    let app = themed(mode);
    let theme = app.theme();
    let form = theme.form().clone();
    let resolve = |role: lurq::app::theme::FormTextRole| role.resolve(&theme.typography(), &theme.palette());
    let (value, placeholder, error) = (
      resolve(form.input.text.clone()),
      resolve(form.input.placeholder.clone()),
      resolve(form.field.error.clone()),
    );
    assert_eq!(
      (value.font_size, value.color),
      (12.0, Color::from_hex(text)),
      "{}",
      mode.name()
    );
    assert_eq!(
      (placeholder.font_size, placeholder.color),
      (12.0, Color::from_hex(muted))
    );
    assert_eq!((error.font_size, error.color), (11.5, Color::from_hex(danger)));
    assert_eq!(form.input.height, Dimension::Px(28.0));
  }
}

/// The focus ring and the Markdown colors follow the mode the theme switches to.
#[test]
fn theme_focus_ring_and_markdown_follow_the_mode() {
  let app = themed(Mode::Dark);
  let theme = app.theme();
  let ring = theme.shadow_style(ExtraShadow::FocusRing.role());
  assert_eq!(
    ring
      .iter()
      .map(|shadow| (shadow.spread, shadow.inset))
      .collect::<Vec<_>>(),
    [(2.0, true)],
    "2 inside the control's edge, where no parent clips it"
  );
  assert_eq!(
    ring[0].color,
    lurq::node::BackgroundColor::Palette(PaletteColor::BorderFocus),
    "in `border_focus`, as the design draws focus"
  );
  for (mode, code, link) in [(Mode::Dark, "#0E0E10", "#EDEDF0"), (Mode::Light, "#F6F6F8", "#18181B")] {
    theme::apply(theme, mode);
    let markdown = theme.markdown().clone();
    assert_eq!(
      markdown.code_block_box.background,
      Some(Color::from_hex(code)),
      "code on code_bg in {}",
      mode.name()
    );
    assert_eq!(
      markdown.link.text.color,
      Some(Color::from_hex(link)),
      "links in accent_text"
    );
    assert_eq!(markdown.link.text.weight, Some(FontWeight::Medium));
  }
}
