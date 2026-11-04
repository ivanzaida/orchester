//! How a view reads its team project: the session's query client provided to what it mounts, the project's
//! [`TeamSource`] in the session's current generation, read again when the generation or a server changes, the
//! person's standing there as the server says it ([`Standing`]), never a role kept on this computer, and where the
//! project lives, for what an outage says ([`place`]).

use std::sync::Arc;

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_api::v2::CallerRole;
use orchester_workspace::{background, sharing::project::TeamAdmins};

use super::{
  TeamFailure, TeamQueries, TeamSource,
  outage::Place,
  queries::{team_admins, team_capabilities},
};
use crate::{project::ShownProject, screens::settings::orchester::follow, services::Services};

/// Provides the session's current query client to what the view mounts.
pub fn provide_client(ctx: &mut Ctx) {
  let services = Services::of(ctx);
  ctx.provide_if_changed(services.team.client());
}

/// The project's server and its name, read again whenever a server answers; `None` while it is read, when it cannot be,
/// and for a local project.
pub fn place(ctx: &mut Ctx, project: &ShownProject) -> Place {
  let services = Services::of(ctx);
  let revision = follow(ctx, (&services.servers, &services.cloud));
  let root = project.open.repo_root.clone();
  ctx
    .future((revision, root, services.servers), |(_, root, servers)| async move {
      match background::run(move || servers.project_sharing(&root)).await {
        Ok(Ok(sharing)) => Ok(sharing.map(|sharing| (sharing.link.server, sharing.server_name().to_owned()))),
        Ok(Err(error)) => {
          // The page says the team's data cannot be read either way; only the server's name is left out.
          tracing::warn!(%error, "a team project's server cannot be read");
          Err(())
        }
        Err(_) => Err(()),
      }
    })
    .state()
    .get()
    .data
    .flatten()
}

/// The project's source in the session's current generation, read again when the generation or a server changes;
/// `Ok(None)` until it is read, and for a local project.
pub fn source(ctx: &mut Ctx, project: &ShownProject) -> Result<Option<TeamSource>, TeamFailure> {
  let services = Services::of(ctx);
  let generation = generation(ctx, &services.team);
  let revision = follow(ctx, (&services.servers, &services.cloud));
  let root = project.open.repo_root.clone();
  let team = services.team.clone();
  let state = ctx
    .future((root, generation, revision), move |(root, ..)| {
      let team = team.clone();
      async move { team.source(&root).await }
    })
    .state()
    .get();
  let pending = state.is_pending();
  match (state.data, state.error) {
    (_, Some(failure)) if !pending => Err(failure),
    (Some(Some(source)), _) if source.key().generation == generation => Ok(Some(source)),
    _ => Ok(None),
  }
}

/// The session's team generation, read again when it ends.
fn generation(ctx: &mut Ctx, team: &TeamQueries) -> u64 {
  let stream = ctx.stream(team.clone(), |team, emitter: StreamEmitter<u64, ()>| async move {
    let mut generations = team.subscribe();
    loop {
      let generation = *generations.borrow_and_update();
      if !emitter.emit(generation) || generations.changed().await.is_err() {
        return;
      }
    }
  });
  stream.state().get().data.unwrap_or_else(|| team.generation())
}

/// The person's standing in a team project: their role as the server's capabilities say it, and for a member who
/// changes the team's tasks, modules and tags, once the members are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
  pub role: CallerRole,
  pub admins: Option<Arc<TeamAdmins>>,
}

impl Standing {
  /// A member changes nothing: only Admins and Server admins do.
  pub fn member(&self) -> bool {
    self.role == CallerRole::Member
  }
}

/// The person's standing in `source`'s project; `Ok(None)` while it is read.
pub fn standing(ctx: &mut Ctx, source: &TeamSource) -> Result<Option<Standing>, TeamFailure> {
  let capabilities = ctx.query(team_capabilities(source.clone()));
  if let Some(failure) = capabilities.error() {
    return Err((*failure).clone());
  }
  let Some(capabilities) = capabilities.data() else {
    return Ok(None);
  };
  let admins = (capabilities.role == CallerRole::Member)
    .then(|| ctx.query(team_admins(source.clone())).data())
    .flatten();
  Ok(Some(Standing {
    role: capabilities.role,
    admins,
  }))
}
