//! Window status bar (`pmOpt`, states `keJ9Z`): the 28px bar below every screen with the
//! runner, the latest background activity, the project's counts and the embedded MCP server's
//! address, and the operations popover (`E1msJS`) that opens upward from the activity item.

mod activity_item;
mod approvals_item;
mod counts;
mod hint;
mod item;
mod mcp_item;
mod open;
mod plan_limits;
mod popover;
mod runner_item;
mod timed;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::Row,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use self::{
  activity_item::{activity_slot, shows_activity},
  counts::counts,
  hint::{Anchors, Hint},
  open::Opener,
  runner_item::runner_item,
};
use crate::{
  activity::{Activity, ActivityLog, bar},
  embedded_mcp::McpStatus,
  runner::RunnerStatus,
  screens::project::{
    approval_queue::ApprovalQueue,
    run_approval_action::{RunApprovalAction, RunApprovalChanges, reload_after, run_approval_action},
  },
  tabs::feed::WindowFeed,
  tasks::{approvals::ShownApproval, data::AutoApprovingRun, run_approvals::RunApprovalChange, usage::ProjectUsage},
  theme::{APPROVALS_BRIEF_BELOW, ExtraColor, STATUS_BAR_HEIGHT},
};

/// A popover or hint sits 6px above the bar (`imT2E`), which is 9px above the 22px item it
/// hangs from.
const POPOVER_GAP: f32 = 9.0;

/// The open project, when there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectBar {
  pub project_name: String,
  pub connections: crate::plan_limits::Connections,
  pub repo_root: PathBuf,
  pub runner: RunnerStatus,
  /// Queued tasks; `None` while the tasks are read.
  pub queued: Option<usize>,
  /// Runs waiting for the user, the Overview's *Need your input*; `None` while the tasks are
  /// read.
  pub needs_you: Option<usize>,
  /// The project's limits and today's usage; `None` while they are read.
  pub usage: Option<ProjectUsage>,
  /// Runs recorded as running while no runner is connected; `None` while the tasks are read.
  pub lost_to_runner: Option<usize>,
  /// Tool requests waiting for a decision, oldest first.
  pub approvals: Arc<Vec<ShownApproval>>,
  /// Those the approval dialog may show on the current page: not a request shown inline on its task's
  /// Conversation tab, unless the person opened it.
  pub dialog: Arc<Vec<ShownApproval>>,
  /// Active runs that auto-approve their agents' requests.
  pub auto_approving: Arc<Vec<AutoApprovingRun>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusBarProps {
  /// `None` on the screens without a project (first launch, setup, startup failure).
  pub project: Option<ProjectBar>,
}

impl lurq::app::component::DevtoolsInspectable for StatusBarProps {}

pub struct StatusBar {
  activity: Activity,
  popover: Signal<bool>,
  hint: Signal<Option<Hint>>,
  anchors: Anchors,
}

impl Component for StatusBar {
  type Props = StatusBarProps;

  fn create(ctx: &mut Ctx) -> Self {
    // *Stop* of the runs that auto-approve; the bar's tasks are read again once it is written.
    let stop = run_approval_action(ctx);
    reload_after(ctx, &stop);
    Self {
      activity: Activity::of(ctx),
      popover: ctx.signal(false),
      hint: ctx.signal(None),
      anchors: Anchors::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StatusBarProps>().clone();
    let stop = run_approval_action(ctx);
    let log = self.log(ctx);
    let mcp = McpStatus::of_ctx(ctx);
    let opener = Opener::new(ctx, &self.activity, props.project.as_ref(), &self.popover);
    let runner = props
      .project
      .as_ref()
      .map(|project| project.runner.state_for(&project.repo_root));
    let hints = hint::Hints::new(&self.hint, &self.anchors);
    let readings = WindowFeed::of(ctx).limits.get().0;
    let limits = plan_limits::Props {
      project_name: props
        .project
        .as_ref()
        .map_or_else(String::new, |project| project.project_name.clone()),
      connections: props
        .project
        .as_ref()
        .map_or_else(Default::default, |project| project.connections.clone()),
      readings,
      brief: false,
    };
    // The bar spans the window; reading its width re-renders the bar as the window resizes.
    let width = ctx.window().logical_width();
    // The left group takes the room the right group leaves and never more: its activity gives
    // way with an ellipsis instead of running under the right group.
    let mut left = Row::new()
      .flex(1.0)
      .clip()
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center);
    if let Some(project) = &props.project {
      left = left.child(runner_item(ctx, project, &opener, &hints));
    }
    let slot = bar::activity_slot(runner.as_ref(), &log);
    // The runs the task lists show as interrupted, once read; until then what the runner was
    // working on when it dropped.
    let interrupted = props.project.as_ref().map_or(0, |project| {
      project.lost_to_runner.unwrap_or_else(|| project.runner.interrupted())
    });
    // A request waiting for a person comes before the latest activity, and any run that auto-approves after it.
    // A runner problem comes first: nothing can be answered without the runner.
    let queue = ctx.use_context::<ApprovalQueue>();
    let waiting = match (&props.project, &slot) {
      (_, bar::ActivitySlot::RunnerProblem(_)) | (None, _) => Vec::new(),
      (Some(project), _) => approvals_item::approvals(
        ctx,
        (&project.approvals, &project.dialog),
        queue.as_ref(),
        width < APPROVALS_BRIEF_BELOW,
      ),
    };
    // A run that waits for the person's decision keeps its item beside the waiting approvals: neither
    // hides the other. Any other activity gives way to them.
    let activity = (waiting.is_empty() || slot.beside_approvals()).then(|| {
      activity_slot(
        ctx,
        (&slot, interrupted),
        &log,
        &opener,
        &hints,
        props.project.is_some(),
      )
    });
    left = left
      .with_children(waiting)
      .with_children(activity.into_iter().flatten());
    if let Some(project) = &props.project {
      left = left.with_children(approvals_item::auto_approving(
        ctx,
        &project.auto_approving,
        stop_press(&stop, project),
      ));
    }
    // The popover hangs from the activity item; without it there is nothing to anchor to.
    let popover = shows_activity(&slot).then(|| popover::popover(ctx, &log, &opener, &self.anchors));
    Row::new()
      .id("status-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STATUS_BAR_HEIGHT))
      .padding_horizontal(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .background(PaletteColor::SurfaceBase)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(left)
      .child(counts(ctx, (props.project.as_ref(), mcp), &opener, &hints, (&limits, width)))
      .with_children(popover)
      // A hint would cover the popover it belongs to.
      .with_children(hints.popup().filter(|_| !self.popover.get()))
  }
}

/// *Stop*: ends the auto-approval of every run the bar names.
fn stop_press(stop: &RunApprovalAction, project: &ProjectBar) -> impl Fn() + Send + Sync + 'static {
  let stop = stop.clone();
  let request = RunApprovalChanges {
    repo_root: project.repo_root.clone(),
    changes: project
      .auto_approving
      .iter()
      .map(|run| (run.run, RunApprovalChange::AutoApprove(false)))
      .collect(),
  };
  move || {
    if !stop.state().get_untracked().is_pending() {
      stop.run(request.clone());
    }
  }
}

impl StatusBar {
  /// The session's activity, read reactively from the window's feed: a status bar in an inactive
  /// tab only renders again once its tab is active.
  fn log(&self, ctx: &mut Ctx) -> ActivityLog {
    WindowFeed::of(ctx).activity.get()
  }
}
