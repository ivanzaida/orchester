//! A team project's Delivery (`docs/plan/orchestrator-autonomy/OA-36-…`): the same figures, built from the tasks and
//! modules its team server holds, read through the session's queries (`crate::team::queries`), and this computer's
//! runs of those tasks (`orchester_workspace::delivery::Delivery::read_team`). Work a teammate's computer merged is not
//! known here, and the server keeps no history of a task's workflow status, so there is no cumulative flow; a line
//! under the toolbar says both. While the server cannot be read, the team's outage card says why
//! (`crate::team::outage`), and nothing read before is shown as current.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{module::Module, task::Task};

use crate::{
  project::ShownProject,
  screens::usage::card::footnote,
  team::{
    TeamFailure, outage,
    queries::{TaskListKey, team_modules, team_tasks},
    view,
  },
};

/// The team server's tasks and modules a team project's delivery is built from; the same reading is the same input.
#[derive(Clone, Debug)]
pub struct TeamInput {
  pub tasks: Arc<Vec<Task>>,
  pub modules: Arc<Vec<Module>>,
  /// The server holds more tasks than one listing reads: the figures count the ones read.
  pub more: bool,
}

impl PartialEq for TeamInput {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.tasks, &other.tasks) && Arc::ptr_eq(&self.modules, &other.modules) && self.more == other.more
  }
}

/// What a team project's delivery is built from, as far as it is read.
pub enum TeamRead {
  Reading,
  Failed(TeamFailure),
  Ready(TeamInput),
}

/// The tasks and modules of the team project `project`, read from its server.
pub fn read(ctx: &mut Ctx, project: &ShownProject) -> TeamRead {
  view::provide_client(ctx);
  let source = match view::source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return TeamRead::Reading,
    Err(failure) => return TeamRead::Failed(failure),
  };
  let tasks = ctx.query(team_tasks(source.clone(), TaskListKey::default()));
  let modules = ctx.query(team_modules(source));
  let failure = [tasks.error(), modules.error()].into_iter().flatten().next();
  if let Some(failure) = failure {
    return TeamRead::Failed((*failure).clone());
  }
  match (tasks.data(), modules.data()) {
    (Some(tasks), Some(modules)) => TeamRead::Ready(TeamInput {
      tasks: tasks.tasks.clone(),
      modules,
      more: tasks.more,
    }),
    _ => TeamRead::Reading,
  }
}

/// The outage card for `failure` in place of the figures.
pub fn unread(ctx: &mut Ctx, project: &ShownProject, failure: &TeamFailure) -> Element {
  let place = view::place(ctx, project);
  let unread = outage::Unread {
    id: "delivery-team-unavailable",
    title: ctx.t("delivery.team.unavailableTitle"),
    unavailable: "delivery.team.unavailableBody",
  };
  outage::card(ctx, &unread, (failure, &place, project.open.project.name().as_str())).into()
}

/// What a team project's figures count, and, when the server holds more tasks than were read, that only those count.
pub fn lines(ctx: &Ctx, input: &TeamInput) -> Vec<Element> {
  let mut lines = vec![footnote("delivery-team-counts", &ctx.t("delivery.team.counts"))];
  if input.more {
    let count = input.tasks.len().to_string();
    lines.push(footnote(
      "delivery-team-more",
      &ctx.t_args("delivery.team.more", [("count", count.as_str())]),
    ));
  }
  lines
}
