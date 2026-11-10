//! Who may press Create in a local project's forms (`Agents · v2` L3–L7, ORC-843): what adds to what agents may do (a
//! profile, an MCP server, a tool) is a person's press, an instruction and a skill are not, and a team project's New
//! profile is still its own page. `cargo test -p orchester-app --test agents local_hub_create_press`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{AgentTab, HubFilter, ProjectPage, Route};
use orchester_core::{resource::ResourceKind, store::ResourceStore};
use orchester_store::ProjectDb;

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
  agents.until("the refusal", |s| s.exists("team-new-person-only"));
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
  assert!(!agents.exists("team-new-person-only"));
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
  assert!(!agents.exists("team-new-person-only"));
}

/// A team project keeps the New profile page it has: it is not the hub's form, and an MCP client's press on its Create
/// still makes nothing.
#[test]
fn agents_new_profile_page_of_a_team_project_is_its_own_page_and_refuses_synthetic_input() {
  let mut agents = Agents::new(true);
  team(&agents);
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewAgent(AgentTab::default())));
  agents.until("the page's own editor", |s| s.exists("agent-role"));
  assert!(!agents.exists("local-hub-new-create"), "not the hub's form");
  agents.set("agent-role", "Reviewer");
  agents.set("agent-instructions", "Review the implementation.");
  agents.set("agent-custom-model", "test-model");
  with_synthetic_input(|| agents.click("agent-create"));
  agents.until("the refusal shown", |s| s.exists("agent-save-error"));
  assert!(agents.exists("agent-create"), "the profile is still unsaved");
  assert_eq!(
    ProjectDb::open(&agents.root)
      .expect("database")
      .resources(ResourceKind::AgentProfile)
      .expect("resources")
      .len(),
    0,
    "automation does not publish what the role is told"
  );
}
