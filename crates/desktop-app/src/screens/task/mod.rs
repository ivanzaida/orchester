//! A task's page (`Task · Summary · running · full page · dark` `lUXff`; the frame of `oTzJZ`, `DVuXu`,
//! `F2LtFn`): one frame for every tab of the task (`/tasks/<key>/<tab>`), so switching tabs keeps it and
//! what it holds. The frame is the header (`header`: breadcrumb, title, run controls, status and run
//! line, the frame's notices, Run again's confirmation in place) with the page tabs, and under it the
//! tab: the Summary (`summary`), the Conversation and Changes, which their own tasks build and mount at
//! [`conversation_tab`] and [`changes_tab`].
//!
//! The task is read off the UI thread (`read`) and again whenever the runner changes one of the
//! project's runs, tasks are saved, or the person tries again after a failed reading; a failed reading
//! again keeps the task as last read on the page and says why. What the task's ended runs left in the
//! repository is read beside it, for *Remove worktree* and Run again.

mod context;
mod conversation;
mod header;
mod read;
mod summary;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
};
use orchester_workspace::{messages::Message, runner::state::RunnerState};

pub use self::context::TaskContext;
use self::{
  header::{HeaderProps, TaskHeader},
  read::{ReadAgain, TaskRead},
  summary::{SummaryProps, TaskSummary},
};
use super::placeholder::centered;
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  ui::{
    icon::Icon,
    message::message_text,
    state::{EmptyState, load_error, loading},
  },
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, view: &TaskView) -> Element {
  let props = PageProps {
    project: project.clone(),
    key: key.clone(),
    tab: view.tab(),
  };
  // Keyed by the task, not the tab: the frame stays as the tabs switch.
  ctx.mount_keyed::<TaskPage>(&format!("task-page-{key}"), props)
}

#[derive(Clone, PartialEq)]
struct PageProps {
  project: ShownProject,
  key: TaskKey,
  tab: TaskTab,
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
      TasksState::Ready(tasks) => tasks
        .tasks
        .iter()
        .find(|task| *task.key() == props.key)
        .map(|task| task.id()),
    };
    let Some(task) = found else {
      return page(not_found(ctx, &props));
    };
    let frame = FrameProps {
      project: props.project,
      task,
      key: props.key,
      tab: props.tab,
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
      .on_click(move || navigation.go(Route::Project(project, ProjectPage::Tasks(Default::default()))));
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
  tab: TaskTab,
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
  state: PageState,
}

impl Component for TaskFrame {
  type Props = FrameProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retries: ctx.signal(0),
      state: PageState {
        removal: ctx.signal(Removal::closed()),
        discard: ctx.signal(false),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FrameProps>().clone();
    ctx.provide(self.state.clone());
    let feed = &props.project.feed;
    let saves = feed.revision.get() + feed.outside.get().tasks;
    let again = ReadAgain {
      changes: feed.changes.get(),
      saves,
      retries: self.retries.get(),
    };
    let root = props.project.open.repo_root.clone();
    let (detail, stale) = match read::task(ctx, root.clone(), props.task, again) {
      TaskRead::Loading => return loading("task-loading", &ctx.t("task.page.loading")).into(),
      TaskRead::Failed(reason) => return read_failed(ctx, &reason, Some(self.retries.clone())),
      TaskRead::Ready(detail, stale) => (detail, stale),
    };
    let services = Services::of(ctx);
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
      tab: props.tab,
      stale,
    };
    let body = match props.tab {
      TaskTab::Summary => ctx.mount_keyed::<TaskSummary>("task-summary", SummaryProps { task }),
      TaskTab::Conversation => conversation_tab(ctx, &task),
      TaskTab::Changes => changes_tab(ctx, &task),
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

/// The Conversation tab's mount point: its timeline (C1) mounts here with one line, the composer (C2)
/// and the decision cards (C3) inside it. Until then the tab says it is not built.
fn conversation_tab(ctx: &mut Ctx, _task: &TaskContext) -> Element {
  tab_not_built(ctx, "task-conversation", Icon::MessageSquare, TaskTab::Conversation)
}

/// The Changes tab's mount point: its file list and diff (D1) mount here with one line. Until then the
/// tab says it is not built.
fn changes_tab(ctx: &mut Ctx, _task: &TaskContext) -> Element {
  tab_not_built(ctx, "task-changes", Icon::FileDiff, TaskTab::Changes)
}

/// How many files the Changes tab lists, for its label (`Changes 4`); the Changes tab (D1) reads it
/// here with one line. `None` while it is not known: the label is the tab's name alone.
pub fn changes_count(_ctx: &mut Ctx, _task: &TaskContext) -> Option<usize> {
  None
}

fn tab_not_built(ctx: &Ctx, id: &str, glyph: Icon, tab: TaskTab) -> Element {
  let title = ctx.t(tab.title_key());
  let body = ctx.t("task.page.tabNotBuilt");
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(centered(glyph, &title, &body).id(id.to_owned()))
    .into()
}
