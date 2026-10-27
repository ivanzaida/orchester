//! *More actions* (`More` `lDoAj` in `j8DakG`, its menu `JapSp`): every action of the task the header does not show
//! as its primary one. First the task's own: *Edit task* (E), *Copy link*, *Open worktree in Files* of the latest run
//! and *Memory for this task*; then the run's: Pause run, Run again and *Return to a step* while they apply; then
//! closing the task (*Mark as done*, *Cancel task* or *Reopen task*, `task_close`); and after a line what removes:
//! *Remove worktree* of the latest run, *Cancel run* and *Delete task…*, the last two in `danger`. An action that
//! applies but cannot be used now stays, unavailable with why.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{pull_request::task_link, run::RunStatus};
use orchester_workspace::{files::Version, tasks::return_work};

use super::controls::{Control, Offer};
use crate::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
    press::OpenIn,
  },
  routes::{FilesView, ProjectPage, Route, TaskTab, TaskView},
  screens::{
    task::{PageState, RemovalPlace, TaskContext, summary},
    task_close::{self, Closer},
  },
  ui::{
    button::ButtonKind,
    clipboard::Clipboard,
    icon::Icon,
    icon_button::IconButton,
    menu::{menu_at_end, menu_panel, separator},
    pressable::Press,
  },
};

/// The menu's width (`JapSp`).
const MENU_WIDTH: f32 = 240.0;

/// The keys of *Edit task*'s shortcut, as the menu shows them.
pub const EDIT_KEY: &str = "E";

/// The actions the menu holds besides the task's own, as the header found them.
pub struct Held {
  /// Run controls the header does not show: Pause run and Cancel run, and the one that moves the run on while a
  /// card's action is the primary one.
  pub controls: Vec<Control>,
  pub run_again: Option<Control>,
  /// Closing and deleting the task; none for a task kept *Not shared*.
  pub closing: Option<(Closer, task_close::Subject)>,
}

/// Where the menu is open.
#[derive(Clone)]
pub struct MenuPlace {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

impl MenuPlace {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
}

/// The `…` button with its menu while it is open.
pub fn more(ctx: &mut Ctx, task: &TaskContext, place: &MenuPlace, held: Held) -> Element {
  let open = place.open.clone();
  let label = ctx.t("task.more.label");
  let button = IconButton::new("task-more", Icon::Ellipsis, label)
    .element(place.anchor.clone())
    .on_click(move || open.update(|open| *open = !*open));
  let shown = place.open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let panel = panel(ctx, task, &place.open, held);
    menu_at_end(place.anchor.clone(), place.open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}

fn panel(ctx: &mut Ctx, task: &TaskContext, open: &Signal<bool>, held: Held) -> Element {
  let own = own_actions(ctx, task);
  let run: Vec<Control> = held
    .controls
    .iter()
    .filter(|control| !is_cancel(control))
    .cloned()
    .chain(held.run_again)
    .chain(return_to_step(ctx, task))
    .collect();
  let removing: Vec<Control> = remove_worktree(ctx, task)
    .into_iter()
    .chain(held.controls.into_iter().filter(is_cancel))
    .collect();
  let mut rows: Vec<Element> = own.iter().map(|control| control.item(ctx, open)).collect();
  if !run.is_empty() {
    rows.push(separator(ctx));
    rows.extend(run.iter().map(|control| control.item(ctx, open)));
  }
  let (closing, delete) = match &held.closing {
    Some((closer, subject)) => {
      let (rows, delete) = closer.rows(ctx, ("task-more", subject), open);
      (rows, Some(delete))
    }
    None => (Vec::new(), None),
  };
  if !closing.is_empty() {
    rows.push(separator(ctx));
    rows.extend(closing);
  }
  if !removing.is_empty() || delete.is_some() {
    rows.push(separator(ctx));
    rows.extend(removing.iter().map(|control| control.item(ctx, open)));
    rows.extend(delete);
  }
  menu_panel("task-more-panel")
    .width(Dimension::Px(MENU_WIDTH))
    .with_children(rows)
    .into()
}

fn is_cancel(control: &Control) -> bool {
  &*control.id == "task-cancel-run"
}

/// *Edit task* while the task can be edited (locked, with why, while it is kept *Not shared*), *Copy link*, *Open
/// worktree in Files* while the latest run has a worktree, and *Memory for this task*.
fn own_actions(ctx: &mut Ctx, task: &TaskContext) -> Vec<Control> {
  let navigation = Navigation::of(ctx);
  let project = task.project.open.project.id();
  let mut own: Vec<Control> = edit(ctx, task).into_iter().collect();
  let link = task_link(project, task.detail.task.key());
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  own.push(Control::new(
    ctx,
    ("task-copy-link", "task.more.copyLink", Icon::Link),
    ButtonKind::Ghost,
    Offer::Ready(Press::run(move || {
      if !clipboard.copy(&link) {
        tracing::warn!("the task's link could not be put on the clipboard");
      }
    })),
  ));
  own.extend(open_worktree(ctx, task));
  let memory = Route::Project(project, task.page(TaskView::Memory));
  own.push(Control::new(
    ctx,
    ("task-memory", "task.more.memory", Icon::Brain),
    ButtonKind::Ghost,
    Offer::Ready(Press::Open(navigation.to(memory))),
  ));
  own
}

fn edit(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let look = ("task-edit", "task.edit", Icon::Pencil);
  if task.detail.not_shared() {
    let key = task.detail.task.key().to_string();
    let reason = super::controls::Reason {
      short: ctx.t("task.notShared.badge"),
      full: ctx.t_args("task.notShared.locked", [("task", key)]),
    };
    return Some(Control::new(ctx, look, ButtonKind::Ghost, Offer::Unavailable(reason)));
  }
  if !task.detail.editable() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::EditTask(task.detail.task.key().clone()),
  );
  let mut edit = Control::new(
    ctx,
    look,
    ButtonKind::Ghost,
    Offer::Ready(Press::Open(navigation.to(route))),
  );
  edit.shortcut = Some(Arc::from(EDIT_KEY));
  Some(edit)
}

/// *Open worktree in Files*: the code viewer at the latest run's worktree, while it has one; unavailable once it was
/// removed.
fn open_worktree(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let run = task.detail.run.as_ref()?;
  run.run.current_attempt()?;
  let look = ("task-open-worktree", "task.more.openWorktree", Icon::FolderTree);
  if run.worktree_removed.is_some() {
    let reason = super::controls::Reason {
      short: ctx.t("task.reasons.worktreeRemoved"),
      full: ctx.t("task.more.worktreeRemoved"),
    };
    return Some(Control::new(ctx, look, ButtonKind::Ghost, Offer::Unavailable(reason)));
  }
  let navigation = Navigation::of(ctx);
  let files = FilesView::new(None, Version::Run(run.id()));
  let route = Route::Project(task.project.open.project.id(), ProjectPage::Files(files));
  Some(Control::new(
    ctx,
    look,
    ButtonKind::Ghost,
    Offer::Ready(Press::Open(navigation.to(route))),
  ))
}

/// *Remove worktree* of the latest run, once it has no live work and still has worktrees: it opens the confirmation
/// under the header. Older runs have theirs in their rows' menus.
fn remove_worktree(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let detail = &task.detail;
  let run = detail.run.as_ref()?;
  let record = detail.history.iter().find(|record| record.id == run.id())?;
  record.without_live_work()?;
  let left = task.left.of(run.id())?.ok()?;
  if left.worktrees.is_empty() {
    return None;
  }
  let look = ("task-remove-worktree", "task.removeWorktree", Icon::FolderX);
  if task.jobs.run(run.id()).removing() {
    return Some(Control::new(ctx, look, ButtonKind::Ghost, Offer::Pending));
  }
  let state = PageState::of(ctx);
  let busy = detail.history.iter().any(|record| task.jobs.run(record.id).busy());
  // Busy with another run, or its confirmation is open already.
  if busy || state.removal.get().run == Some(run.id()) {
    return Some(Control::new(ctx, look, ButtonKind::Ghost, Offer::Disabled));
  }
  let job = task.project.feed.jobs.of(run.id());
  let id = run.id();
  let press = Press::run(move || {
    job.clear_failed_removal();
    state.open_removal(id, RemovalPlace::Header);
  });
  Some(Control::new(ctx, look, ButtonKind::Ghost, Offer::Ready(press)))
}

/// *Return to a step* of the latest run, while its work can go back to one of its agent steps: a run that ended and
/// was not merged, or one that waits for the person's decision. It opens the Summary at the card that returns it,
/// which it brings into view and rings: *Return the work to an earlier step*, or the decision of a waiting run. The
/// card is where the person picks the step and writes the note, and says when it cannot be returned now.
fn return_to_step(ctx: &mut Ctx, task: &TaskContext) -> Option<Control> {
  let detail = &task.detail;
  let run = detail.run.as_ref()?;
  if detail.latest_unreadable.is_some() || detail.not_shared() || return_work::targets(run).is_empty() {
    return None;
  }
  let card = match run.run.status() {
    RunStatus::WaitingForPerson(_) => PointedCard::Retries,
    _ => {
      summary::returnable(task)?;
      PointedCard::Return
    }
  };
  let project = task.project.open.project.id();
  let route = Route::Project(project, task.page(TaskView::of(TaskTab::Summary)));
  let pointed = Pointed {
    project,
    task: task.key.clone(),
    card,
  };
  let navigation = Navigation::of(ctx);
  let press = Press::run(move || navigation.open_to_card(&route, OpenIn::ActiveTab, pointed.clone()));
  Some(Control::new(
    ctx,
    ("task-return-to-step", "task.returnToStep", Icon::CornerDownLeft),
    ButtonKind::Ghost,
    Offer::Ready(press),
  ))
}
