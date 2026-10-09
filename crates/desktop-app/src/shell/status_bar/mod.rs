//! The status bar (`Status bar` `sMpGe` of `q9mf0k`; handover §3.3), for the active tab's project: on the
//! left the project and its folder, the runner, then the waiting requests, the activity slot and the runs
//! that auto-approve; on the right the project's counts and tokens, the app's MCP server and the version.
//! Every item shows only measured data. A screen outside a project shows the activity and the app's items
//! only, its events named by their project. A startup screen's bar has the version only.
//!
//! [`StatusBar`] gathers what the bar shows from the window ([`BarFacts`]) and does what its items ask
//! (`act`); [`StatusBarView`] draws it, so a test can hand it any state.

mod act;
mod activity;
mod approvals;
mod counts;
mod facts;
mod item;
mod runner;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Spacer},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  activity::{
    Activity, ActivityLog,
    bar::{self, ActivitySlot},
  },
  display,
};

pub use self::facts::{BarAction, BarFacts, Dispatch, ProjectFacts, RunnerFacts};
use self::{
  act::StopAction,
  item::{give_way, glyph, item, trimmed},
};
use crate::{
  mcp::McpStatus,
  project::{ProjectFeed, ProjectState, Projects, TasksState},
  routes::Route,
  services::Services,
  ui::icon::Icon,
};

/// `Status bar` height.
const STATUS_BAR_HEIGHT: f32 = 28.0;
/// The project's folder is shortened in the middle past this many characters (handover §5.3).
const FOLDER_MAX_CHARS: usize = 32;

/// The window's status bar.
pub struct StatusBar {
  stop: StopAction,
}

impl Component for StatusBar {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      stop: act::stop_action(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = Services::of(ctx);
    let mcp = ctx
      .use_context::<McpStatus>()
      .expect("the root provides the MCP status before mounting the shell");
    let log = log(ctx, &services.activity);
    let projects = Projects::of(ctx);
    let shown = Route::parse(&ctx.route_path())
      .ok()
      .and_then(|route| route.project())
      .and_then(|project| shown(&projects, project));
    let folder_and_feed = shown
      .as_ref()
      .map(|(facts, feed)| (facts.repo_root.clone(), feed.clone()));
    let dispatch = act::dispatch(ctx, &services, folder_and_feed, &self.stop);
    let facts = BarFacts {
      project: shown.map(|(facts, _)| facts),
      log,
      mcp,
    };
    ctx.mount_keyed::<StatusBarView>("status-bar-view", BarProps { facts, dispatch })
  }
}

/// The open `project`'s facts and its feed, once it is open.
fn shown(projects: &Projects, project: ProjectId) -> Option<(ProjectFacts, ProjectFeed)> {
  let Some(ProjectState::Open(open)) = projects.state(project) else {
    return None;
  };
  let feed = projects.feed(project);
  let runner = RunnerFacts::of(feed.status.get().as_ref(), &open.repo_root);
  let tasks = match feed.tasks_with_runner(&open.repo_root) {
    TasksState::Ready(tasks) => Some(tasks),
    TasksState::Loading | TasksState::Failed(_) => None,
  };
  let facts = ProjectFacts {
    id: project,
    name: open.project.name().as_str().to_owned(),
    repo_root: open.repo_root,
    runner,
    tasks,
    usage: feed.usage.get(),
    stale: feed.stale.get(),
  };
  Some((facts, feed))
}

/// The session's activity, whenever it changes.
fn log(ctx: &mut Ctx, activity: &Activity) -> ActivityLog {
  let stream = ctx.stream(
    activity.clone(),
    |activity, emitter: StreamEmitter<ActivityLog, ()>| async move {
      let mut updates = activity.subscribe();
      loop {
        let log = updates.borrow_and_update().clone();
        if !emitter.emit(log) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  stream.state().get().data.unwrap_or_default()
}

/// What [`StatusBarView`] shows and where its actions go.
#[derive(Clone)]
pub struct BarProps {
  pub facts: BarFacts,
  pub dispatch: Dispatch,
}

/// Props are compared by what the bar shows; the dispatch goes to the same window.
impl PartialEq for BarProps {
  fn eq(&self, other: &Self) -> bool {
    self.facts == other.facts
  }
}

impl DevtoolsInspectable for BarProps {}

/// The status bar as `BarProps` says.
pub struct StatusBarView;

impl Component for StatusBarView {
  type Props = BarProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let BarProps { facts, dispatch } = ctx.props::<BarProps>().clone();
    let project = facts.project.as_ref();
    let slot = bar::activity_slot(project.map(|project| &project.runner.state), &facts.log);
    let mut left = Vec::new();
    let mut right = Vec::new();
    if let Some(project) = project {
      left.push(project_item(ctx, project));
      left.extend(runner::runner(ctx, &project.runner, &dispatch));
      right.extend(counts::counts(ctx, project, &dispatch));
    }
    // A runner problem comes first: nothing can be answered without the runner.
    let waiting = match (project, &slot) {
      (_, ActivitySlot::RunnerProblem(_)) | (None, _) => Vec::new(),
      (Some(project), _) => project.tasks.as_ref().map_or_else(Vec::new, |tasks| {
        approvals::approvals(ctx, project, &tasks.pending_tool_approvals, &dispatch)
      }),
    };
    // A run that waits for the person's decision keeps its item beside the waiting requests; any other
    // activity gives way to them.
    let beside = waiting.is_empty() || slot.beside_approvals();
    left.extend(waiting);
    // Tasks that could not be read again are the project's own unresolved failure: they take the slot
    // until a reading succeeds.
    let stale = project.and_then(|project| project.stale.as_ref());
    match (stale, &slot) {
      (_, ActivitySlot::RunnerProblem(_) | ActivitySlot::Hidden) => {}
      (Some(reason), _) => left.extend(activity::stale(ctx, reason, &dispatch)),
      (None, ActivitySlot::Activity(slot)) if beside => {
        left.extend(activity::activity(ctx, slot, project, &dispatch));
      }
      (None, ActivitySlot::Activity(_)) => {}
    }
    if let Some(tasks) = project.and_then(|project| project.tasks.as_ref()) {
      left.extend(approvals::auto_approving(ctx, &tasks.auto_approving, &dispatch));
    }
    right.extend(counts::app(ctx, facts.mcp));
    Row::new()
      .id("status-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STATUS_BAR_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .clip()
      .with_children(left)
      // The room between the groups: as wide as the bar to begin with, it gives way before anything else,
      // so the right group ends at the bar's end whenever both fit.
      .child(
        Spacer::new()
          .width(Dimension::Pct(100.0))
          .flex_shrink(1.0)
          .shrink_order(give_way::ROOM),
      )
      .with_children(right)
  }
}

/// *Kontur · H:/projects/kontur* (`Status · Kontur · H:/projects/kontur` `ZQiMf`).
fn project_item(ctx: &Ctx, project: &ProjectFacts) -> Element {
  let text = ctx.t_args(
    "shell.status.project",
    [
      ("name", project.name.clone()),
      ("folder", display::shorten_path(&project.repo_root, FOLDER_MAX_CHARS)),
    ],
  );
  item("status-project")
    .flex_shrink(1.0)
    .shrink_order(give_way::PROJECT)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::HardDrive, PaletteColor::TextMuted))
    .child(trimmed(&text, PaletteColor::TextMuted))
    .into()
}

/// The status bar of a startup screen (`Status bar` `xmswq` of `N0Lpc`): the version only.
pub fn version_only(ctx: &mut Ctx) -> Element {
  let version = ctx.t_args("shell.status.version", [("version", env!("CARGO_PKG_VERSION"))]);
  Row::new()
    .id("status-bar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STATUS_BAR_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(Row::new().flex(1.0))
    .child(
      item("status-version")
        .child(glyph(Icon::Sparkles, PaletteColor::TextMuted))
        .child(trimmed(&version, PaletteColor::TextMuted)),
    )
    .into()
}
