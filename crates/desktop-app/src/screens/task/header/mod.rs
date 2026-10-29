//! The task page's header (ORC-100, `Task header` `j8DakG`), 20 over it and 28 at its sides, its rows 10 apart: the
//! way back to Tasks (`back`), the title with the one primary action the task's state calls for (`primary`) and
//! *More actions* with every other (`more`), the status line (`run_line`), the task's module (`module`, ORC-48, no
//! frame yet), the frame's notices, Run again's, closing's and deleting's and the latest run's *Remove worktree*
//! confirmations in place, and the page tabs Summary, Conversation and Changes with the count of its files (`Tabs`
//! `QFr7g`). It is the same component on every tab, so a request under way or an open confirmation stays as the tabs
//! switch. E opens Edit task, as *More actions* says, unless a field has the keys.

mod actions;
mod back;
pub(super) mod controls;
mod module;
mod more;
mod notices;
mod primary;
mod refusal;
pub(super) mod run_again;
mod run_line;
mod run_mode;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, events::KeyboardEvent, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{messages::Message, tasks::run_again::RunAgainCheck};

use self::{
  controls::{Controls, RunControls, controls},
  more::{Held, MenuPlace},
  refusal::Refusals,
  run_again::{Confirmation, Confirming, StartAction},
};
use super::{
  PageState, RemovalPlace, TaskContext,
  summary::runs::{Runs, removal::confirm_for},
};
use crate::{
  navigation::{Navigation, link::Open, press::OpenIn},
  routes::{ProjectPage, Route, TaskList, TaskTab, TaskView},
  screens::task_close::{self, Closer, Subject},
  services::Services,
  theme::{ExtraSpacing, TaskSpacing},
  ui::tabs::{PageTab, page_tabs},
};

#[derive(Clone, PartialEq)]
pub struct HeaderProps {
  pub task: TaskContext,
  /// What the page shows: its tab is the one chosen.
  pub view: TaskView,
  /// Why the task could not be read again; the page shows it as last read.
  pub stale: Option<Message>,
}

impl lurq::app::component::DevtoolsInspectable for HeaderProps {}

pub struct TaskHeader {
  refusals: Refusals,
  /// Run again's confirmation, the page's: the Summary of a run that committed nothing opens it too.
  confirming: Signal<Confirming>,
  /// Its switch that removes the worktrees of the run before, off each time it opens.
  remove: Signal<bool>,
  /// Closing, reopening and deleting the task; a deleted task's page goes to the Tasks list.
  closer: Closer,
  /// Where *More actions* is open.
  menu: MenuPlace,
}

impl Component for TaskHeader {
  type Props = HeaderProps;

  fn create(ctx: &mut Ctx) -> Self {
    let state = super::PageState::of(ctx);
    let project = ctx.props::<HeaderProps>().task.project.clone();
    let tasks = Route::Project(project.open.project.id(), ProjectPage::Tasks(TaskList::default()));
    Self {
      refusals: Refusals::new(ctx),
      confirming: state.run_again,
      remove: state.run_again_remove,
      closer: Closer::new(ctx, (project.open, project.feed), Some(tasks)),
      menu: MenuPlace::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HeaderProps>().clone();
    let services = Services::of(ctx);
    let (open, feed) = (props.task.project.open.clone(), props.task.project.feed.clone());
    let detail = props.task.detail.clone();
    let runners = (services.runners.clone(), services.servers.clone());
    let again = (
      feed.revision.get() + feed.outside.get().tasks,
      // It checks only after a run that ended; what the project's other runs used today grows with their
      // agents' turns, reported as transcript activity, while one of them runs.
      feed.run_changes(feed.running.get()),
      feed.outside.get().limits,
    );
    let check = run_again::check(ctx, (open.repo_root.clone(), services.programs), &detail, again);
    let start = run_again::start_action(ctx, runners, (open, feed), self.confirming.clone());
    let latest_busy = detail
      .run
      .as_ref()
      .is_some_and(|run| props.task.jobs.run(run.id()).busy());
    let (primary, more) = self.actions(ctx, &props, check.as_ref(), latest_busy);
    let title_row = actions::title_row(ctx, &props, primary, more);
    let confirmation = check
      .as_ref()
      .and_then(|check| check.as_ref().ok())
      .and_then(|check| self.confirmation(ctx, &props, check, (&start, latest_busy)));
    let run_controls = RunControls::of(ctx);
    let state = run_controls.action.state().get();
    let refused = self.refusals.shown(&state, run_controls.requested.get(), &detail);
    Column::new()
      .id("task-header")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(TaskSpacing::Header.role())
          .left(TaskSpacing::Side.role())
          .right(TaskSpacing::Side.role()),
      )
      .spacing(ExtraSpacing::Loose.role())
      .on_key_down(edit_key(ctx, &props.task, &props.view))
      .child(back::row(ctx, &props))
      .child(title_row)
      .child(run_line::status_line(ctx, &props.task))
      .with_children(module::field(ctx, &props.task))
      .with_children(notices::notices(ctx, &detail, props.stale.as_ref(), refused))
      .with_children(confirmation)
      .with_children(task_close::panel(
        ctx,
        &self.closer,
        ("task-close", task_close::StopLook::Primary),
      ))
      .with_children(removal(ctx, &props))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::new().top(SpacingSize::Xs))
          .child(tabs(ctx, &props)),
      )
  }
}

impl TaskHeader {
  /// The primary action, and *More actions* with every other. A task kept *Not shared* starts no run: it has Share
  /// task, and Edit locked in its menu.
  fn actions(
    &self,
    ctx: &mut Ctx,
    props: &HeaderProps,
    check: Option<&Result<RunAgainCheck, Message>>,
    latest_busy: bool,
  ) -> (Option<controls::Control>, Element) {
    let detail = props.task.detail.clone();
    let kept = detail.not_shared();
    let run_controls = RunControls::of(ctx);
    let pending = run_controls.pending();
    let shown = controls(
      ctx,
      &Controls {
        detail: &detail,
        online: props.task.online,
        action: &run_controls.action,
        pending,
        requested: &run_controls.requested,
        approval_waits: props.task.waiting_approval().is_some(),
      },
    );
    let primary = primary::primary(ctx, &props.task, &props.view, shown.moves.clone());
    // The control that moves the run on goes in the menu while a card's action is the primary one.
    let moves = shown
      .moves
      .filter(|moves| primary.as_ref().is_none_or(|primary| primary.id != moves.id));
    let run_again = run_again::button(
      ctx,
      check,
      (&self.confirming, &self.remove),
      (latest_busy, props.task.online),
    );
    let held = if kept {
      Held {
        controls: Vec::new(),
        run_again: None,
        closing: None,
      }
    } else {
      Held {
        controls: moves.into_iter().chain(shown.pause).chain(shown.cancel).collect(),
        run_again,
        closing: Some((self.closer.clone(), subject(&props.task))),
      }
    };
    let more = more::more(ctx, &props.task, &self.menu, held);
    (primary, more)
  }

  /// Run again's confirmation, while it is open and the task can run again. `busy`: git removes or
  /// merges the latest run's worktrees.
  fn confirmation(
    &self,
    ctx: &mut Ctx,
    props: &HeaderProps,
    check: &RunAgainCheck,
    (start, busy): (&StartAction, bool),
  ) -> Option<Element> {
    let Confirming(Some(key)) = self.confirming.get() else {
      return None;
    };
    let (run, next) = (props.task.detail.run.as_ref()?, check.next.as_ref().ok()?);
    let leftovers = props.task.left.of(run.id()).and_then(Result::ok);
    Some(run_again::confirmation(
      ctx,
      &Confirmation {
        detail: &props.task.detail,
        run,
        check,
        next,
        key,
        leftovers: leftovers.as_ref(),
        remove: &self.remove,
        open: &self.confirming,
        action: start,
        busy,
      },
    ))
  }
}

/// The task as closing and deleting it name it.
fn subject(task: &TaskContext) -> Subject {
  let detail = &task.detail;
  Subject {
    id: detail.task.id(),
    key: Arc::from(detail.task.key().to_string()),
    status: detail.task.organization().status(),
    shared: detail.shared.is_some(),
  }
}

/// E opens Edit task while the task can be edited, unless a field has the keys or a modifier is held. Local
/// integration and the task's memory are editors of their own, whose editors and read-only text are no text inputs
/// to lurq: E stays theirs there.
fn edit_key(ctx: &mut Ctx, task: &TaskContext, view: &TaskView) -> impl Fn(KeyboardEvent) + use<> {
  let own_editor = matches!(view, TaskView::LocalIntegration | TaskView::Memory);
  let editable = task.detail.editable() && !task.detail.not_shared() && !own_editor;
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::EditTask(task.detail.task.key().clone()),
  );
  move |event: KeyboardEvent| {
    let plain = !(event.ctrl || event.alt || event.meta || event.shift);
    if !editable || event.default_prevented() || event.text_input_focused || !plain {
      return;
    }
    if event.key.eq_ignore_ascii_case(more::EDIT_KEY) {
      event.prevent_default();
      navigation.open(route.clone(), OpenIn::ActiveTab);
    }
  }
}

/// The latest run's *Remove worktree* confirmation, while the header's menu opened it.
fn removal(ctx: &mut Ctx, props: &HeaderProps) -> Option<Element> {
  let run = props.task.detail.run.as_ref()?.id();
  let removal = PageState::of(ctx).removal.get();
  if !removal.shows(run, RemovalPlace::Header) {
    return None;
  }
  let runs = Runs {
    task: &props.task,
    removal,
  };
  confirm_for(ctx, &runs, run)
}

/// Summary, Conversation, and Changes with the count of the files it lists.
fn tabs(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let changes_reads = super::changes::ChangesReads::of(ctx);
  let (project, key) = (
    props.task.project.open.project.id(),
    props.task.detail.task.key().clone(),
  );
  let kept = props.task.detail.not_shared();
  // Changes opens on the file it showed last.
  let open = |tab: TaskTab| -> Open {
    let (navigation, changes_reads, key) = (navigation.clone(), changes_reads.clone(), key.clone());
    Arc::new(move |at| {
      let view = match tab {
        TaskTab::Changes => TaskView::Changes(changes_reads.shown_file()),
        TaskTab::Summary | TaskTab::Conversation => TaskView::of(tab),
      };
      navigation.open(Route::Project(project, ProjectPage::task(key.clone(), view, kept)), at);
    })
  };
  let chosen = props.view.tab();
  let tab =
    |tab: TaskTab, label: Arc<str>| PageTab::to(format!("task-tab-{}", tab.name()), label, chosen == tab, open(tab));
  let changes = ctx.t(TaskTab::Changes.title_key());
  let count = super::changes_count(ctx, &props.task);
  let changes = super::changes::tab_label(ctx, &changes, count);
  let tabs = vec![
    tab(TaskTab::Summary, ctx.t(TaskTab::Summary.title_key())),
    tab(TaskTab::Conversation, ctx.t(TaskTab::Conversation.title_key())),
    tab(TaskTab::Changes, changes),
  ];
  page_tabs(ctx, "task-tabs", tabs).into()
}
