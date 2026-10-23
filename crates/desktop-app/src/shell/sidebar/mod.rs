//! The sidebar (`Sidebar` `ev8rR`): the project switcher, the project's screens with the Configure
//! section, the runs under way (`running`), and All projects, Settings and Project settings at the
//! bottom. Tasks shows how many tasks the project has, a dash while they are read, and Memory how many
//! records wait for approval. The project is the tab's (`Tab::sidebar_project`): on the app's Settings the
//! one the tab was in last, with Settings selected; a tab that has been in no project shows only the
//! switcher and the bottom group. All projects and Add project have no sidebar.
//!
//! The design draws the sidebar only where everything fits. In a window too short for it, the switcher
//! stays at the top and All projects, Settings and Project settings at the bottom, and what lies between
//! (the screens and the runs under way) scrolls, the thumb in the sidebar's right padding, with a hairline
//! at an edge where it cuts rows (`edges`).

mod edges;
pub mod running;
pub mod switcher;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;

use self::{
  edges::Watch,
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

pub struct Sidebar {
  edges: Watch,
}

impl Component for Sidebar {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self { edges: Watch::new(ctx) }
  }

  fn after_layout(&self) {
    self.edges.after_layout();
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // A path that names no route selects nothing; the screen says why it names none.
    let route = Route::parse(&ctx.route_path()).ok();
    let navigation = Navigation::of(ctx);
    let selected = route.as_ref().map(Route::nav_screen);
    let project = navigation.tabs().active().sidebar_project();
    let switcher = ctx.mount_keyed::<Switcher>("project-switcher", SwitcherProps { project });
    let edges = self.edges.edges();
    let mut screens = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::horizontal(SpacingSize::Md))
      .spacing(ExtraSpacing::Tight.role());
    let mut bottom = vec![Screen::AllProjects, Screen::Settings];
    if let Some(project) = project {
      let counts = counts(ctx, project);
      let items = Items {
        project: Some(project),
        selected,
        navigation: &navigation,
        counts: &counts,
      };
      screens = screens
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
      counts: &[],
    };
    let bottom = items.group(ctx, &bottom);
    Column::new()
      .id("sidebar")
      .width(Dimension::Px(SIDEBAR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Snug.role())
          .bottom(ExtraSpacing::Loose.role()),
      )
      .spacing(ExtraSpacing::Tight.role())
      .child(pinned(switcher))
      // `Gap` (`Eo8Zf`): 8 between the switcher and the screens, with the sidebar's gap on each side.
      .with_children(project.map(|_| pinned(edges.top())))
      // The room the design leaves empty (`Spacer`) between the screens and the bottom group.
      .child(
        ScrollVertical::new(screens)
          .with_scroll_state(self.edges.scroll())
          .id("sidebar-scroll")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .on_scroll(self.edges.on_scroll()),
      )
      .with_children(self.edges.bottom(edges).map(pinned))
      .child(pinned(bottom))
  }
}

/// `child` inside the sidebar's side padding, outside its scrolling part.
fn pinned(child: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Md))
    .child(child)
}

/// The counts beside the project's screens: how many tasks `project` has, a dash while they are read,
/// how many runs are in its merge queue (`Nav · Merge queue`) and how many memory records wait for approval
/// (`Nav · Memory` in `q9mf0k`). A count of none is left out, and so are all when the tasks cannot be read or the
/// project is not open.
fn counts(ctx: &mut Ctx, project: ProjectId) -> Vec<(Screen, String)> {
  let projects = Projects::of(ctx);
  if projects.state(project).is_none_or(|state| state.open().is_none()) {
    return Vec::new();
  }
  match projects.feed(project).tasks.get() {
    TasksState::Ready(tasks) => [
      // A project without tasks shows no count (`a5CcQx`).
      (Screen::Tasks, tasks.tasks.len()),
      (
        Screen::MergeQueue,
        tasks.merge_queue.iter().filter(|entry| entry.state.in_queue()).count(),
      ),
      (Screen::Memory, tasks.proposed_memory.len()),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(screen, count)| (screen, count.to_string()))
    .collect(),
    TasksState::Loading => vec![(Screen::Tasks, ctx.t("shell.sidebar.unknownCount").to_string())],
    TasksState::Failed(_) => Vec::new(),
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
/// screen, the navigation that goes there, and the counts beside the screens that show one.
struct Items<'a> {
  project: Option<ProjectId>,
  selected: Option<Screen>,
  navigation: &'a Navigation,
  counts: &'a [(Screen, String)],
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
    NavItem {
      id: format!("nav-{}", screen.name()),
      icon: screen_icon(screen),
      label: &label,
      count: self
        .counts
        .iter()
        .find(|(counted, _)| *counted == screen)
        .map(|(_, count)| count.as_str()),
      selected: self.selected == Some(screen),
    }
    .opens(self.navigation.to(route))
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
