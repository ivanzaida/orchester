//! Startup screens and switcher theme roles.

use super::*;

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
  check(ShellText::MenuSection.role(), (10.5, 1.35), FontWeight::Medium, "Geist");
  check(ShellText::FolderTitle.role(), (12.5, 1.4), FontWeight::Medium, "Geist");
  check(ShellText::FolderBody.role(), (11.5, 1.45), FontWeight::Normal, "Geist");
  assert_eq!(
    theme.typography_style(ShellText::MenuSection.role()).letter_spacing,
    0.6
  );
  let color = |role: TypographyStyle| theme.typography_style(role).color;
  assert_eq!(color(ShellText::MenuSection.role()), Color::from_hex("#8A8A92"));
  assert_eq!(color(ShellText::FolderTitle.role()), Color::from_hex("#EDEDF0"));
  assert_eq!(color(ShellText::FolderBody.role()), Color::from_hex("#B3B3BA"));
  assert_eq!(
    theme.spacing_value(ShellSpacing::FolderActions.role()),
    Dimension::Px(ShellSpacing::FolderActions.value())
  );
  assert_eq!(ShellSpacing::FolderActions.value(), 22.0);
  assert_eq!(color(ShellText::SwitcherInitial.role()), Color::from_hex("#B3B3BA"));
  assert_eq!(color(ShellText::SwitcherPath.role()), Color::from_hex("#8A8A92"));
  // *Jump to*'s result and count (`KeDEy`, `qiz3y` in `aDYAX`).
  check(ShellText::JumpResult.role(), (12.5, 1.35), FontWeight::Normal, "Geist");
  check(ShellText::JumpCount.role(), (11.5, 1.35), FontWeight::Normal, "Geist");
  assert_eq!(color(ShellText::JumpResult.role()), Color::from_hex("#EDEDF0"));
  assert_eq!(color(ShellText::JumpCount.role()), Color::from_hex("#8A8A92"));
  for (spacing, value) in [
    (StartupSpacing::Block, 20.0),
    (StartupSpacing::Panel, 40.0),
    (StartupSpacing::Page, 28.0),
    (StartupSpacing::PageSide, 56.0),
    (StartupSpacing::Steps, 10.0),
    (StartupSpacing::Row, 10.0),
  ] {
    assert_eq!(spacing.value(), value, "{spacing:?}");
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
  check(
    SettingsText::SectionTitle.role(),
    (13.0, 1.3),
    FontWeight::SemiBold,
    "Geist",
  );
  for (spacing, value) in [
    (SettingsSpacing::PanelTop, 28.0),
    (SettingsSpacing::PanelSide, 40.0),
    (SettingsSpacing::SectionTitle, 10.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}
