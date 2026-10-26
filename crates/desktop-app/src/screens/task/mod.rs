//! A task's page (ORC-100, `Task page · redesign` `C3wC8`: `01 · Summary` `AsE7R`, `02 · Conversation` `XIBg8`,
//! `03 · Changes` `X5ZKG`): one frame for every tab of the task (`/tasks/<key>/<tab>`), so switching tabs keeps it and
//! what it holds. The frame is the header (`header`, `Task header` `j8DakG`: the way back to Tasks, the title with
//! the one action the task's state calls for and *More actions*, the status line, the frame's notices and
//! confirmations in place) with the page tabs Summary, Conversation and Changes, and under it the tab: the Summary
//! (`summary`, which holds the task's requirements, its run, its decision and its runs), the Conversation and
//! Changes, which their own tasks build and mount at [`conversation_tab`] and [`changes_tab`]. The Summary's
//! properties open the task's memory (`memory`) in its place, Changes' *Try locally* Local integration
//! (`local_integration`). The frame holds the run controls' request, so the header and the Summary share it.
//!
//! The task is read off the UI thread (`read`) and again whenever the runner changes one of the
//! project's runs, tasks are saved, or the person tries again after a failed reading. A failed reread shows
//! the error until fresh data succeeds, so retained data cannot offer decisions. What the task's ended runs
//! left in the repository is read beside it, for *Remove worktree* and Run again.

pub(crate) mod cards;
pub(crate) mod changes;
mod context;
mod conversation;
mod header;
mod local_integration;
mod memory;
mod orchestrated;
mod read;
mod sharing;
mod summary;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
};
use orchester_workspace::{messages::Message, runner::state::RunnerState};

pub use self::context::TaskContext;
pub(crate) use self::conversation::{links, step_label};
use self::{
  changes::{ChangesReads, FileCount, Follow},
  header::{
    HeaderProps, TaskHeader,
    controls::{RunControls, control_action},
  },
  read::{ReadAgain, TaskRead},
  summary::{EventWords, SummaryProps, TaskSummary, Version},
};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route, TaskView},
  services::Services,
  ui::{
    icon::Icon,
    message::message_text,
    state::{EmptyState, load_error, loading},
  },
};

/// The page of the task `key`; `kept`: the task kept *Not shared* with that key, which a shared task may have too.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, (key, kept): (&TaskKey, bool), view: &TaskView) -> Element {
  let props = PageProps {
    project: project.clone(),
    key: key.clone(),
    kept,
    view: view.without_file(),
  };
  // Keyed by the task, not the tab: the frame stays as the tabs switch.
  let place = if kept { "kept-" } else { "" };
  ctx.mount_keyed::<TaskPage>(&format!("task-page-{place}{key}"), props)
}

#[derive(Clone, PartialEq)]
struct PageProps {
  project: ShownProject,
  key: TaskKey,
  kept: bool,
  /// What the page shows, on no particular file: Changes reads its file from the route itself.
  view: TaskView,
}

impl lurq::app::component::DevtoolsInspectable for PageProps {}

/// Finds the task the path names among the project's tasks; once found, shows its frame.
struct TaskPage;

impl Component for TaskPage {
  type Props = PageProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PageProps>().clone();
    let found = match props.project.feed.tasks.get() {
      TasksState::Loading => return page(loading("task-loading", &ctx.t("task.page.loading"))),
      TasksState::Failed(reason) => return page(read_failed(ctx, &reason, None)),
      // A task kept Not shared keeps its key from before the join, which a shared task may have too: its own
      // address names it. A link to a key no shared task has still finds it.
      TasksState::Ready(tasks) => {
        let find =
          |list: &[orchester_core::task::Task]| list.iter().find(|task| *task.key() == props.key).map(|task| task.id());
        let kept = || find(&tasks.not_shared).map(|task| (task, true));
        if props.kept {
          kept()
        } else {
          find(&tasks.tasks).map(|task| (task, false)).or_else(kept)
        }
      }
    };
    let Some((task, kept)) = found else {
      return page(not_found(ctx, &props));
    };
    let frame = FrameProps {
      project: props.project,
      task,
      key: props.key,
      kept,
      view: props.view,
    };
    page(ctx.mount_keyed::<TaskFrame>(&format!("task-frame-{task}"), frame))
  }
}

/// The page's own element, which tests and agents find the task page by.
fn page(content: impl Into<Element>) -> Element {
  Column::new()
    .id("screen-task")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(content)
    .into()
}

fn not_found(ctx: &mut Ctx, props: &PageProps) -> Element {
  let key = props.key.to_string();
  let tasks = ctx.t("task.page.toTasks");
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  let back =
    crate::ui::button::ActionButton::new("task-not-found-tasks", crate::ui::button::ButtonKind::Secondary, tasks)
      .opens(navigation.to(Route::Project(project, ProjectPage::Tasks(Default::default()))));
  let name = props.project.open.project.name().as_str().to_owned();
  let card = EmptyState::new(
    "task-not-found",
    Icon::CircleHelp,
    ctx.t_args(
      "task.page.notFound",
      [("task", key.as_str()), ("project", name.as_str())],
    ),
    ctx.t("task.page.notFoundBody"),
  )
  .action(back);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(card)
    .into()
}

/// Reading failed: why, with Try again when `retry` is given.
fn read_failed(ctx: &Ctx, reason: &Message, retry: Option<Signal<u64>>) -> Element {
  let body = message_text(ctx, reason);
  let title = ctx.t("task.page.loadFailed");
  let content = match retry {
    Some(retries) => load_error(ctx, "task-load-error", (&title, &body), move || {
      retries.update(|count| *count += 1);
    }),
    None => crate::ui::notice::Notice::new("task-load-error", crate::ui::notice::NoticeTone::Danger, title)
      .body(body)
      .into(),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(content)
    .into()
}

#[derive(Clone, PartialEq)]
struct FrameProps {
  project: ShownProject,
  task: TaskId,
  key: TaskKey,
  /// The task is one kept *Not shared*: its page's links keep its own address.
  kept: bool,
  view: TaskView,
}

impl lurq::app::component::DevtoolsInspectable for FrameProps {}

/// Whose *Remove worktree* confirmation is open, and where: under the header when the header's button
/// opened it for the latest run, in the run's row of *Runs of this task* when the row's did. One at a
/// time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Removal {
  pub run: Option<RunId>,
  pub place: RemovalPlace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RemovalPlace {
  #[default]
  Header,
  Row,
}

impl lurq::app::component::DevtoolsInspectable for Removal {}

impl Removal {
  pub const fn closed() -> Self {
    Self {
      run: None,
      place: RemovalPlace::Header,
    }
  }

  /// Open for `run` at `place`.
  pub const fn of(run: RunId, place: RemovalPlace) -> Self {
    Self { run: Some(run), place }
  }

  /// Open for `run` at `place` now.
  pub fn shows(self, run: RunId, place: RemovalPlace) -> bool {
    self.run == Some(run) && self.place == place
  }
}

/// What the frame shares with its parts while it shows one task.
#[derive(Clone)]
pub struct PageState {
  pub removal: Signal<Removal>,
  /// The open removal's switch that agrees to discard uncommitted changes; off each time one opens.
  pub discard: Signal<bool>,
  /// Where the Summary is scrolled, kept as the tabs switch.
  pub summary_scroll: ScrollState,
  /// The version of the task's requirements the Summary's brief shows, once one was shown or chosen; kept as
  /// the tabs switch and the task is read again.
  pub requirements: Signal<Option<Version>>,
  /// A decision's own words for the run's last event, where they rest on what the decision read itself (the account
  /// a plan limit stopped); for the run and the moment it read them.
  pub event: Signal<Option<EventWords>>,
  /// *Share task*'s question is open over the Summary of a task kept *Not shared*.
  pub share: Signal<bool>,
  /// Run again's confirmation under the header, which the Summary of a run that committed nothing opens too (ORC-93),
  /// and its switch that removes the worktrees of the run before, off each time it opens.
  pub run_again: Signal<header::run_again::Confirming>,
  pub run_again_remove: Signal<bool>,
}

impl PageState {
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the task frame provides its state before mounting its parts")
  }

  /// Opens the removal of `run` at `place`, its switch off.
  pub fn open_removal(&self, run: RunId, place: RemovalPlace) {
    self.discard.set(false);
    self.removal.set(Removal::of(run, place));
  }
}

/// One task's frame and its current tab.
struct TaskFrame {
  retries: Signal<u64>,
  /// The Changes tab's reads, for its count in the page tabs and the tab itself.
  changes: ChangesReads,
  /// Why the task could not be read again, kept while it is read again until a reading succeeds.
  stale: Signal<StaleRead>,
  state: PageState,
  /// The run control last pressed, in the header or the Summary.
  requested: Signal<Option<header::controls::Requested>>,
  /// The composer's draft and sends, kept across the page's tabs and the task's runs.
  composer: conversation::ComposerState,
  /// Whether the task's latest run ran when the task was last read: only then do its transcript's lines
  /// change the task.
  live: Signal<bool>,
}

impl Component for TaskFrame {
  type Props = FrameProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<FrameProps>().clone();
    let composer = conversation::ComposerState::new(ctx, &props.project, (props.task, &props.key, props.kept));
    Self {
      composer,
      retries: ctx.signal(0),
      changes: ChangesReads::new(ctx),
      stale: ctx.signal(StaleRead(None)),
      state: PageState {
        removal: ctx.signal(Removal::closed()),
        discard: ctx.signal(false),
        summary_scroll: ScrollState::new(),
        requirements: ctx.signal(None),
        event: ctx.signal(None),
        share: ctx.signal(false),
        run_again: ctx.signal(header::run_again::Confirming::default()),
        run_again_remove: ctx.signal(false),
      },
      requested: ctx.signal(None),
      live: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FrameProps>().clone();
    // First: lurq keeps a component's futures by the order it asks for them, and a page asks for this one
    // only when it was made while a send ran, which holds for the page's life.
    self.composer.await_flight(ctx);
    let services = Services::of(ctx);
    let runners = (services.runners.clone(), services.servers.clone());
    let action = control_action(ctx, (props.project.open.clone(), props.task), runners);
    ctx.provide(RunControls {
      action,
      requested: self.requested.clone(),
    });
    ctx.provide(self.composer.clone());
    ctx.provide(self.state.clone());
    ctx.provide(self.changes.clone());
    let feed = &props.project.feed;
    // A change of the merge queue changes where the task's run stands in its delivery (ORC-19).
    let queue = crate::project::merge_queue::changes(ctx, &props.project.open.repo_root);
    let saves = feed.revision.get() + feed.outside.get().tasks + queue;
    let again = ReadAgain {
      changes: feed.run_changes(self.live.get()),
      saves,
      retries: self.retries.get(),
    };
    let root = props.project.open.repo_root.clone();
    let (detail, stale) = match read::task(ctx, root.clone(), props.task, again) {
      TaskRead::Loading => return loading("task-loading", &ctx.t("task.page.loading")).into(),
      TaskRead::Failed(reason) => return read_failed(ctx, &reason, Some(self.retries.clone())),
      TaskRead::Ready(detail, stale, pending) => (detail, self.stale(stale, pending)),
    };
    // Retained future data is no longer safe to act on after a failed reread, including during its retry.
    if let Some(reason) = &stale {
      return read_failed(ctx, reason, Some(self.retries.clone()));
    }
    // Set only when it changes: the frame then renders again, following the transcript's lines or not.
    let live = read::runs(&detail);
    if self.live.get_untracked() != live {
      self.live.set(live);
    }
    let jobs = feed.job_state.get();
    let separator = ctx.t("task.listSeparator");
    let left = read::leftovers(
      ctx,
      (root.clone(), services.programs, separator),
      &detail,
      (saves, jobs.removed_outside()),
    );
    let online = feed
      .status
      .get()
      .is_some_and(|status| status.state_for(&root) == RunnerState::Online);
    let follow = Follow::of(&detail, online, props.view.tab());
    self.changes.read(ctx, &root, &detail, follow);
    let task = TaskContext {
      project: props.project.clone(),
      key: props.key.clone(),
      detail,
      left,
      online,
      jobs,
    };
    let header = HeaderProps {
      task: task.clone(),
      view: props.view.clone(),
      stale,
    };
    let body = match props.view {
      TaskView::Summary => ctx.mount_keyed::<TaskSummary>("task-summary", SummaryProps { task }),
      TaskView::Memory => memory::tab(ctx, &task),
      TaskView::Conversation => conversation_tab(ctx, &task),
      TaskView::Changes(_) => changes_tab(ctx, &task),
      TaskView::LocalIntegration => local_integration::tab(ctx, &task),
    };
    Column::new()
      .id("task-frame")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(ctx.mount_keyed::<TaskHeader>("task-header", header))
      .child(body)
      .into()
  }
}

impl TaskFrame {
  /// Why the task could not be read again: `failed` now, else what failed before while the task is
  /// read again (`pending`), so the notice does not flicker; nothing once a reading succeeded.
  fn stale(&self, failed: Option<Message>, pending: bool) -> Option<Message> {
    let kept = match failed {
      Some(reason) => Some(reason),
      None if pending => self.stale.get_untracked().0,
      None => None,
    };
    if self.stale.get_untracked().0 != kept {
      self.stale.set(StaleRead(kept.clone()));
    }
    kept
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct StaleRead(Option<Message>);

impl lurq::app::component::DevtoolsInspectable for StaleRead {}

/// The Conversation tab's mount point: its timeline (C1), with the composer (C2) and the decision cards (C3)
/// inside it.
fn conversation_tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  conversation::tab(ctx, task)
}

/// The Changes tab's mount point: its file list and diff (`changes`).
fn changes_tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  changes::tab(ctx, task)
}

/// How many files the Changes tab lists, for its label (`Changes 4`, `Changes 4+` while its worktree's
/// files are not known); `None` while it is not known: the label is the tab's name alone.
pub fn changes_count(ctx: &mut Ctx, _task: &TaskContext) -> Option<FileCount> {
  changes::count(ctx)
}

/// How many active records the task's memory has, for the Summary's link to it (`3 memory notes`); `None` while it
/// is read.
pub fn memory_count(ctx: &mut Ctx, task: &TaskContext) -> Option<usize> {
  memory::active_count(ctx, task)
}
