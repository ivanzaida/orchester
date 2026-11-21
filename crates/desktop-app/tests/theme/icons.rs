//! The theme's icon set: lurq's components draw the app's lucide glyphs by their design names and lurq's.

use lurq::{app::App, ui::icons};
use orchester_app::{
  theme::{self, Mode},
  ui::icon::Icon,
};

#[test]
fn theme_icons_draw_every_app_icon_by_its_design_name() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for icon in Icon::ALL {
    let glyph = theme
      .icon(icon.name())
      .unwrap_or_else(|| panic!("{} is in the set", icon.name()));
    assert_eq!(
      (&*glyph.family, glyph.glyph),
      ("lucide", icon.glyph()),
      "{}",
      icon.name()
    );
  }
  assert_eq!(Icon::ChartColumn.name(), "chart-column");
  assert_eq!(Icon::Trash2.name(), "trash-2");
  assert_eq!(Icon::ArrowUpToLine.name(), "arrow-up-to-line");
}

#[test]
fn theme_icons_give_lurqs_names_the_apps_glyphs_and_keep_its_others() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let close = theme.icon("close").expect("lurq's close");
  assert_eq!((&*close.family, close.glyph), ("lucide", Icon::X.glyph()));
  let airplane = theme.icon("airplane").expect("lurq's own icon");
  assert_eq!(&*airplane.family, icons::FAMILY);
}
