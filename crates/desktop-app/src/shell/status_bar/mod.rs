//! The status bar (`Status bar` `sMpGe` of `q9mf0k`, redesigned in `Status bar · redesign` `bZVXG`; handover §3.3),
//! for the active tab's project. Calm, it holds two things: on the left the activity slot (the latest event or the
//! operation under way), at the right end the health dot, whose card on hover holds the runner, the agents, the
//! queue, the team server, the MCP server and the version (`health`). Everything else shows only while it needs the
//! person: on the left the runner while it is not online, a failure nobody opened, tasks that could not be read and
//! runs interrupted when Orchester closed; on the right, in this order, runs that auto-approve with *Stop*, the plan
//! limits, a team server that is not connected (`X39PHh`), Orchester Cloud while it cannot be reached (`ZSB6h`),
//! this copy's update while it needs the person or has news (`update`, `acFkz`),
//! whether agents may control the window and the MCP server unless the installed app's server listens, then the
//! dot. The project's name, its counts and the waiting requests show elsewhere (the switcher, the health card,
//! *Needs you* in the title bar), today's tokens on the sidebar's usage card. The sign-in gate has a bar of its own
//! ([`gate()`]). Every item shows only measured data. A screen outside a project shows the activity and the app's
//! items only, its events named by their project. A startup screen's bar has the version only.
//!
//! [`StatusBar`] gathers what the bar shows from the window ([`BarFacts`]) and does what its items ask
//! (`act`); [`StatusBarView`] draws it, so a test can hand it any state.

mod act;
mod activity;
mod auto_approving;
mod cloud;
mod facts;
mod gate;
mod health;
mod item;
mod mcp;
mod operations;
mod plan_limits;
mod recovery;
mod runner;
mod server;
mod update;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Spacer},
  core::ElementRef,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::plan_limits::PlanReadings, id::ProjectId};
use orchester_workspace::{
  activity::{
    Activity, ActivityLog,
    bar::{self, ActivitySlot},
  },
  recovery::RecoveryState,
  runner::state::RunnerState,
  tasks::outside::Outside,
};

use self::{
  act::Stopper,
  item::{give_way, glyph, item, label},
};
pub use self::{
  cloud::CloudFacts,
  facts::{AccountLimits, BarAction, BarFacts, Dispatch, ProjectFacts, ReadFailure, RunnerFacts, Stopping},
  gate::gate,
  item::dot,
  mcp::from_scratch,
  server::{ServerFacts, ServerState, server_facts},
  update::UpdateFacts,
};
use crate::{
  lifecycle,
  mcp::{McpLink, McpStatus},
  project::{ProjectFeed, ProjectState, Projects, TasksState},
  routes::Route,
  services::{
    Services,
    accounts::{self, Accounts},
    plan_limits as readings,
  },
  ui::icon::Icon,
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
    let link = ctx
      .use_context::<McpLink>()
      .expect("the root provides the MCP server's link before mounting the shell");
    let log = log(ctx, &services.activity);
    let mcp = mcp_status(ctx, &link);
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
    let readings = readings::readings(ctx);
    let listed = accounts::listed(ctx);
    // Read again when the project's connections changed, or an account moved them (its removal, a new folder).
    let changed = (
      shown
        .as_ref()
        .map(|(_, feed)| feed.outside.get().of(Outside::Connections)),
      accounts::changes(ctx, &services.workspace),
    );
    let used = used_accounts(ctx, shown.as_ref().map(|(facts, _)| facts.repo_root.clone()), changed);
    let servers = server::server_facts(
      ctx,
      &services.servers,
      shown.as_ref().map(|(facts, _)| facts.repo_root.clone()),
    );
    let cloud = cloud::cloud_facts(ctx, &services.cloud);
    let update = update::update_facts(ctx, &services.updates);
    let facts = BarFacts {
      project: shown.map(|(mut facts, _)| {
        facts.limits = limits(&listed, &readings, &used);
        facts
      }),
      log,
      mcp,
      stopping,
      servers,
      cloud,
      update,
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
    repo_root: open.repo_root,
    runner,
    tasks,
    read_failure,
    undecided_runs: recovery.undecided(project),
    limits: Vec::new(),
  };
  Some((facts, feed))
}

/// The accounts the project's connections run on, by id, read when the project shows and again when they
/// changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Used(Vec<orchester_core::id::AgentAccountId>);

impl DevtoolsInspectable for Used {}

fn used_accounts(ctx: &mut Ctx, repo_root: Option<std::path::PathBuf>, changed: (Option<u64>, u64)) -> Used {
  ctx
    .future((repo_root, changed), |(repo_root, _)| {
      if repo_root.is_some() {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "used accounts");
      }
      used_in(repo_root)
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The accounts the connections of the project at `repo_root` run on; none without a project.
async fn used_in(repo_root: Option<std::path::PathBuf>) -> Result<Used, orchester_workspace::messages::Message> {
  let Some(repo_root) = repo_root else {
    return Ok(Used::default());
  };
  orchester_workspace::background::run(move || {
    let connections = orchester_workspace::connections::data::ProjectConnections::load_in(&repo_root)
      .map_err(|error| orchester_workspace::messages::store_error(&error))?;
    let mut used: Vec<_> = connections
      .connections
      .iter()
      .filter_map(|connection| connection.account().id(connection.provider()))
      .collect();
    used.dedup();
    Ok::<_, orchester_workspace::messages::Message>(Used(used))
  })
  .await?
}

/// The used accounts' readings, in the order the accounts are listed (by CLI, Default first).
fn limits(listed: &Accounts, readings: &PlanReadings, used: &Used) -> Vec<AccountLimits> {
  listed
    .0
    .iter()
    .filter(|account| used.0.contains(&account.id()))
    .filter_map(|account| {
      Some(AccountLimits {
        account: account.clone(),
        reading: readings.of_account(account.id())?.clone(),
        source: readings.source_of(account.id())?,
      })
    })
    .collect()
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

/// The app's MCP server's status, whenever it changes.
pub fn mcp_status(ctx: &mut Ctx, link: &McpLink) -> McpStatus {
  let stream = ctx.stream(link.clone(), |link, emitter: StreamEmitter<McpStatus, ()>| async move {
    let mut changes = link.changes();
    loop {
      let status = changes.borrow_and_update().clone();
      if !emitter.emit(status) || changes.changed().await.is_err() {
        return;
      }
    }
  });
  stream.state().get().data.unwrap_or_else(|| link.status())
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
pub struct StatusBarView {
  /// The bar, which the MCP item's popover opens over.
  bar: ElementRef,
}

impl Component for StatusBarView {
  type Props = BarProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { bar: ctx.element_ref() }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let BarProps { facts, dispatch } = ctx.props::<BarProps>().clone();
    let project = facts.project.as_ref();
    let slot = bar::activity_slot(project.map(|project| &project.runner.state), &facts.log);
    let mut left = Vec::new();
    let mut right = Vec::new();
    // Online, the health dot stands for the runner; in any other state it says so first (`vNIo3`, `a7yNee`).
    if let Some(project) = project.filter(|project| project.runner.state != RunnerState::Online) {
      left.extend(runner::runner(ctx, &project.runner, &dispatch));
    }
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
      (None, None, ActivitySlot::Activity(slot)) => {
        left.extend(activity::activity(ctx, (slot, &facts.log), project, &dispatch));
      }
    }
    // The right group in the design's order (`18 · Several at once` `y0SDOa`), the dot always last.
    if let Some(tasks) = project.and_then(|project| project.tasks.as_ref()) {
      right.extend(auto_approving::auto_approving(
        ctx,
        &tasks.auto_approving,
        &facts.stopping,
        &dispatch,
      ));
    }
    if let Some(project) = project {
      right.extend(plan_limits::item(ctx, &project.limits, &self.bar));
    }
    right.extend(server::item(ctx, &facts.servers, &dispatch));
    right.extend(cloud::item(ctx, &facts.cloud, &dispatch));
    right.extend(update::item_of(ctx, &facts.update, &dispatch));
    right.extend(mcp::app_items(ctx, (&facts.mcp, &self.bar), &dispatch));
    right.extend(health::health(ctx, &facts, &dispatch));
    Row::new()
      .id("status-bar")
      .ref_element(self.bar.clone())
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

/// The status bar of a startup screen (`Status bar` `xmswq` of `N0Lpc`): the version only.
pub fn version_only(ctx: &mut Ctx) -> Element {
  let version = gate::version(ctx);
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
