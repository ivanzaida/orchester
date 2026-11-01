//! The steps after a team is joined (`docs/design/startup.md`, decisions 5–6): the team's shared projects the person is
//! invited to ([`projects`], 05 `v5qbWM`, none yet 12 `NaEPI`), and linking the one chosen to a folder ([`link`], 06
//! `go2aM`, not a repository 13 `sN8Pa`, another `origin` 13b `tDFeg`). Both read the same thing here. A failure to
//! read is a danger notice with *Try again*, never an empty list.

mod link;
mod projects;

use std::sync::Arc;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::id::ServerId;
use orchester_workspace::{
  background,
  sharing::{
    ServerEntry, ServerProject, Servers,
    cloud::Cloud,
    team_join::{JoinError, JoinRefusal, TeamJoin},
  },
};

pub(super) use self::{link::screen as link_screen, projects::screen as projects_screen};
use super::frame::Crumb;
use crate::{
  routes::{JoinWay, Route},
  screens::settings::orchester::{
    Said,
    connect::{Shown, join_error},
    sharing_said,
  },
};

/// The screen's name in element ids, as the step's other pages have it.
const ID: &str = "join_team";

/// A joined team and the person's projects on it, as read. A read is compared by itself: each one replaces the last.
#[derive(Clone, Debug)]
struct Team(Arc<(ServerEntry, Vec<ServerProject>)>);

impl PartialEq for Team {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Team {}

impl Team {
  fn entry(&self) -> &ServerEntry {
    &self.0.0
  }

  fn projects(&self) -> &[ServerProject] {
    &self.0.1
  }
}

/// `server` and the person's projects on it; none is an answer here, which 12 `NaEPI` shows, not a refusal.
async fn read(servers: Servers, cloud: Cloud, server: ServerId) -> Result<Team, Shown> {
  let all = servers.clone();
  let entry = background::run(move || all.entries())
    .await
    .map_err(Shown::Message)?
    .map_err(|error| Shown::Said(sharing_said(&error)))?
    .into_iter()
    .find(|entry| entry.stored.id == server && entry.in_use())
    .ok_or_else(|| Shown::Said(Said::new("sharing.join.noServer")))?;
  let projects = match TeamJoin::new(servers, cloud).projects(server).await {
    Ok(projects) => projects,
    Err(JoinError::Refused(JoinRefusal::NoProjects { .. })) => Vec::new(),
    Err(error) => {
      tracing::warn!(%error, "a team's shared projects could not be read");
      return Err(join_error(&error));
    }
  };
  Ok(Team(Arc::new((entry, projects))))
}

/// *Join a team* in the breadcrumb.
fn join_crumb(ctx: &Ctx) -> Crumb {
  Crumb {
    name: "join",
    label: ctx.t("startup.join.crumb").to_string(),
    route: Route::JoinTeam(JoinWay::Cloud),
  }
}
