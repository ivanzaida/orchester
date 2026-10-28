//! The sidebar (`Sidebar v2` `Zn8dR` in `Sidebar · redesign` `WWXmX`): the project switcher, whose menu holds
//! Project settings, All projects and Settings; *Jump to…* (`shell::jump`); the six places for daily work; the
//! Configure section with Pipelines and Agents, which folds per project (`configure`); the runs under way
//! (`running`), only while there are any; and at the bottom the usage card, which opens Usage and Settings
//! (`usage_card`). Tasks shows how many tasks the project has, a dash while they are read, and Review how many runs
//! wait in the merge queue. The project is the tab's (`Tab::sidebar_project`): on the app's Settings the one the
//! tab was in last; a tab that has been in no project shows only the switcher and *Jump to…*. All projects and Add
//! project have no sidebar.
//!
//! The design draws the sidebar only where everything fits. In a window too short for it, the switcher and *Jump
//! to…* stay at the top and the usage card at the bottom, and what lies between (the places, Configure and the runs
//! under way) scrolls, the thumb in the sidebar's right padding, with a hairline at an edge where it cuts rows
//! (`edges`).

mod configure;
mod edges;
pub mod place;
pub mod running;
pub mod switcher;
mod usage_card;

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
  place::Place,
  running::{Running, RunningProps, under_way},
  switcher::{Switcher, SwitcherProps},
};
use crate::{
  navigation::Navigation,
  project::{ProjectState, Projects, TasksState},
  routes::Route,
  shell::jump,
  theme::ExtraSpacing,
  ui::nav_item::NavItem,
};

/// `Sidebar v2` width.
const SIDEBAR_WIDTH: f32 = 240.0;
/// A section's head (`Configure head` `hbx5J`, `Running head` `veG7t`).
const SECTION_HEAD_HEIGHT: f32 = 24.0;

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
    let selected = route.as_ref().and_then(Place::of);
    let project = navigation.tabs().active().sidebar_project();
    let switcher = ctx.mount_keyed::<Switcher>("project-switcher", SwitcherProps { project });
    let jump = jump::row(ctx, &navigation);
    let edges = self.edges.edges();
    let mut screens = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::horizontal(SpacingSize::Md))
      .spacing(ExtraSpacing::Tight.role());
    let mut bottom = None;
    if let Some(project) = project {
      let counts = counts(ctx, project);
      let items = Items {
        project,
        selected,
        navigation: &navigation,
        counts: &counts,
      };
      let daily = items.group(ctx, &Place::DAILY);
      let configure = configure::section(ctx, &items);
      screens = screens
        .with_children(daily)
        .with_children(configure)
        .with_children(running(ctx, project));
      bottom = Some(usage_card::card(ctx, project, &navigation));
    }
    Column::new()
      .id("sidebar")
      .width(Dimension::Px(SIDEBAR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Snug.role())
          .bottom(SpacingSize::Md),
      )
      .spacing(ExtraSpacing::Tight.role())
      .child(pinned(switcher))
      .child(jump)
      // The places' 8 from *Jump to…* (`Screens` `LxKdr`), with the sidebar's gap on each side.
      .with_children(project.map(|_| pinned(edges.top())))
      // The room the design leaves empty between the places and the usage card.
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
      .with_children(bottom.map(pinned))
  }
}

/// `child` inside the sidebar's side padding, outside its scrolling part.
fn pinned(child: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Md))
    .child(child)
}

/// A count beside a place, and whether it counts what waits for the person.
struct Count {
  place: Place,
  value: String,
  waiting: bool,
}

/// The counts beside the places: how many tasks `project` has, a dash while they are read, and how many runs are in
/// its merge queue beside Review, in `warning`. A count of none is left out, and so are all when the tasks cannot be
/// read or the project is not open.
fn counts(ctx: &mut Ctx, project: ProjectId) -> Vec<Count> {
  let projects = Projects::of(ctx);
  if projects.state(project).is_none_or(|state| state.open().is_none()) {
    return Vec::new();
  }
  match projects.feed(project).tasks.get() {
    TasksState::Ready(tasks) => [
      // A project without tasks shows no count (`a5CcQx`).
      (Place::Tasks, tasks.tasks.len(), false),
      (
        Place::Review,
        orchester_workspace::merge_queue::in_queue(tasks.merge_queue.iter()),
        true,
      ),
    ]
    .into_iter()
    .filter(|(_, count, _)| *count > 0)
    .map(|(place, count, waiting)| Count {
      place,
      value: count.to_string(),
      waiting,
    })
    .collect(),
    TasksState::Loading => vec![Count {
      place: Place::Tasks,
      value: ctx.t("shell.sidebar.unknownCount").to_string(),
      waiting: false,
    }],
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

/// What every entry of the sidebar needs: the project its places belong to, the selected place, the navigation
/// that goes there, and the counts beside the places that show one.
struct Items<'a> {
  project: ProjectId,
  selected: Option<Place>,
  navigation: &'a Navigation,
  counts: &'a [Count],
}

impl Items<'_> {
  fn group(&self, ctx: &mut Ctx, places: &[Place]) -> Vec<Element> {
    places.iter().map(|&place| self.item(ctx, place)).collect()
  }

  fn item(&self, ctx: &mut Ctx, place: Place) -> Element {
    let label = ctx.t(place.title_key());
    let count = self.counts.iter().find(|count| count.place == place);
    NavItem {
      id: format!("nav-{}", place.name()),
      icon: place.icon(),
      label: &label,
      count: count.map(|count| count.value.as_str()),
      waiting: count.is_some_and(|count| count.waiting),
      selected: self.selected == Some(place),
    }
    .opens(self.navigation.to(place.route(self.project)))
  }
}

/// The room before a section (`Section gap` `cYdI6`): 10, with the list's gap on each side.
fn section_gap() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Loose.role()))
}

/// A section's head with its label (`Configure head` `hbx5J`, `Running head` `veG7t`), its parts `spacing` apart;
/// add what follows the label as children.
fn section_head(label: &str, spacing: SpacingSize) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(spacing)
    .align_items(Alignment::Center)
    .child(section_label(label))
}

/// A section's label: 11/500 `text_muted`.
fn section_label(label: &str) -> Text {
  Text::new(label)
    .variant(TypographyStyle::FieldLabel)
    .color(PaletteColor::TextMuted)
    .nowrap()
}
