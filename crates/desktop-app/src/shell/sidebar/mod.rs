//! The sidebar (`Sidebar` `ev8rR`): the project switcher, the project's screens with the Configure
//! section, the runs under way (`running`), and All projects, Settings and Project settings at the
//! bottom. Tasks shows how many tasks the project has, a dash while they are read. Outside a project
//! (All projects, Settings) only the switcher and the bottom group show.

pub mod running;
pub mod switcher;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;

use self::{
  running::{Running, RunningProps, under_way},
  switcher::{Switcher, SwitcherProps},
};
use crate::{
  navigation::Navigation,
  project::{ProjectState, Projects, TasksState},
  routes::{ProjectPage, Route, Screen},
  theme::ExtraSpacing,
  ui::{icon::screen_icon, nav_item::NavItem},
};

/// `Sidebar` width.
const SIDEBAR_WIDTH: f32 = 240.0;

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
    let switcher = ctx.mount_keyed::<Switcher>("project-switcher", SwitcherProps { project });
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
      .child(switcher);
    let mut bottom = vec![Screen::AllProjects, Screen::Settings];
    if let Some(project) = project {
      let tasks = tasks_count(ctx, project);
      let items = Items {
        project: Some(project),
        selected,
        navigation: &navigation,
        tasks: tasks.as_deref(),
      };
      sidebar = sidebar
        // `Gap` (`Eo8Zf`): 8 between the switcher and the screens, with the sidebar's gap on each side.
        .child(Column::new().padding(Padding::new().top(SpacingSize::Sm)))
        .child(items.group(ctx, &Screen::PROJECT_MAIN))
        .child(section_label(&ctx.t("shell.nav.configure")))
        .child(items.group(ctx, &Screen::CONFIGURE))
        .with_children(running(ctx, project));
      bottom.push(Screen::ProjectSettings);
    }
    let items = Items {
      project,
      selected,
      navigation: &navigation,
      tasks: None,
    };
    sidebar.child(Column::new().flex(1.0)).child(items.group(ctx, &bottom))
  }
}

/// How many tasks `project` has, a dash while they are read; nothing when they cannot be read or the
/// project is not open.
fn tasks_count(ctx: &mut Ctx, project: ProjectId) -> Option<String> {
  let projects = Projects::of(ctx);
  projects.state(project)?.open()?;
  match projects.feed(project).tasks.get() {
    TasksState::Ready(tasks) => Some(tasks.tasks.len().to_string()),
    TasksState::Loading => Some(ctx.t("shell.sidebar.unknownCount").to_string()),
    TasksState::Failed(_) => None,
  }
}

/// The project's runs under way, once its tasks are read.
fn running(ctx: &mut Ctx, project: ProjectId) -> Option<Element> {
  let projects = Projects::of(ctx);
  let Some(ProjectState::Open(open)) = projects.state(project) else {
    return None;
  };
  let feed = projects.feed(project);
  let online = feed.runner_online(&open.repo_root);
  let runs = feed
    .tasks
    .with(|tasks| tasks.ready().map(|tasks| under_way(tasks, online)))?;
  let props = RunningProps {
    project,
    repo_root: open.repo_root,
    runs,
  };
  Some(ctx.mount_keyed::<Running>("running", props))
}

/// What every navigation item of the sidebar needs: the project its screens belong to, the selected
/// screen, the navigation that goes there, and the count Tasks shows.
struct Items<'a> {
  project: Option<ProjectId>,
  selected: Option<Screen>,
  navigation: &'a Navigation,
  tasks: Option<&'a str>,
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
      count: self.tasks.filter(|_| screen == Screen::Tasks),
      selected: self.selected == Some(screen),
    }
    .on_click(move || navigation.go(route.clone()))
  }
}

/// A section's label (`Section label` `TuunG`); add its action as a child.
fn section_label(label: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::horizontal(SpacingSize::Sm)
        .top(ExtraSpacing::Section.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .align_items(Alignment::Center)
    .child(
      Text::new(label)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0),
    )
}
