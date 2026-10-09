//! The app menu (`App menu / Orchester` `jksEw`): the mark and a chevron open it below. A custom title
//! bar has no native menu, so the window's own commands live here. The design draws the button only;
//! the menu is built from the shared `Menu item` and `Menu separator` and the raised surfaces of menus.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
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
    menu_item::{MenuItem, separator},
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
  /// The menu's rows; a window without tabs (a startup screen) offers Quit only.
  fn menu(&self, ctx: &mut Ctx) -> Column {
    let navigation = ctx.use_context::<Navigation>();
    let window = ctx.window();
    let entries = ENTRIES
      .iter()
      .filter(|entry| navigation.is_some() || matches!(entry, Some((_, _, _, Command::Quit))));
    entries.fold(
      Column::new()
        .id("app-menu-list")
        .width(Dimension::Px(MENU_WIDTH))
        .padding(SpacingSize::Xs)
        .spacing(ExtraSpacing::Hair.role())
        .rounded(RadiusSize::Lg)
        .background(PaletteColor::SurfaceRaised)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .box_shadow(ShadowStyle::Md),
      |menu, entry| {
        let Some((id, glyph, label_key, command)) = *entry else {
          return menu.child(separator(ctx));
        };
        let label = ctx.t(label_key);
        let (open, navigation, window) = (self.open.clone(), navigation.clone(), window.clone());
        menu.child(
          MenuItem {
            id,
            icon: glyph,
            label: &label,
          }
          .on_click(move || {
            open.set(false);
            match (command, &navigation) {
              (Command::NewTab, Some(navigation)) => navigation.open_tab(),
              (Command::AllProjects, Some(navigation)) => navigation.go(Route::AllProjects),
              (Command::Settings, Some(navigation)) => navigation.go(Route::Settings),
              (Command::Quit, _) => window.request_close(),
              // Only Quit is listed without tabs.
              (Command::NewTab | Command::AllProjects | Command::Settings, None) => {}
            }
          }),
        )
      },
    )
  }
}
