//! The hub head of a profile's page: New agent at its right. The project's profiles are listed by the
//! Agents page (`screens::local_hub`).
use lurq::app::ctx::Ctx;
use orchester_workspace::agents::data::ProjectAgents;

use super::{AgentsNav, Shown};
use crate::{
  routes::AgentTab,
  screens::hub::Head,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// New agent, at the hub head's right.
pub(super) fn head(ctx: &Ctx, listed: &ProjectAgents, nav: &AgentsNav) -> Head {
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
