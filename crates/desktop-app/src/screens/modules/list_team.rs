//! The Modules list of a team project (`01d · Modules · member` `rvXxg`, ORC-717): the workspace's modules read from
//! its server through the session's lurq queries (`crate::team::pages`), with their progress and forecast as the server
//! counts them, each row naming who holds the module (`Holder` `Ju7rK`). A member has no New module and reads, over the
//! rows, one calm line naming who changes them (`Read-only line` `AEKrF`: 2 over it, 10 under it); the standing is the
//! server's. While the team's data cannot be read, the outage card stands in place of the rows (`crate::team::outage`).

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, padding::Padding},
};
use orchester_workspace::modules::data::ModuleSummary;

use super::list::{Forecasts, ModulesList, Rows, padded, reading};
use crate::{
  project::ShownProject,
  routes::ModuleListTab,
  team::{
    TeamFailure, lock, outage,
    pages::{TeamModule, team_module_list},
    view::{self, Standing},
  },
  theme::ExtraSpacing,
};

/// What the list shows of a team project.
enum Team {
  Reading,
  Failed(TeamFailure),
  Ready(Arc<Vec<TeamModule>>, Standing),
}

impl ModulesList {
  /// The list of the team project `project` on `tab`.
  pub(super) fn team(&self, ctx: &mut Ctx, project: &ShownProject, tab: ModuleListTab) -> Column {
    view::provide_client(ctx);
    let place = view::place(ctx, project);
    match read(ctx, project) {
      Team::Reading => {
        let header = self.header(ctx, (project, tab), None, (true, false));
        self.screen(header, reading(ctx))
      }
      Team::Failed(failure) => {
        let header = self.header(ctx, (project, tab), None, (false, false));
        let unread = outage::Unread {
          id: "modules-team-unavailable",
          title: ctx.t("modules.team.unavailable.title"),
          unavailable: "modules.team.unavailable.body",
        };
        let name = project.open.project.name().as_str();
        let card = outage::card(ctx, &unread, (&failure, &place, name));
        self.screen(header, padded(card.into()))
      }
      Team::Ready(modules, standing) => {
        let listed: Vec<ModuleSummary> = modules.iter().map(summary).collect();
        let forecasts = modules
          .iter()
          .filter_map(|module| Some((module.module.id(), module.forecast.clone()?)))
          .collect();
        let holders = modules
          .iter()
          .map(|module| {
            (
              module.module.id(),
              module.claim.as_ref().map(|claim| claim.holder.clone()),
            )
          })
          .collect::<HashMap<_, _>>();
        let member = standing.member();
        let rows = Rows {
          lock: member.then(|| lock_line(ctx, &standing)),
          holders: Some(holders),
          creates: !member,
        };
        let header = self.header(ctx, (project, tab), Some(&listed), (false, !member));
        let body = self.body(
          ctx,
          (project, tab),
          &listed,
          (Some(&Forecasts(Arc::new(forecasts))), rows),
        );
        self.screen(header, body)
      }
    }
  }
}

/// The project's modules and the person's standing, as the server reads them.
fn read(ctx: &mut Ctx, project: &ShownProject) -> Team {
  let source = match view::source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return Team::Reading,
    Err(failure) => return Team::Failed(failure),
  };
  let standing = match view::standing(ctx, &source) {
    Ok(Some(standing)) => standing,
    Ok(None) => return Team::Reading,
    Err(failure) => return Team::Failed(failure),
  };
  let modules = ctx.query(team_module_list(source));
  if let Some(failure) = modules.error() {
    return Team::Failed((*failure).clone());
  }
  modules
    .data()
    .map_or(Team::Reading, |modules| Team::Ready(modules, standing))
}

/// A team module as the list's rows read a module.
fn summary(module: &TeamModule) -> ModuleSummary {
  ModuleSummary {
    module: module.module.clone(),
    progress: module.progress,
    tasks: usize::try_from(module.tasks).unwrap_or(usize::MAX),
  }
}

/// *Only Admins create and change modules: dana-k (Admin) and olha (Server admin). You claim a module to work on all
/// its tasks.*
fn lock_line(ctx: &Ctx, standing: &Standing) -> Element {
  let words = lock::lock_words(
    ctx,
    standing.admins.as_deref(),
    ("modules.team.readOnly", "modules.team.readOnlyUnnamed"),
  );
  lock::locked("modules-team-read-only", &words)
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Loose.role()),
    )
    .into()
}
