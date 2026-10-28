//! What stands under the Overview's header while something outside the page is wrong (05 `JzNRO`): the runner away
//! (information: it reconnects on its own, and what was read stays), the runner that gave up restarting (the failure
//! card's form with Restart runner and Open runner log, as the status bar offers them), and a shared project whose
//! team-server sign-in ended (*Join again*, which opens the server in Settings › Orchester). Each task whose latest run
//! can't be read keeps a row of its own with Open log and a link to the task.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  messages,
  runner::state::RunnerState,
  tasks::{data::ProjectTasks, list::Latest},
};

use super::section::link;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchesterView, ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  shell::status_bar::{ServerFacts, ServerState, server_facts},
  ui::{
    button::{ActionButton, ButtonKind},
    failure::Failure,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// The runner's notice: away while it reconnects, or failed once it gave up.
pub(super) fn runner(ctx: &mut Ctx, project: &ShownProject) -> Option<Element> {
  let root = project.open.repo_root.clone();
  let state = project.feed.status.get()?.state_for(&root);
  match state {
    RunnerState::Reconnecting { .. } => Some(
      Notice::new(
        "overview-runner-away",
        NoticeTone::Info,
        ctx.t("overview.runner.awayTitle"),
      )
      .icon(Icon::WifiOff)
      .body(ctx.t("overview.runner.awayBody"))
      .into(),
    ),
    RunnerState::Failed { cause, .. } => {
      let reason = message_text(ctx, &messages::runner_failure(&cause));
      let body = ctx.t_args("overview.runner.body", [("reason", reason)]);
      let runners = Services::of(ctx).runners;
      let log = runners.clone();
      let restart = ActionButton::new(
        "overview-runner-restart",
        ButtonKind::Secondary,
        ctx.t("overview.runner.restart"),
      )
      .icon(Icon::RotateCcw)
      .on_click(move || runners.restart());
      let open_log = ActionButton::new(
        "overview-runner-log",
        ButtonKind::Ghost,
        ctx.t("overview.runner.openLog"),
      )
      .icon(Icon::FileText)
      .on_click(move || log.open_log_of(&root));
      Some(
        Notice::new(
          "overview-runner-notice",
          NoticeTone::Danger,
          ctx.t("overview.runner.title"),
        )
        .icon(Icon::CircleAlert)
        .body(body)
        .action(restart)
        .action(open_log)
        .into(),
      )
    }
    _ => None,
  }
}

/// The team server's notice, for a shared project whose sign-in there ended.
pub(super) fn server(ctx: &mut Ctx, project: &ShownProject, shared: bool) -> Option<Element> {
  shared.then(|| {
    let props = ServerProps {
      repo_root: project.open.repo_root.clone(),
      name: project.open.project.name().as_str().into(),
    };
    ctx.mount_keyed::<ServerNotice>("overview-server-notice", props)
  })
}

#[derive(Clone, PartialEq)]
struct ServerProps {
  repo_root: PathBuf,
  name: Box<str>,
}

impl DevtoolsInspectable for ServerProps {}

/// Follows the project's server as the status bar does, and speaks only while its sign-in ended.
struct ServerNotice;

impl Component for ServerNotice {
  type Props = ServerProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ServerProps { repo_root, name } = ctx.props::<ServerProps>().clone();
    let servers = Services::of(ctx).servers;
    let ServerFacts::One {
      id,
      name: server,
      state: ServerState::SignedOut,
    } = server_facts(ctx, &servers, Some(repo_root))
    else {
      return Element::new();
    };
    let join = ActionButton::new(
      "overview-server-join",
      ButtonKind::Secondary,
      ctx.t("overview.server.join"),
    )
    .icon(Icon::LogIn)
    .opens(Navigation::of(ctx).to(Route::Orchester(OrchesterView::server(id))));
    Notice::new(
      "overview-server-notice",
      NoticeTone::Info,
      ctx.t_args("overview.server.title", [("server", server)]),
    )
    .icon(Icon::CloudOff)
    .body(ctx.t_args("overview.server.body", [("project", &*name)]))
    .action(join)
    .into()
  }
}

/// A row for each task whose latest run can't be read, with Open log and *Open task ↗*, which opens the task.
pub(super) fn unreadable(
  ctx: &mut Ctx,
  (project, root): (ProjectId, &std::path::Path),
  tasks: &ProjectTasks,
) -> Vec<Element> {
  tasks
    .tasks
    .iter()
    .filter_map(|task| match tasks.phases.get(&task.id())? {
      Latest::Unreadable(unreadable) => Some((task.key(), unreadable.title())),
      Latest::Run(_) => None,
    })
    .map(|(task_key, error)| {
      let key = task_key.to_string();
      let id = format!("overview-unreadable-{key}");
      let row = Failure::new(id.clone(), ctx.t("overview.unreadable.title"), error)
        .log(root.to_path_buf())
        .row(ctx, Some(&key));
      let route = Route::Project(
        project,
        ProjectPage::Task(task_key.clone(), TaskView::of(TaskTab::default())),
      );
      let open = Navigation::of(ctx).to(route);
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(row))
        .child(link(
          &format!("{id}-open"),
          &ctx.t("overview.unreadable.open"),
          Press::Open(open),
        ))
        .into()
    })
    .collect()
}
