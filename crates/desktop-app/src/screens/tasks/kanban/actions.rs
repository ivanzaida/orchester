//! The board's state that its columns and cards share, which the board provides to them: the drag under way and where
//! it is over, the cards selected and the one the keys reach, a column's refusal of a drop, what a drop asks in place
//! before it changes anything, the last change that did not happen, and the action that makes the changes a drop or a
//! menu asks for (`What it adds` `bNMv9`). Closing a task goes through the screen's closer (`task_close`), whose
//! confirmation stops a run that has not ended.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
  },
  core::{ElementRef, Signal},
};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, RunId, TagId, TaskId},
  task::organization::{ChangeActor, Priority, WorkflowStatus},
};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{moves::write, run_again::request_messages},
  workspace::OpenProject,
};
use parking_lot::Mutex;

use super::{
  bulk::{BulkAction, bulk_action},
  card::CardModel,
  display::Display,
  model::ColumnKey,
};
use crate::{project::ProjectFeed, screens::task_close::Subject, services::Services};

/// Where a drop goes: a column, in a row of the board when it has rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
  pub column: ColumnKey,
  pub lane: Option<String>,
}

/// A card a drag lifted.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
  pub card: CardModel,
  pub from: Target,
  /// Where in the card the pointer took it, and the card's size.
  pub grab: (f32, f32),
  pub size: (f32, f32),
  /// Where it would go now, if over a column.
  pub over: Option<Target>,
}

impl DevtoolsInspectable for Drag {}

/// Why a column turns a task away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// A run of it has not ended: only Done and Canceled, which stop it after a confirmation, take it.
  RunNotEnded,
  /// The draft misses details: the readiness words say which.
  Incomplete(Arc<str>),
  /// It has a run, so it does not go back to the drafts.
  HasRun,
  /// Columns by pipeline only show: a pipeline is chosen on the task's page.
  Pipeline,
}

/// A column's refusal of a task, shown at the column's top while the drag is over it and after the drop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
  pub column: ColumnKey,
  pub key: Arc<str>,
  pub refusal: Refusal,
}

impl DevtoolsInspectable for Refused {}

/// What a drop or a menu asks in place, over the columns, before anything changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
  /// Queuing the draft starts it at once: `free` agent slots are free and new starts are not held.
  StartsNow { task: Subject, free: usize, pipeline: Arc<str> },
  /// The task whose latest run `after` ended runs again.
  RunAgain { task: Subject, after: RunId, pipeline: Arc<str> },
  /// Done closes the task whose work waits to be merged without merging it; its branch stays.
  Unmerged(Subject),
  /// Closing the selected `tasks` stops `runs` of them first, which have not ended.
  BulkClose {
    closing: orchester_workspace::tasks::close::Closing,
    tasks: Vec<Subject>,
    runs: Vec<(Subject, RunId)>,
  },
  /// Deleting the selected tasks.
  BulkDelete(Vec<Subject>),
}

impl DevtoolsInspectable for Ask {}

/// One change of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
  pub task: TaskId,
  pub key: Arc<str>,
  pub kind: ChangeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
  Status(WorkflowStatus),
  Queue,
  ReturnToDraft,
  /// Runs the task again after its latest run `after`, of the pipeline named `pipeline`.
  RunAgain {
    after: RunId,
    pipeline: Arc<str>,
  },
  Priority(Priority),
  Module(Option<ModuleId>),
  Tags(Vec<TagId>),
}

/// A change that did not happen: the task's key and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
  pub key: Arc<str>,
  pub message: Message,
}

impl DevtoolsInspectable for Failed {}

/// The cards selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selected(pub BTreeSet<TaskId>);

impl DevtoolsInspectable for Selected {}

/// The card the keys reach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focused(pub Option<TaskId>);

impl DevtoolsInspectable for Focused {}

/// A part of a card's menu (`Card menu` `vV3BX`): its commands, or the list one of them opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuPage {
  #[default]
  Commands,
  MoveTo,
  Priority,
  Tags,
}

/// The card whose menu is open, and which part of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenMenu(pub Option<(TaskId, MenuPage)>);

impl DevtoolsInspectable for OpenMenu {}

pub type ChangeAction = FutureAction<Vec<Change>, (), Failed>;

/// What the board's parts share.
#[derive(Clone)]
pub struct BoardState {
  /// How the project's tasks are shown, kept for the project (the screen keeps it).
  pub display: Signal<Display>,
  pub drag: Signal<Option<Drag>>,
  /// Where the pointer is while a card is lifted, in the window.
  pub pointer: Signal<(f32, f32)>,
  pub selected: Signal<Selected>,
  pub focused: Signal<Focused>,
  pub menu: Signal<OpenMenu>,
  /// Backlog's quick add is open.
  pub quick_open: Signal<bool>,
  pub refused: Signal<Option<Refused>>,
  pub asked: Signal<Option<Ask>>,
  /// A task was canceled from the board: the Canceled column shows folded with its count.
  pub canceled_now: Signal<bool>,
  /// The last change that did not happen, until the next one or a dismissal.
  pub failed: Signal<Option<Failed>>,
  pub changes: ChangeAction,
  /// What the selection bar asks of several tasks at once.
  pub bulk: BulkAction,
  /// The drop targets as laid out: the zones over a column first, then the columns.
  targets: Arc<Mutex<Vec<(Target, ElementRef)>>>,
  /// The board's own element, for where a lifted card is drawn.
  pub frame: ElementRef,
  /// A drag that Esc set down: it ends without a drop.
  pub set_down: Arc<Mutex<bool>>,
}

impl BoardState {
  pub fn new(ctx: &mut Ctx, (project, feed): (OpenProject, ProjectFeed), display: Signal<Display>) -> Self {
    let (failed, asked, selected) = (ctx.signal(None), ctx.signal(None), ctx.signal(Selected::default()));
    let changes = change_action(ctx, (project.clone(), feed.clone()), failed.clone());
    let bulk = bulk_action(ctx, (project, feed), (asked.clone(), failed.clone(), selected.clone()));
    Self {
      display,
      drag: ctx.signal(None),
      pointer: ctx.signal((0.0, 0.0)),
      selected,
      focused: ctx.signal(Focused::default()),
      menu: ctx.signal(OpenMenu::default()),
      quick_open: ctx.signal(false),
      refused: ctx.signal(None),
      asked,
      canceled_now: ctx.signal(false),
      failed,
      changes,
      bulk,
      targets: Arc::default(),
      frame: ElementRef::default(),
      set_down: Arc::default(),
    }
  }

  /// Whether a change is under way; a render that reads it follows it.
  pub fn busy(&self) -> bool {
    self.changes.state().get().is_pending()
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the board provides its state before mounting its cards")
  }

  /// Makes `changes` one after another, unless a change is under way.
  pub fn change(&self, changes: Vec<Change>) {
    if changes.is_empty() || self.changes.state().get_untracked().is_pending() {
      return;
    }
    self.failed.set(None);
    self.changes.run(changes);
  }

  /// The drop targets as this render lays them out, from scratch.
  pub fn clear_targets(&self) {
    self.targets.lock().clear();
  }

  /// The element of `target`, made the first time it is asked for in a render.
  pub fn target_ref(&self, target: Target) -> ElementRef {
    let mut targets = self.targets.lock();
    if let Some((_, element)) = targets.iter().find(|(held, _)| *held == target) {
      return element.clone();
    }
    let element = ElementRef::default();
    targets.push((target, element.clone()));
    element
  }

  /// The target the window point (`x`, `y`) is over: a zone before the column it is in.
  pub fn target_at(&self, (x, y): (f32, f32)) -> Option<Target> {
    self
      .targets
      .lock()
      .iter()
      .find(|(_, element)| {
        let bounds = element.bounds();
        element.is_attached()
          && x >= bounds.x
          && x <= bounds.x + bounds.width
          && y >= bounds.y
          && y <= bounds.y + bounds.height
      })
      .map(|(target, _)| target.clone())
  }
}

/// Who asks: a person, or automation driving the window through its MCP server.
pub fn actor_now() -> ChangeActor {
  if is_synthetic_input() {
    ChangeActor::Orchester
  } else {
    ChangeActor::Person
  }
}

/// Makes the changes asked for in turn, the first that does not happen ending the rest with why, and has the tasks
/// read again either way.
fn change_action(ctx: &mut Ctx, (project, feed): (OpenProject, ProjectFeed), failed: Signal<Option<Failed>>) -> ChangeAction {
  let services = Services::of(ctx);
  ctx.future_action(move |changes: Vec<Change>| {
    let (project, feed, services, failed) = (project.clone(), feed.clone(), services.clone(), failed.clone());
    let actor = actor_now();
    async move {
      let mut done = Ok(());
      for change in changes {
        if let Err(message) = make(&project, &services, &change, actor).await {
          done = Err(Failed {
            key: change.key.clone(),
            message,
          });
          break;
        }
      }
      feed.saved();
      if let Err(failure) = &done {
        failed.set(Some(failure.clone()));
      }
      done
    }
  })
}

/// Makes one change of a task of `project`.
async fn make(project: &OpenProject, services: &Services, change: &Change, actor: ChangeActor) -> Result<(), Message> {
  let (root, servers, task) = (project.repo_root.clone(), services.servers.clone(), change.task);
  let blocking = match change.kind.clone() {
    ChangeKind::RunAgain { after, pipeline } => return run_again(project, services, (task, after), &pipeline).await,
    ChangeKind::Status(status) => {
      background::run(move || write::set_status_in(&root, &servers, task, (status, actor))).await
    }
    ChangeKind::Queue => background::run(move || write::queue_draft_in(&root, &servers, task, actor)).await,
    ChangeKind::ReturnToDraft => {
      background::run(move || write::return_to_draft_in(&root, &servers, task, actor)).await
    }
    ChangeKind::Priority(priority) => {
      background::run(move || {
        write::organize_in(&root, &servers, (task, actor), |value| value.with_priority(priority))
      })
      .await
    }
    ChangeKind::Module(module) => {
      background::run(move || write::set_module_in(&root, &servers, (task, actor), module)).await
    }
    ChangeKind::Tags(tags) => {
      background::run(move || write::organize_in(&root, &servers, (task, actor), |value| value.with_tags(tags))).await
    }
  };
  blocking.and_then(|done| done)
}

/// Runs `task` again after its latest run `after`, under a shared task's claim; the runner checks Run again's rule and
/// says why it refuses.
async fn run_again(
  project: &OpenProject,
  services: &Services,
  (task, after): (TaskId, RunId),
  pipeline: &str,
) -> Result<(), Message> {
  let claim = services
    .servers
    .claim_for_work(&project.repo_root, task)
    .await
    .map_err(|refusal| refusal.message())?;
  match services
    .runners
    .run_again_in(project, IdempotencyKey::generate(), (task, after, None))
    .await
  {
    Ok(_) => Ok(()),
    Err(error) => {
      claim.release(&services.servers).await;
      Err(
        request_messages(&error, pipeline)
          .into_iter()
          .next()
          .unwrap_or_else(orchester_workspace::messages::interrupted),
      )
    }
  }
}
