//! The app menu (`App menu / Orchester` `jksEw`): the mark and a chevron open it below. A custom title
//! bar has no native menu, so the window's own commands live here. The design draws the button only;
//! the menu is the shared menu (`ui::menu`). *Browser* opens the Browser window on the project of the tab shown, else
//! on a project whose browser has tabs (`docs/design/browser.md`, "The *Browser* window").

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
  browser::BrowserLink,
  navigation::Navigation,
  project::Projects,
  routes::{Route, Screen},
  services::Services,
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel, separator},
    pressable::{in_title_bar, pressable},
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
  Browser,
  Quit,
}

/// The rows of the menu: id, icon, i18n key, command; `None` is a separator.
const ENTRIES: [Option<(&str, Icon, &str, Command)>; 6] = [
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
  Some(("menu-browser", Icon::Globe, "shell.menu.browser", Command::Browser)),
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
    let mut button = in_title_bar(pressable(move || open.update(|open| *open = !*open)))
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
      menu_panel("app-menu-list").width(Dimension::Px(MENU_WIDTH)),
      |menu, entry| {
        let Some((id, glyph, label_key, command)) = *entry else {
          return menu.child(separator(ctx));
        };
        let label = ctx.t(label_key);
        let (navigation, window) = (navigation.clone(), window.clone());
        // All projects and Settings are places, opened by a browser's rules; New tab, Browser and Quit are actions.
        let item = match (command, navigation) {
          (Command::AllProjects, Some(navigation)) => MenuItem::opens(id, label, navigation.to(Route::AllProjects)),
          (Command::Settings, Some(navigation)) => MenuItem::opens(id, label, navigation.to(Route::Settings)),
          (Command::NewTab, Some(navigation)) => MenuItem::new(id, label, move || navigation.open_tab()),
          (Command::Browser, Some(navigation)) => browser_item(ctx, (id, label), &navigation),
          (Command::Quit, _) => MenuItem::new(id, label, move || window.request_close()),
          // Only Quit is listed without tabs.
          (Command::NewTab | Command::AllProjects | Command::Settings | Command::Browser, None) => return menu,
        };
        menu.child(item.icon(glyph).render(ctx, &self.open))
      },
    )
  }
}

/// *Browser*: the Browser window on the project of the tab shown, else on the first project whose browser has tabs;
/// unavailable while there is neither.
fn browser_item(ctx: &mut Ctx, (id, label): (&str, std::sync::Arc<str>), navigation: &Navigation) -> MenuItem {
  let shown = navigation
    .route_now()
    .project()
    .and_then(|project| Projects::of(ctx).state(project))
    .and_then(|state| state.repo_root().map(std::path::Path::to_path_buf));
  let project = shown.or_else(|| {
    Services::of(ctx)
      .runners
      .browser_window()
      .projects()
      .into_iter()
      .next()
      .map(|(root, _)| root)
  });
  match (BrowserLink::of(ctx), project) {
    (Some(link), Some(project)) => MenuItem::new(id, label, move || link.show(&project, None)),
    _ => MenuItem::unavailable(id, label, ctx.t("shell.menu.browserNoProject")),
  }
}
