//! The status bar (`Status bar` `sMpGe` of `q9mf0k`; handover §3.3), for the active tab's project: on the
//! left the project, its folder in the name's tooltip, the runner and the activity slot; on the right the project's
//! counts and tokens, the waiting requests with *Review* and the runs that auto-approve with *Stop* (`xommZ`),
//! the app's MCP server and the version.
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
mod operations;
mod recovery;
mod runner;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
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
  recovery::RecoveryState,
};

pub use self::facts::{BarAction, BarFacts, Dispatch, ProjectFacts, ReadFailure, RunnerFacts, Stopping};
use self::{
  act::Stopper,
  item::{frame, give_way, glyph, item, label},
};
use crate::{
  lifecycle,
  mcp::McpStatus,
  project::{ProjectFeed, ProjectState, Projects, TasksState},
  routes::Route,
  services::Services,
  theme::ExtraColor,
  ui::{icon::Icon, tooltip::Truncated},
};

/// `Status bar` height.
pub const STATUS_BAR_HEIGHT: f32 = 28.0;

/// The window's status bar.
pub struct StatusBar {
  stopper: Stopper,
}

impl Component for StatusBar {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      stopper: Stopper::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First, before the render's streams: lurq keeps a component's futures by the order it asks for them.
    let stop = self.stopper.action(ctx);
    let services = Services::of(ctx);
    let mcp = ctx
      .use_context::<McpStatus>()
      .expect("the root provides the MCP status before mounting the shell");
    let log = log(ctx, &services.activity);
    let recovery = lifecycle::recovery::follow(ctx, &services.recovery);
    let projects = Projects::of(ctx);
    let shown = Route::parse(&ctx.route_path())
      .ok()
      .and_then(|route| route.project())
      .and_then(|project| shown(&projects, project, &recovery));
    let folder_and_feed = shown
      .as_ref()
      .map(|(facts, feed)| (facts.repo_root.clone(), feed.clone()));
    let dispatch = act::dispatch(ctx, &services, folder_and_feed, (&stop, &self.stopper));
    let stopping = shown.as_ref().map_or(Stopping::Idle, |(facts, _)| {
      let runs: Vec<_> = facts
        .tasks
        .as_ref()
        .map(|tasks| tasks.auto_approving.iter().map(|run| run.run).collect())
        .unwrap_or_default();
      self.stopper.stopping(&stop, &facts.repo_root, &runs)
    });
    let facts = BarFacts {
      project: shown.map(|(facts, _)| facts),
      log,
      mcp,
      stopping,
    };
    ctx.mount_keyed::<StatusBarView>("status-bar-view", BarProps { facts, dispatch })
  }
}

/// The open `project`'s facts and its feed, once it is open; `recovery` says which of its interrupted runs
/// wait for the person's decision.
fn shown(projects: &Projects, project: ProjectId, recovery: &RecoveryState) -> Option<(ProjectFacts, ProjectFeed)> {
  let Some(ProjectState::Open(open)) = projects.state(project) else {
    return None;
  };
  let feed = projects.feed(project);
  let runner = RunnerFacts::of(feed.status.get().as_ref(), &open.repo_root);
  let (tasks, first_failure) = match feed.tasks_with_runner(&open.repo_root) {
    TasksState::Ready(tasks) => (Some(tasks), None),
    TasksState::Loading => (None, None),
    TasksState::Failed(reason) => (None, Some(ReadFailure::First(reason))),
  };
  let read_failure = first_failure.or_else(|| feed.stale.get().map(ReadFailure::Again));
  let facts = ProjectFacts {
    id: project,
    name: open.project.name().as_str().to_owned(),
    repo_root: open.repo_root,
    runner,
    tasks,
    usage: feed.usage.get(),
    read_failure,
    undecided_runs: recovery.undecided(project),
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
    // Tasks that could not be read are the project's own unresolved failure: they take the slot until a
    // reading succeeds. Then the runs the previous session left interrupted stay pinned until the person
    // decides about them (`uZTnL`), also while the runner starts; only a runner problem comes first.
    let unread = project.and_then(|project| project.read_failure.as_ref());
    let recovering = project.filter(|project| project.undecided_runs > 0);
    match (unread, recovering, &slot) {
      (_, _, ActivitySlot::RunnerProblem(_)) => {}
      (Some(failure), _, ActivitySlot::Activity(_)) => left.extend(activity::read_failure(ctx, failure, &dispatch)),
      (None, Some(project), _) => left.extend(recovery::pinned(ctx, project, &dispatch)),
      (_, _, ActivitySlot::Hidden) => {}
      (None, None, ActivitySlot::Activity(slot)) if beside => {
        left.extend(activity::activity(ctx, (slot, &facts.log), project, &dispatch));
      }
      (None, None, ActivitySlot::Activity(_)) => {}
    }
    // The requests and the runs that answer their own follow Tokens today (`xommZ`).
    right.extend(waiting);
    if let Some(tasks) = project.and_then(|project| project.tasks.as_ref()) {
      right.extend(approvals::auto_approving(
        ctx,
        &tasks.auto_approving,
        &facts.stopping,
        &dispatch,
      ));
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

/// *Kontur* (`ZQiMf`), its folder in a mono tooltip from the item's start while the pointer rests on it,
/// the item in the `hover` fill with its words in `text_secondary` (`f5nEqj`, `C9Phsz` in `z61KIT`); once
/// the name is cut, the tooltip says both.
fn project_item(ctx: &mut Ctx, project: &ProjectFacts) -> Element {
  let folder = project.repo_root.display().to_string();
  let both = ctx.t_args(
    "shell.status.projectCut",
    [("name", project.name.as_str()), ("folder", folder.as_str())],
  );
  let name = Truncated::new("status-project-text", project.name.as_str(), TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .hint(folder, both)
    .mono()
    .framed(PaletteColor::TextSecondary, |row, hovered| {
      let row = frame(row).rounded(RadiusSize::Sm);
      if hovered {
        row
          .background(ExtraColor::Hover.role())
          .child(glyph(Icon::HardDrive, PaletteColor::TextSecondary))
      } else {
        row.child(glyph(Icon::HardDrive, PaletteColor::TextMuted))
      }
    })
    .render(ctx);
  Row::new()
    .id("status-project")
    .flex_shrink(1.0)
    .shrink_order(give_way::PROJECT)
    .shrink_limit(ShrinkLimit::Content)
    .child(name)
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
        .child(label(&version, PaletteColor::TextMuted)),
    )
    .into()
}
