//! What the Agents hub of a team project gives the person who may change the team's resources and the one who may not:
//! New in the head and Edit in the panel, and how its route stays on the resource shown (`Agents · v2` `acFkz`, A1–A6,
//! ORC-831). `cargo test -p orchester-app --test agents team_hub`.

use orchester_app::routes::{
  AgentTab, HubFilter, HubView, InstructionsTab, InstructionsView, ProjectPage, Route, ToolTab,
};
use orchester_core::{id::ResourceId, sharing::MemberRole};

use super::{
  harness::Agents,
  support::project_id,
  team_hub::{open, team, team_with_profile_and_tool},
};

fn at(page: ProjectPage) -> Route {
  Route::Project(project_id(), page)
}

fn hub(filter: HubFilter, item: Option<ResourceId>) -> Route {
  at(ProjectPage::hub(HubView::new(filter, item)))
}

#[test]
fn team_hub_gives_a_member_neither_new_nor_edit() {
  let mut agents = Agents::new(true);
  let (profile, tool) = team_with_profile_and_tool(&agents, None);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  assert_eq!(agents.text("team-hub-panel-title"), "Builder");
  assert!(!agents.exists("team-hub-new") && !agents.exists("team-hub-edit"));
  // Nothing disabled in their place: no bar either.
  assert!(!agents.exists("team-hub-actions"));
  open(&mut agents, HubView::new(HubFilter::Tools, Some(tool)));
  assert_eq!(agents.text("team-hub-panel-title"), "Checks");
  assert!(!agents.exists("team-hub-new") && !agents.exists("team-hub-edit"));
  open(&mut agents, HubView::default());
  assert!(!agents.exists("team-hub-new"));
}

#[test]
fn team_hub_edit_opens_the_editor_of_a_profile_and_of_a_tool() {
  let mut agents = Agents::new(true);
  let (profile, tool) = team_with_profile_and_tool(&agents, Some(MemberRole::Owner));
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.click("team-hub-edit");
  let editor = at(ProjectPage::Agent(profile, AgentTab::default()));
  agents.until("the profile's editor", |s| s.window.navigation().route_now() == editor);
  open(&mut agents, HubView::new(HubFilter::Tools, Some(tool)));
  agents.click("team-hub-edit");
  let editor = at(ProjectPage::Tool(tool, ToolTab::default()));
  agents.until("the tool's editor", |s| s.window.navigation().route_now() == editor);
}

#[test]
fn team_hub_new_opens_the_create_page_of_the_kind_the_filter_names() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pages = [
    (
      HubFilter::Profiles,
      "New agent",
      ProjectPage::NewAgent(AgentTab::default()),
    ),
    (
      HubFilter::Instructions,
      "New instruction",
      ProjectPage::Instructions(InstructionsView::new_rule(InstructionsTab::default())),
    ),
    (HubFilter::Skills, "New skill", ProjectPage::NewSkill),
    (HubFilter::Tools, "New tool", ProjectPage::NewTool),
  ];
  for (filter, label, page) in pages {
    // The filter may hold nothing (no profile is the team's here): the head is there all the same.
    agents.window.navigation().go(hub(filter, None));
    agents.until("the head", |s| {
      s.exists("team-hub-new") && s.text("team-hub-new") == label
    });
    agents.click("team-hub-new");
    let created = at(page);
    agents.until("the create page", |s| s.window.navigation().route_now() == created);
  }
}

#[test]
fn team_hub_has_no_new_for_a_kind_without_a_create_page_and_a_menu_of_the_others_under_all() {
  let mut agents = Agents::new(true);
  team(&agents);
  // An MCP server has no create page yet: no New, and nothing disabled in its place.
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  assert!(!agents.exists("team-hub-new"));
  open(&mut agents, HubView::default());
  assert_eq!(agents.text("team-hub-new"), "New");
  assert!(!agents.exists("team-hub-new-menu"));
  agents.click("team-hub-new");
  agents.until("the menu", |s| s.exists("team-hub-new-menu"));
  for kind in ["profiles", "instructions", "skills", "tools"] {
    assert!(agents.exists(&format!("team-hub-new-{kind}")), "{kind}");
  }
  assert!(!agents.exists("team-hub-new-mcp_servers"));
  agents.click("team-hub-new-skills");
  let created = at(ProjectPage::NewSkill);
  agents.until("New skill", |s| s.window.navigation().route_now() == created);
}

#[test]
fn team_hub_corrects_a_route_to_the_resource_it_shows_and_keeps_the_chosen_one_between_filters() {
  let mut agents = Agents::new(true);
  let [skill, _, github, _] = team(&agents);
  // A resource the filter does not hold: the first of the list shows, and the route says so.
  agents.window.navigation().go(hub(HubFilter::Skills, Some(github)));
  agents.until("corrected", |s| {
    s.window.navigation().route_now() == hub(HubFilter::Skills, Some(skill))
  });
  assert_eq!(agents.text("team-hub-panel-title"), "review-diff");
  // One that is gone.
  agents
    .window
    .navigation()
    .go(hub(HubFilter::All, Some(ResourceId::generate())));
  agents.until("corrected", |s| {
    s.window.navigation().route_now() == hub(HubFilter::All, Some(github))
  });
  // The chosen resource stays while the filter holds it, and the filter that does not leaves the first.
  open(&mut agents, HubView::new(HubFilter::All, Some(skill)));
  agents.click("team-hub-filter-skills");
  agents.until("kept", |s| {
    s.window.navigation().route_now() == hub(HubFilter::Skills, Some(skill))
  });
  agents.click("team-hub-filter-tools");
  agents.until("dropped", |s| {
    s.window.navigation().route_now() == hub(HubFilter::Tools, None)
  });
}

#[test]
fn a_local_project_keeps_the_profiles_list_on_agents() {
  let mut agents = Agents::new(true);
  agents.go();
  assert!(agents.exists("agents-new"));
  assert!(!agents.exists("screen-team-hub") && !agents.exists("team-hub-list"));
}
