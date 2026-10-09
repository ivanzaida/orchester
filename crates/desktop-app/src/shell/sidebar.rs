//! The sidebar (`Sidebar` `ev8rR`): the project switcher, the project's screens with the Configure
//! section, and All projects, Settings and Project settings at the bottom. Outside a project (All
//! projects, Settings) only the switcher and the bottom group show. The counts beside items and the
//! Running section of the design come with their data; until then they are not shown.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::Navigation,
  project::Projects,
  routes::{ProjectPage, Route, Screen},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, ShellText},
  ui::{
    icon::{Icon, icon, screen_icon},
    nav_item::NavItem,
    pressable::pressable,
  },
};

/// `Sidebar` width.
const SIDEBAR_WIDTH: f32 = 240.0;
/// `Project switcher` height and its `Glyph` tile.
const SWITCHER_HEIGHT: f32 = 40.0;
const SWITCHER_GLYPH_SIZE: f32 = 26.0;

pub struct Sidebar;

impl Component for Sidebar {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // A path that names no route selects nothing; the screen says why it names none.
    let route = Route::parse(&ctx.route_path()).ok();
    let navigation = Navigation::of(ctx);
    let selected = route.as_ref().map(Route::nav_screen);
    let project = route.as_ref().and_then(Route::project);
    let mut sidebar = Column::new()
      .id("sidebar")
      .width(Dimension::Px(SIDEBAR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::horizontal(SpacingSize::Md)
          .top(ExtraSpacing::Snug.role())
          .bottom(ExtraSpacing::Loose.role()),
      )
      .spacing(ExtraSpacing::Tight.role())
      .child(switcher(ctx, project, &navigation));
    let mut bottom = vec![Screen::AllProjects, Screen::Settings];
    if let Some(project) = project {
      let items = Items {
        project: Some(project),
        selected,
        navigation: &navigation,
      };
      sidebar = sidebar
        // `Gap` (`Eo8Zf`): 8 between the switcher and the screens, with the sidebar's gap on each side.
        .child(Column::new().padding(Padding::new().top(SpacingSize::Sm)))
        .child(items.group(ctx, &Screen::PROJECT_MAIN))
        .child(section_label(&ctx.t("shell.nav.configure")))
        .child(items.group(ctx, &Screen::CONFIGURE));
      bottom.push(Screen::ProjectSettings);
    }
    let items = Items {
      project,
      selected,
      navigation: &navigation,
    };
    sidebar.child(Column::new().flex(1.0)).child(items.group(ctx, &bottom))
  }
}

/// What every navigation item of the sidebar needs: the project its screens belong to, the selected
/// screen, and the navigation that goes there.
struct Items<'a> {
  project: Option<ProjectId>,
  selected: Option<Screen>,
  navigation: &'a Navigation,
}

impl Items<'_> {
  fn group(&self, ctx: &mut Ctx, screens: &[Screen]) -> Column {
    screens.iter().fold(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role()),
      |group, &screen| match self.target(screen) {
        Some(route) => group.child(self.item(ctx, screen, route)),
        None => group,
      },
    )
  }

  /// Where the item of `screen` goes: in the project when the screen is one of a project's.
  fn target(&self, screen: Screen) -> Option<Route> {
    match screen {
      Screen::AllProjects => Some(Route::AllProjects),
      Screen::Settings => Some(Route::Settings),
      screen => Some(Route::Project(self.project?, ProjectPage::of(screen)?)),
    }
  }

  fn item(&self, ctx: &mut Ctx, screen: Screen, route: Route) -> Element {
    let label = ctx.t(screen.title_key());
    let navigation = self.navigation.clone();
    NavItem {
      id: format!("nav-{}", screen.name()),
      icon: screen_icon(screen),
      label: &label,
      count: None,
      selected: self.selected == Some(screen),
    }
    .on_click(move || navigation.go(route.clone()))
  }
}

/// A section's label (`Section label` `TuunG`).
fn section_label(label: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::horizontal(SpacingSize::Sm)
        .top(ExtraSpacing::Section.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .child(
      Text::new(label)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}

/// The project switcher (`Project switcher` `i0xDK7`): the project the screen shows, by its name, its
/// folder and its initial in the tile, or that none is open. Projects are chosen in All projects, where
/// it leads. While the project opens, or when no project in the list has its id, it says only that a
/// project is shown; its id is never shown.
fn switcher(ctx: &mut Ctx, project: Option<ProjectId>, navigation: &Navigation) -> Element {
  let known = project.and_then(|project| Projects::of(ctx).state(project));
  let listed = known.as_ref().and_then(|state| {
    Some((
      state.name()?.as_str().to_owned(),
      state.repo_root()?.display().to_string(),
    ))
  });
  let (name, detail, tile) = match (project, listed) {
    (Some(_), Some((name, folder))) => {
      let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
      let tile = Text::new(&initial).variant(ShellText::SwitcherInitial.role()).into();
      (
        name,
        Some(Text::new(&folder).variant(ShellText::SwitcherPath.role())),
        tile,
      )
    }
    (Some(_), None) => (ctx.t("shell.sidebar.project").to_string(), None, folders_glyph()),
    (None, _) => (
      ctx.t("shell.sidebar.noProject").to_string(),
      Some(Text::new(&ctx.t("shell.sidebar.chooseProject")).variant(TypographyStyle::Caption)),
      folders_glyph(),
    ),
  };
  let tile = glyph_tile(tile);
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Hair.role())
    .child(truncated(Text::new(&name).variant(TypographyStyle::Label)))
    .with_children(detail.map(truncated));
  let navigation = navigation.clone();
  pressable(move || navigation.go(Route::AllProjects))
    .id("project-switcher")
    .describe("state", if project.is_some() { "project" } else { "no-project" })
    .describe("a11y_name", name)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SWITCHER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm).left(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Inline.role())
    .rounded(ExtraRadius::Tile.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(tile)
    .child(text)
    .child(icon(Icon::ChevronsUpDown, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

/// One line that ends in an ellipsis when it does not fit.
fn truncated(text: Text) -> Text {
  text
    .nowrap()
    .max_width(Dimension::Pct(100.0))
    .text_overflow(TextOverflow::Elipsis)
}

/// The tile's content while no project's name is known.
fn folders_glyph() -> Element {
  icon(Icon::Folders, IconSize::Base, PaletteColor::TextSecondary).into()
}

/// The square before the project's name (`Glyph` `yJEuT`).
fn glyph_tile(content: Element) -> Row {
  Row::new()
    .width(Dimension::Px(SWITCHER_GLYPH_SIZE))
    .height(Dimension::Px(SWITCHER_GLYPH_SIZE))
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
}
