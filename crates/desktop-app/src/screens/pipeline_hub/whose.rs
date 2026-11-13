//! Whose pipelines the hub shows. At the head's right it says *Local* for a local project and *Team* for a team's, with
//! *· member* for whoever is not its admin, as the Agents hub does. Over a team's list a line says that what is saved
//! goes to the team server at once, as the version everyone's runs use, or, to a member, who changes the pipelines
//! (D-043). A member reads the pipelines: no *New pipeline*, no ⋯ menu and no Save bar.

use lurq::{app::ctx::Ctx, node::Element};

use super::model::Listing;
use crate::{
  project::ShownProject,
  screens::{
    hub::team,
    local_hub::head,
    team_hub::head::{self as team_head, Reach},
  },
};

/// How the project stands, at the head's right; nothing while the pipelines are read for the first time.
pub(super) fn status(ctx: &Ctx, listing: Option<&Listing>) -> Vec<Element> {
  match listing.map(Listing::team) {
    Some(Some(team)) => team_head::status(ctx, team.may_change, Reach::Answered),
    Some(None) => head::status(ctx),
    None => Vec::new(),
  }
}

/// The line over a team's pipelines; none for a local project, and none until the project's sharing is read.
pub(super) fn notices(ctx: &Ctx, project: &ShownProject, listing: Option<&Listing>) -> Vec<Element> {
  let shared = listing.is_some_and(|listing| listing.team().is_some());
  team::of(project)
    .filter(|_| shared)
    .map(|team| team::line(ctx, "pipeline-hub-team-line", &team).into())
    .into_iter()
    .collect()
}
