//! What the Agents page shows while it cannot yet tell a team project from a local one: the first read of the team's
//! configuration is under way or failed. A project's list is the team's or the local one by that read, so the page
//! shows neither before it (`Failure states · redesign` `SsC16`).

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use crate::{
  project::ShownProject,
  theme::HubSpacing,
  ui::{failure::Failure, state::loading},
};

/// The page while the first read is under way.
pub fn reading(ctx: &Ctx) -> Element {
  page(loading("team-hub-team-loading", &ctx.t("team_hub.reading")).into())
}

/// The page when the first read failed: why, with Try again, which reads it anew.
pub fn failed(ctx: &mut Ctx, project: &ShownProject, error: Message) -> Element {
  let retries = project.feed.team_retries.clone();
  let card = Failure::new("team-hub-team-error", ctx.t("team_hub.loadFailed"), error)
    .retry(false, move || retries.update(|n| *n += 1))
    .log(project.open.repo_root.clone())
    .still_works(ctx.t("hub.failure.stillWorks"))
    .page(ctx);
  page(card)
}

fn page(body: Element) -> Element {
  Column::new()
    .id("screen-team-hub-unread")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Side.role(), HubSpacing::HeadTop.role()))
    .child(body)
    .into()
}
