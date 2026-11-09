//! The hub head of a profile's page: New agent at its right. The project's profiles are listed by the
//! Agents page (`screens::local_hub`).
use lurq::app::ctx::Ctx;
use orchester_workspace::{agents::data::ProjectAgents, sharing::team::TeamConfiguration};

use super::{AgentsNav, Shown};
use crate::{
  routes::AgentTab,
  screens::hub::{Head, team},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// New agent, at the hub head's right; none for a member of a shared project.
pub(super) fn head(ctx: &Ctx, (listed, team): (&ProjectAgents, Option<&TeamConfiguration>), nav: &AgentsNav) -> Head {
  if !team::may_change(team) {
    return Head::default();
  }
  Head::actions([new_button(ctx, "agents-new", ButtonKind::Primary, listed, nav).into()])
}

/// New agent while the profiles are read (`KHDar`): New agent opens a new profile, whose page says
/// what it needs once the project's connections are read.
pub(super) fn reading_head(ctx: &Ctx, nav: &AgentsNav) -> Head {
  let new = ActionButton::new("agents-new", ButtonKind::Primary, ctx.t("agents.new"))
    .icon(Icon::Plus)
    .opens(nav.to(Shown::New(AgentTab::default())));
  Head::actions([new.into()])
}

fn new_button(ctx: &Ctx, id: &str, kind: ButtonKind, listed: &ProjectAgents, nav: &AgentsNav) -> ActionButton {
  let button = ActionButton::new(id, kind, ctx.t("agents.new")).icon(Icon::Plus);
  if listed.connections.is_empty() {
    button.unavailable(ctx.t("agents.needsConnection"))
  } else {
    button.opens(nav.to(Shown::New(AgentTab::default())))
  }
}
