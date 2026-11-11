//! Who may press Create in a local project's forms (`Agents · v2` L3–L7, ORC-843): what adds to what agents may do (a
//! profile, an MCP server, a tool) is a person's press, an instruction and a skill are not, and a team project's New
//! profile is the team hub's form. `cargo test -p orchester-app --test agents local_hub_create_press`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{AgentTab, HubFilter, ProjectPage, Route};
use orchester_core::resource::ResourceKind;

use super::{
  harness::Agents,
  local_hub_create::{held, show_form},
  support::project_id,
  team_hub::team,
};

#[test]
fn local_hub_create_a_tool_is_a_persons_press() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Tools);
  agents.set("team-new-tool-name-input", "cargo-check");
  agents.set(
    "team-new-tool-program-input",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  with_synthetic_input(|| agents.click("local-hub-new-create"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  assert_eq!(
    held(&agents, ResourceKind::Tool),
    0,
    "an MCP client's press made no tool"
  );
}

#[test]
fn local_hub_create_an_instruction_is_not_refused_to_synthetic_input() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Instructions);
  agents.set("team-new-instruction-name-input", "Handoff notes");
  agents.set(
    "team-new-instruction-text-input",
    "Write a handoff note at the end of every run.",
  );
  with_synthetic_input(|| agents.click("local-hub-new-create"));
  agents.until("the instruction is made", |s| held(s, ResourceKind::Instruction) == 1);
  assert_eq!(agents.refused(), None);
}

#[test]
fn local_hub_create_a_skill_is_not_refused_to_synthetic_input() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Skills);
  agents.set("team-new-skill-name-input", "review-diff");
  agents.set(
    "team-new-skill-description-input",
    "Review a diff for the project's rules.",
  );
  agents.set(
    "team-new-skill-skill-md-input",
    "# review-diff\n\nRead the diff file by file.",
  );
  with_synthetic_input(|| agents.click("local-hub-new-create"));
  agents.until("the skill is made", |s| held(s, ResourceKind::Skill) == 1);
  assert_eq!(agents.refused(), None);
}

/// A team project's New profile page is the team hub's form (A7), not the old page of its own nor a local hub's form:
/// a team's resources are the hub's alone.
#[test]
fn agents_new_profile_page_of_a_team_project_is_the_team_hubs_form() {
  let mut agents = Agents::new(true);
  team(&agents);
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewAgent(AgentTab::default())));
  agents.until("the team hub's form", |s| s.exists("team-hub-new-create"));
  assert!(!agents.exists("agent-role"), "not the old page");
  assert!(!agents.exists("local-hub-new-create"), "not a local hub's form");
}
