//! Closing and deleting a task by hand (ORC-91), from the task page's header and from a Tasks list row, each through
//! its *More actions* menu: *Mark as done* and *Cancel task* close the task whatever its latest run did, *Reopen task*
//! opens a closed one again, and *Delete task…* asks first. A task whose run has not ended asks in place whether to
//! stop that run first (`confirm`), and deleting says in place what goes with the task (`delete`): under the header
//! on the task page, over the rows on the Tasks list, in a popover at the card on the Tasks board (`Popover · stop
//! run` `XdfNz`, which the board draws from [`Closer::stop_asked`]), never in a dialog. The design has no frame for
//! the others yet: they
//! follow Run again's confirmation in place and the Memory page's *Delete this record?* (`YdVse`). A shared project's
//! tasks are its team server's, which keeps no deletion: *Delete task* is unavailable there, with why. Only a person
//! deletes a task: an MCP client's press is refused, and the confirmation says so.

mod confirm;
mod delete;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  task::organization::{ChangeActor, WorkflowStatus},
};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    cancel_run::{Stop, StopRefused},
    close::{Closing, close_in, reopen_in},
    delete::delete_in,
  },
  workspace::OpenProject,
};

pub use self::{
  confirm::{StopLook, failure, panel},
  delete::confirmation_in,
};
use crate::{
  navigation::{Navigation, press::OpenIn},
  project::ProjectFeed,
  routes::Route,
  services::Services,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel, separator},
  },
};

/// The *More actions* menu's width (no frame yet): room for *Delete task…* and, in a shared project, why it is
/// unavailable on a few lines under it.
const MENU_WIDTH: f32 = 260.0;

/// The task a menu or a confirmation is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
  pub id: TaskId,
  pub key: Arc<str>,
  pub status: WorkflowStatus,
  /// A shared project's task, which its team server keeps.
  pub shared: bool,
}

/// What waits in place for the person.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Asked {
  #[default]
  Nothing,
  /// Closing `task` waits until its run, which has not ended, is confirmed stopped.
  StopRun {
    task: Subject,
    closing: Closing,
    run: RunId,
  },
  Delete(Subject),
}

impl DevtoolsInspectable for Asked {}

/// What the person asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
  Close(Subject, Closing, Stop),
  Reopen(Subject),
  /// Delete the task, with its unmerged work when `true`.
  Delete(Subject, bool),
}

impl DevtoolsInspectable for Request {}

impl Request {
  const fn subject(&self) -> &Subject {
    match self {
      Self::Close(subject, ..) | Self::Reopen(subject) | Self::Delete(subject, _) => subject,
    }
  }
}

/// A request and who made it: a press from an MCP client's synthetic input is automation's, and the task's history
/// says so, as `update_task`'s changes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
  request: Request,
  actor: ChangeActor,
}

impl DevtoolsInspectable for Asking {}

/// What the person asked last did not happen, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
  pub task: TaskId,
  /// The task's key, for an error shown away from its row.
  pub key: Arc<str>,
  pub message: Message,
}

impl DevtoolsInspectable for Failed {}

type Action = FutureAction<Asking, Request, Failed>;

/// A screen's closing and deleting of its project's tasks: what waits in place, the request under way and how the
/// last one ended. The screen makes it once and provides it to its parts.
#[derive(Clone)]
pub struct Closer {
  asked: Signal<Asked>,
  /// The delete confirmation's choice to remove unmerged work too; off each time it opens.
  unmerged: Signal<bool>,
  /// An MCP client pressed Delete: refused, which the confirmation says.
  person_only: Signal<bool>,
  action: Action,
  root: PathBuf,
}

impl Closer {
  /// Closes and deletes the tasks of `project`, whose `feed` reads them again after each change; `leave` is where the
  /// screen goes once its task was deleted (the task page, which shows it no more).
  pub fn new(ctx: &mut Ctx, (project, feed): (OpenProject, ProjectFeed), leave: Option<Route>) -> Self {
    let asked = ctx.signal(Asked::Nothing);
    let action = action(ctx, (project.clone(), feed), asked.clone());
    if let Some(route) = leave {
      let navigation = Navigation::of(ctx);
      ctx.watch(&action.state(), move |state| {
        if !state.is_pending() && matches!(state.data, Some(Request::Delete(..))) && state.error.is_none() {
          navigation.open(route.clone(), OpenIn::ActiveTab);
        }
      });
    }
    Self {
      asked,
      unmerged: ctx.signal(false),
      person_only: ctx.signal(false),
      action,
      root: project.repo_root,
    }
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the screen provides its closer before mounting its rows")
  }

  /// Asks for `request`, unless one is under way; a press from an MCP client's synthetic input is automation's.
  fn run(&self, request: Request) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    let actor = if is_synthetic_input() {
      ChangeActor::Orchester
    } else {
      ChangeActor::Person
    };
    self.action.run(Asking { request, actor });
  }

  /// Closes `subject` as `closing` asks: at once, or once its run, which has not ended, is confirmed stopped in place.
  pub fn close(&self, subject: Subject, closing: Closing) {
    self.run(Request::Close(subject, closing, Stop::Unconfirmed));
  }

  /// Opens the closed `subject` again.
  pub fn reopen(&self, subject: Subject) {
    self.run(Request::Reopen(subject));
  }

  /// Whether a request is under way; a render that reads it follows it.
  pub fn busy(&self) -> bool {
    self.action.state().get().is_pending()
  }

  /// The task the last request closed, and how, once it did; a render that reads it follows it.
  pub fn closed(&self) -> Option<(Subject, Closing)> {
    let state = self.action.state().get();
    match (state.is_pending(), state.error, state.data) {
      (false, None, Some(Request::Close(subject, closing, _))) if self.asked.get() == Asked::Nothing => {
        Some((subject, closing))
      }
      _ => None,
    }
  }

  /// The task whose run waits to be confirmed stopped before it closes, and how it closes, for a screen that asks it
  /// in a place of its own (the board, at the card); a render that reads it follows it.
  pub fn stop_asked(&self) -> Option<(Subject, Closing)> {
    match self.asked.get() {
      Asked::StopRun { task, closing, .. } => Some((task, closing)),
      Asked::Nothing | Asked::Delete(_) => None,
    }
  }

  /// The task whose deletion waits for the person's answer; a render that reads it follows it.
  pub fn delete_asked(&self) -> Option<Subject> {
    match self.asked.get() {
      Asked::Delete(task) => Some(task),
      Asked::Nothing | Asked::StopRun { .. } => None,
    }
  }

  /// Why the last request about `task` did not happen, once it ended; a render that reads it follows it.
  pub fn failure_of(&self, task: TaskId) -> Option<Message> {
    let state = self.action.state().get();
    let pending = state.is_pending();
    state
      .error
      .filter(|failed| !pending && failed.task == task)
      .map(|failed| failed.message)
  }

  /// The person confirmed stopping the run that waits: the run stops and the task closes. Answers whether a stop
  /// waited.
  pub fn confirm_stop(&self) -> bool {
    let Asked::StopRun { task, closing, run } = self.asked.get_untracked() else {
      return false;
    };
    self.run(Request::Close(task, closing, Stop::Confirmed(Some(run))));
    true
  }

  /// Keeps what waits in place as it is (the run goes on, the task stays), unless a request is under way.
  pub fn keep(&self) {
    let pending = self.action.state().get_untracked().is_pending();
    if !pending && self.asked.get_untracked() != Asked::Nothing {
      self.asked.set(Asked::Nothing);
    }
  }

  /// Opens the delete confirmation of `subject` in place, its choice off.
  pub fn ask_delete(&self, subject: Subject) {
    self.unmerged.set(false);
    self.person_only.set(false);
    self.asked.set(Asked::Delete(subject));
  }

  /// Deletes `subject`, a person's press only.
  fn delete(&self, subject: Subject) {
    if is_synthetic_input() {
      tracing::warn!("synthetic input asked to delete a task; not taken");
      self.person_only.set(true);
      return;
    }
    self.person_only.set(false);
    self.run(Request::Delete(subject, self.unmerged.get_untracked()));
  }

  /// The *More actions* menu of `subject`, its rows' ids starting with `id`, closed by a choice through `open`.
  fn menu(&self, ctx: &mut Ctx, (id, subject): (&str, &Subject), open: &Signal<bool>) -> Column {
    let (rows, delete) = self.rows(ctx, (id, subject), open);
    menu_panel(&format!("{id}-panel"))
      .width(Dimension::Px(MENU_WIDTH))
      .with_children(rows)
      .child(separator(ctx))
      .child(delete)
  }

  /// The rows of a *More actions* menu that close, reopen and delete `subject`, their ids starting with `id`, closed
  /// by a choice through `open`: those that close or reopen it, and *Delete task…* apart, for the menu's end (the
  /// task page's menu holds them among its own, ORC-100).
  pub fn rows(&self, ctx: &mut Ctx, (id, subject): (&str, &Subject), open: &Signal<bool>) -> (Vec<Element>, Element) {
    let busy = self.action.state().get().is_pending();
    let item = |suffix: &str, (label, glyph): (Arc<str>, Icon), request: Request| {
      let id = format!("{id}-{suffix}");
      if busy {
        return MenuItem::unavailable(id, label, ctx.t("task_close.menu.busy")).icon(glyph);
      }
      let closer = self.clone();
      MenuItem::new(id, label, move || closer.run(request.clone())).icon(glyph)
    };
    let close = |closing| Request::Close(subject.clone(), closing, Stop::Unconfirmed);
    let done = (ctx.t("task_close.menu.done"), Icon::CircleCheck);
    let cancel = (ctx.t("task_close.menu.cancel"), Icon::CircleSlash);
    let items = match subject.status {
      WorkflowStatus::Done => vec![
        item(
          "reopen",
          (ctx.t("task_close.menu.reopen"), Icon::RotateCcw),
          Request::Reopen(subject.clone()),
        ),
        item("cancel", cancel, close(Closing::Canceled)),
      ],
      WorkflowStatus::Canceled => vec![
        item(
          "reopen",
          (ctx.t("task_close.menu.reopen"), Icon::RotateCcw),
          Request::Reopen(subject.clone()),
        ),
        item("done", done, close(Closing::Done)),
      ],
      _ => vec![
        item("done", done, close(Closing::Done)),
        item("cancel", cancel, close(Closing::Canceled)),
      ],
    };
    let delete_label = ctx.t("task_close.menu.delete");
    let delete = if subject.shared {
      MenuItem::unavailable(
        format!("{id}-delete"),
        delete_label,
        ctx.t("task_close.menu.deleteShared"),
      )
    } else {
      let (closer, subject) = (self.clone(), subject.clone());
      MenuItem::new(format!("{id}-delete"), delete_label, move || {
        closer.ask_delete(subject.clone())
      })
    };
    let delete = delete.icon(Icon::Trash2).render(ctx, open);
    let rows: Vec<Element> = items.into_iter().map(|item| item.render(ctx, open)).collect();
    (rows, delete)
  }
}

/// Carries out what the person asked, then has the tasks read again. Closing a task whose run has not ended opens
/// the confirmation that stops it; a request that did not happen says why.
fn action(ctx: &mut Ctx, (project, feed): (OpenProject, ProjectFeed), asked: Signal<Asked>) -> Action {
  let services = Services::of(ctx);
  ctx.future_action(move |Asking { request, actor }: Asking| {
    let (project, feed, asked, services) = (project.clone(), feed.clone(), asked.clone(), services.clone());
    async move {
      let root = project.repo_root.clone();
      let done = match request.clone() {
        Request::Close(subject, closing, stop) => {
          let closed = close_in(
            &project,
            (&services.runners, &services.servers),
            subject.id,
            (closing, stop, actor),
          )
          .await;
          after_close(&asked, (subject, closing), closed)
        }
        Request::Reopen(subject) => {
          let servers = services.servers.clone();
          background::run(move || reopen_in(&root, &servers, subject.id, actor).map_err(|error| error.message()))
            .await
            .and_then(|reopened| reopened)
        }
        Request::Delete(subject, unmerged) => {
          let programs = services.programs.clone();
          let deleted = background::run(move || delete_in(&root, &programs, subject.id, unmerged))
            .await
            .and_then(|deleted| deleted);
          if deleted.is_ok() {
            asked.set(Asked::Nothing);
          }
          deleted
        }
      };
      feed.saved();
      done.map(|()| request.clone()).map_err(|message| {
        let subject = request.subject();
        Failed {
          task: subject.id,
          key: subject.key.clone(),
          message,
        }
      })
    }
  })
}

/// What closing `subject` came to: closed, or waiting for the person to confirm stopping its run, which a run started
/// since a confirmation opened names anew.
fn after_close(
  asked: &Signal<Asked>,
  (subject, closing): (Subject, Closing),
  closed: Result<(), StopRefused>,
) -> Result<(), Message> {
  match closed {
    Ok(()) => {
      asked.set(Asked::Nothing);
      Ok(())
    }
    Err(StopRefused::Unconfirmed(run)) => {
      asked.set(Asked::StopRun {
        task: subject,
        closing,
        run,
      });
      Ok(())
    }
    Err(refused @ StopRefused::StartedSince(run)) => {
      asked.set(Asked::StopRun {
        task: subject,
        closing,
        run,
      });
      Err(refused.message())
    }
    Err(StopRefused::Failed(message)) => Err(message),
  }
}

/// Where a *More actions* menu is open: the component that shows the button keeps it.
#[derive(Clone)]
pub struct MenuPlace {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl MenuPlace {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  /// Whether the menu is open: a row that shows its button under the pointer keeps it while the menu is.
  pub fn is_open(&self) -> bool {
    self.open.get()
  }
}

/// The *More actions* button (`…`) of `subject`, as `id`, with its menu while it is open, at the end of a row or of
/// the task page's actions. The menu is declared only while open: a list shows a button on every row, and every
/// row's menu would weigh on how fast the list is laid out.
pub fn more_menu(ctx: &mut Ctx, (closer, place): (&Closer, &MenuPlace), subject: &Subject, id: &str) -> Element {
  let open = place.open.clone();
  let label = ctx.t_args("task_close.menu.label", [("task", &*subject.key)]);
  let button = IconButton::new(id.to_owned(), Icon::Ellipsis, label)
    .element(place.anchor.clone())
    .on_click(move || open.update(|open| *open = !*open));
  let shown = place.open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let panel = closer.menu(ctx, (id, subject), &place.open);
    menu_at_end(place.anchor.clone(), place.open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}
