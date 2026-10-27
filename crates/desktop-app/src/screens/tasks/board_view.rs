//! The Tasks screen's board (`kanban`): the view's rows the list reads, as cards, with what the screen knows of the
//! project — whether its runner has a free agent slot and new starts are not held (queuing a draft then starts it, so
//! a drop on Todo asks first), whether it is shared, whether the person is no longer a member (read-only, quick add
//! locked) and whether the *Not shared* filter is on (read-only cards). How the tasks are shown is kept for the project
//! in the application database as it changes.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::{
  runner::state::RunnerState,
  tasks::{
    browse::{self, Facets, TaskSort},
    list::TaskRow,
  },
};

use super::{
  Shown, TasksScreen,
  cache::Reading,
  kanban::{
    self, BoardInput,
    display::{self as kept, Display},
  },
  online,
  views::list_view,
};
use crate::{project::ShownProject, services::Services};

impl TasksScreen {
  /// The board of the view the list is on, over `rows` as searched, filtered and ordered.
  pub(super) fn board(
    &self,
    ctx: &mut Ctx,
    shown: &Shown<'_>,
    (reading, rows): (&Reading, &[TaskRow]),
    filtered: bool,
  ) -> Element {
    let (project, list) = (shown.project, shown.list);
    let view = list_view(list.tab());
    let kept: Vec<TaskRow> = rows
      .iter()
      .filter(|row| view.keeps(row, &reading.waiting))
      .cloned()
      .collect();
    // Every task of the view whatever the search and filters: a column says how many it hides.
    let every: Vec<TaskRow> = if filtered {
      let separator = ctx.t("tasks.listSeparator");
      browse::faceted(&reading.tasks, ("", &separator), &Facets::default(), TaskSort::Manual)
        .into_iter()
        .filter(|row| view.keeps(row, &reading.waiting))
        .collect()
    } else {
      kept.clone()
    };
    let read_only = shown.shared.and_then(|shared| shared.read_only(ctx));
    let (search, facets) = (self.choices.search.clone(), self.choices.facets.clone());
    let highlight = list.highlight().map(ToString::to_string);
    kanban::board(
      ctx,
      BoardInput {
        project,
        view: list.tab(),
        rows: &kept,
        every: &every,
        tasks: &reading.tasks,
        needs: &reading.needs,
        highlight: highlight.as_deref(),
        display: self.display.clone(),
        free: free_slots(project, &reading.tasks),
        shared: shown.shared.is_some() || reading.tasks.shared,
        read_only,
        not_shared: shown.not_shared,
        filtered,
        clear: Arc::new(move || {
          search.set(String::new());
          facets.set(Facets::default());
        }),
      },
    )
  }
}

/// The agent slots free in the project's runner while it serves the project and new starts are not held: queuing a
/// draft then starts it at once. `None` otherwise.
fn free_slots(project: &ShownProject, tasks: &orchester_workspace::tasks::data::ProjectTasks) -> Option<usize> {
  if !online(project) || tasks.queue.project {
    return None;
  }
  let status = project.feed.status.get()?;
  if status.state_for(&project.open.repo_root) != RunnerState::Online {
    return None;
  }
  let work = status.work()?;
  let slots = usize::try_from(work.agent_slots.get()).ok()?;
  Some(slots.saturating_sub(work.agents_working()))
}

/// How `project`'s tasks were last shown, kept as it changes. Reads one row of the application database once, when the
/// screen opens; a change is kept off the UI thread.
pub(super) fn kept_display(ctx: &mut Ctx, project: &ShownProject) -> Display {
  let services = Services::of(ctx);
  let id = project.open.project.id();
  kept::read(&services.workspace, id)
}

/// Keeps `display` for `project` whenever it changes, off the UI thread.
pub(super) fn keep_display(ctx: &mut Ctx, project: &ShownProject, display: &Signal<Display>) {
  let services = Services::of(ctx);
  let id = project.open.project.id();
  ctx.watch(display, move |shown: &Display| {
    let (workspace, shown) = (services.workspace.clone(), shown.clone());
    drop(
      services
        .runtime
        .spawn_blocking(move || kept::keep(&workspace, id, &shown)),
    );
  });
}
