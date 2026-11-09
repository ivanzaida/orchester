//! *Also in this profile's prompts* (`Also in this profile’s prompts` `d0vBCk` in 01c, new in ORC-107): the published
//! instructions and skills the profile gives its agents (`docs/design/team-configuration.md`, "Decided 2026-11-09"),
//! each opening its tab of the hub, where they are chosen in the profile's panel.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::AgentProfile;
use orchester_workspace::agents::data::ProjectAgents;

use crate::{
  navigation::Navigation,
  screens::hub::{
    HubTab,
    parts::{Entry, boxed, group, head},
  },
  ui::pressable::Press,
};

pub fn render(ctx: &mut Ctx, listed: &ProjectAgents, profile: &AgentProfile) -> Element {
  let prompts = listed.prompts.for_profile(profile);
  let none = ctx.t("agents.promptsNone");
  let names = |names: &[Arc<str>]| -> Arc<str> {
    if names.is_empty() {
      none.clone()
    } else {
      ctx.t_args("agents.promptsGiven", [("names", names.join(", "))])
    }
  };
  let (instructions, skills) = (names(&prompts.instructions), names(&prompts.skills));
  let navigation = Navigation::of(ctx);
  let project = ctx.props::<super::editor::Props>().project.open.project.id();
  let open = |tab: HubTab| Press::Open(navigation.to(tab.route(project)));
  let rows = [
    Entry::new("agent-prompts-instructions", ctx.t("agents.promptsInstructionsTitle"))
      .hint(instructions)
      .press(open(HubTab::Instructions))
      .elsewhere()
      .into(),
    Entry::new("agent-prompts-skills", ctx.t("agents.promptsSkillsTitle"))
      .hint(skills)
      .press(open(HubTab::Skills))
      .elsewhere()
      .into(),
  ];
  group(
    "agent-prompts",
    head("agent-prompts", &ctx.t("agents.promptsTitle"), None, None),
    boxed("agent-prompts-rows", rows),
  )
  .into()
}
