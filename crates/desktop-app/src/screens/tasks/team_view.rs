//! The Tasks screen of a team project (`team`): the session's query client provided to what it mounts, then while the
//! server's tasks are read the list's skeleton, once read the same header, views, rows and board as a local project's
//! with the team's lines under the header, and while the team's data cannot be read the outage in their place.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Text},
  node::Element,
};
use orchester_api::v2::MAX_PAGE_LIMIT;
use orchester_workspace::tasks::data::ProjectTasks;

use super::{
  Shown, TasksScreen,
  facets::Options,
  header, inset,
  kanban::display::Layout,
  queue,
  shared::Shared,
  team::{self, Team, TeamShown},
};
use crate::{
  project::ShownProject,
  routes::TaskList,
  services::Services,
  team::{TeamFailure, queries::PAGES},
  theme::ExtraTypography,
};

impl TasksScreen {
  /// The list of a team project, `local` being what this computer knows of its runs.
  pub(super) fn team(
    &self,
    ctx: &mut Ctx,
    page: Column,
    (project, list): (&ShownProject, &TaskList),
    (shared, local): (Option<&Shared>, &ProjectTasks),
  ) -> Column {
    let services = Services::of(ctx);
    ctx.provide_if_changed(services.team.client());
    let id = project.open.project.id();
    match team::read(ctx, (project, shared), (list, &self.choices), local) {
      Team::Reading => {
        // No New task until the server says the person may create one.
        let page = page.child(self.header(ctx, (id, None, true), (None, None)));
        let name = project.open.project.name().as_str();
        let label = ctx.t_args("tasks.loading", [("project", name)]);
        self.reading(ctx, page, (project, list), label)
      }
      Team::Failed(failure) => page
        .child(inset(header::title_and_search(ctx, &self.choices)))
        .child(inset(outage(ctx, (project, shared), &failure, local))),
      Team::Ready(team) => self.team_list(ctx, page, (project, list, shared), &team),
    }
  }

  /// The team's tasks as read: the header without what a member cannot do, the team's lines, and the views, rows or
  /// board.
  fn team_list(
    &self,
    ctx: &mut Ctx,
    page: Column,
    (project, list, shared): (&ShownProject, &TaskList, Option<&Shared>),
    team: &TeamShown,
  ) -> Column {
    let id = project.open.project.id();
    let shown = Shown {
      project,
      list,
      source: &team.tasks,
      shared: None,
      not_shared: false,
      team: Some(team),
    };
    let options = Arc::new(Options::of(ctx, &team.tasks));
    let chosen = self.choices.facets.get_untracked();
    let offered = options.offered(&chosen);
    if offered.count() != chosen.count() {
      self.choices.facets.set(offered);
    }
    let header = self.header(ctx, (id, None, team.member()), (Some(options.clone()), queue(project)));
    let place = shared.map_or("", |shared| {
      shared
        .sharing
        .link
        .address
        .as_deref()
        .unwrap_or_else(|| shared.sharing.server_name())
    });
    let board = self.display.get().layout == Layout::Board;
    let mut lines: Vec<Element> = Vec::new();
    // The board draws the team's line (01b `zv7b6`); the list does not (01 `QdL7N`).
    if board {
      lines.push(inset(team::team_line(ctx, id, (place, team.role))).into());
    }
    if team.member() {
      lines.push(inset(team::lock_line(ctx, team.admins.as_deref(), board)).into());
    }
    if team.more {
      let count = (PAGES * usize::from(MAX_PAGE_LIMIT)).to_string();
      let more = Text::new(&ctx.t_args("tasks.team.more", [("count", count)]))
        .id("tasks-team-more")
        .variant(ExtraTypography::Note.role());
      lines.push(inset(more).into());
    }
    team::actions::TeamActions::provide(ctx, &team.source, &self.team_actions);
    lines.extend(team::actions::refusal(ctx).map(|refusal| Element::from(inset(refusal))));
    let blocking = team::actions::blocking_dialog(ctx);
    self
      .list(ctx, page.child(header).with_children(lines), &shown, &options)
      .with_children(blocking)
  }
}

/// Why the team's data cannot be shown and what runs here; *Try again* opens the team's reads anew and asks the server
/// at once.
fn outage(
  ctx: &mut Ctx,
  (project, shared): (&ShownProject, Option<&Shared>),
  failure: &TeamFailure,
  local: &ProjectTasks,
) -> Column {
  let services = Services::of(ctx);
  let (team, servers) = (services.team, services.servers);
  let server = shared.map(|shared| shared.sharing.link.server);
  let retry = move || {
    team.renew();
    if let Some(server) = server {
      servers.retry_now(server);
    }
  };
  let name = project.open.project.name().as_str();
  team::outage::outage(ctx, (failure, shared, name), (&project.open.repo_root, local), retry)
}
