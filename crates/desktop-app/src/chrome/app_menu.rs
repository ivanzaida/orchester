//! The app menu (`App menu / Orchester` `jksEw`): the mark and a chevron open it below. A custom title
//! bar has no native menu, so the window's own commands live here. The design draws the button only;
//! the menu is the shared menu (`ui::menu`).

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Placement, Popup, Row},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  brand::mark,
  navigation::Navigation,
  routes::{Route, Screen},
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel, separator},
    pressable::pressable,
  },
};

/// The button (`App menu / Orchester`).
const BUTTON_HEIGHT: f32 = 28.0;
/// The menu, as wide as a `Menu item` with the menu's padding.
const MENU_WIDTH: f32 = 208.0;

/// What a menu row does.
#[derive(Clone, Copy)]
enum Command {
  NewTab,
  AllProjects,
  Settings,
  Quit,
}

/// The rows of the menu: id, icon, i18n key, command; `None` is a separator.
const ENTRIES: [Option<(&str, Icon, &str, Command)>; 5] = [
  Some(("menu-new-tab", Icon::Plus, "common.newTab", Command::NewTab)),
  Some((
    "menu-all-projects",
    Icon::Folders,
    Screen::AllProjects.title_key(),
    Command::AllProjects,
  )),
  Some((
    "menu-settings",
    Icon::Settings,
    Screen::Settings.title_key(),
    Command::Settings,
  )),
  None,
  Some(("menu-quit", Icon::LogOut, "shell.menu.quit", Command::Quit)),
];

pub struct AppMenu {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for AppMenu {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = self.open.clone();
    let mut button = pressable(move || open.update(|open| *open = !*open))
      .id("app-menu")
      .describe("a11y_name", ctx.t("shell.menu.label"))
      .describe("state", if self.open.get() { "open" } else { "closed" })
      .ref_element(self.anchor.clone())
      .height(Dimension::Px(BUTTON_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()).left(SpacingSize::Xs))
      .spacing(SpacingSize::Xs)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(mark())
      .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted));
    if self.open.get() {
      button = button.background(ExtraColor::Hover.role());
    }
    let menu = Popup::new(self.anchor.clone(), self.menu(ctx))
      .open(self.open.clone())
      .placement(Placement::BottomStart)
      .offset(0.0, ExtraSpacing::MenuOffset.value());
    Row::new().child(button).child(menu)
  }
}

impl AppMenu {
  fn menu(&self, ctx: &mut Ctx) -> Column {
    let navigation = Navigation::of(ctx);
    let window = ctx.window();
    ENTRIES.iter().fold(
      menu_panel("app-menu-list").width(Dimension::Px(MENU_WIDTH)),
      |menu, entry| {
        let Some((id, glyph, label_key, command)) = *entry else {
          return menu.child(separator(ctx));
        };
        let label = ctx.t(label_key);
        let (navigation, window) = (navigation.clone(), window.clone());
        let item = MenuItem::new(id, label, move || match command {
          Command::NewTab => navigation.open_tab(),
          Command::AllProjects => navigation.go(Route::AllProjects),
          Command::Settings => navigation.go(Route::Settings),
          Command::Quit => window.request_close(),
        });
        menu.child(item.icon(glyph).render(ctx, &self.open))
      },
    )
  }
}
