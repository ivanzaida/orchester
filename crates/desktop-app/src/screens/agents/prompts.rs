//! *Also in this profile's prompts* (`Also in this profile’s prompts` `d0vBCk` in 01c, new in ORC-107): the published
//! project rules that select its agents, its role's rules and the project and role skills, each opening its tab of
//! the hub.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{id::ResourceId, name::Name};
use orchester_workspace::agents::data::ProjectAgents;

use crate::{
  navigation::Navigation,
  screens::hub::{
    HubTab,
    parts::{Entry, boxed, group, head},
  },
  ui::pressable::Press,
};

pub fn render(ctx: &mut Ctx, listed: &ProjectAgents, (id, role): (ResourceId, &Name)) -> Element {
  let prompts = listed.prompts.for_profile(id, role);
  let none = ctx.t("agents.promptsNone");
  let names = |names: &[Arc<str>]| names.join(", ");
  let rules = if prompts.project_rules.is_empty() {
    none.clone()
  } else {
    ctx.t_args("agents.promptsProject", [("names", names(&prompts.project_rules))])
  };
  let role_rules = if prompts.role_rules.is_empty() {
    none.clone()
  } else {
    ctx.t_args(
      "agents.promptsRole",
      [
        ("names", names(&prompts.role_rules)),
        ("role", role.as_str().to_owned()),
      ],
    )
  };
  let mut skills = Vec::new();
  if !prompts.project_skills.is_empty() {
    skills.push(ctx.t_args(
      "agents.promptsSkillsProject",
      [("names", names(&prompts.project_skills))],
    ));
  }
  if !prompts.role_skills.is_empty() {
    skills.push(ctx.t_args("agents.promptsSkillsRole", [("names", names(&prompts.role_skills))]));
  }
  let skills: Arc<str> = if skills.is_empty() {
    none
  } else {
    skills
      .iter()
      .map(|part| part.to_string())
      .collect::<Vec<_>>()
      .join("; ")
      .into()
  };
  let navigation = Navigation::of(ctx);
  let project = ctx.props::<super::editor::Props>().project.open.project.id();
  let open = |tab: HubTab| Press::Open(navigation.to(tab.route(project)));
  let rows = [
    Entry::new("agent-prompts-project", ctx.t("agents.promptsProjectTitle"))
      .hint(rules)
      .press(open(HubTab::Instructions))
      .elsewhere()
      .into(),
    Entry::new("agent-prompts-role", ctx.t("agents.promptsRoleTitle"))
      .hint(role_rules)
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
