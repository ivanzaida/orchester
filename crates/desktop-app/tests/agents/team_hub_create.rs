//! The create forms of the Agents hub (`Agents · v2` `acFkz`, A7–A12, ORC-835): New in the head opens the kind's form
//! in the panel as a page, with only Create and Cancel (no draft: Create makes v1 and sends it to the team server, and
//! nothing shows as created before the server takes it). The team's part and what each computer fills in afterwards
//! stand apart. `cargo test -p orchester-app --test agents team_hub_create`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{resource::ResourceKind, sharing::MemberRole, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  support::project_id,
  team_hub::{open, team, team_as},
};

/// The kinds that have a form: their filter, the head's New, the form's title and what its rows are named after.
const KINDS: [(HubFilter, &str, &str, &str, ResourceKind); 5] = [
  (
    HubFilter::Profiles,
    "New agent",
    "New agent profile",
    "profile",
    ResourceKind::AgentProfile,
  ),
  (
    HubFilter::McpServers,
    "New MCP server",
    "New MCP server",
    "mcp",
    ResourceKind::McpServer,
  ),
  (
    HubFilter::Instructions,
    "New instruction",
    "New instruction",
    "instruction",
    ResourceKind::Instruction,
  ),
  (
    HubFilter::Skills,
    "New skill",
    "New skill",
    "skill",
    ResourceKind::Skill,
  ),
  (HubFilter::Tools, "New tool", "New tool", "tool", ResourceKind::Tool),
];

fn form_route(filter: HubFilter) -> Route {
  Route::Project(project_id(), ProjectPage::hub(HubView::adding(filter)))
}

fn hub_route(filter: HubFilter) -> Route {
  Route::Project(project_id(), ProjectPage::hub(HubView::new(filter, None)))
}

fn show_form(agents: &mut Agents, filter: HubFilter) {
  agents.window.navigation().go(form_route(filter));
  agents.until("the form", |s| s.exists("team-hub-new-create"));
}

/// How many resources of `kind` this computer holds, and how many versions wait for the team server.
fn held(agents: &Agents, kind: ResourceKind) -> (usize, usize) {
  let db = ProjectDb::open(&agents.root).expect("database");
  (
    db.resources(kind).expect("resources").len(),
    db.waiting_publishes().expect("the outbox").len(),
  )
}

#[test]
fn team_hub_create_new_opens_the_form_of_each_kind_in_the_panel_with_only_create_and_cancel() {
  let mut agents = Agents::new(true);
  team(&agents);
  for (filter, new, title, name, _) in KINDS {
    agents.window.navigation().go(hub_route(filter));
    agents.until("the head", |s| {
      s.exists("team-hub-new") && s.text("team-hub-new") == new
    });
    agents.click("team-hub-new");
    agents.until("the form", |s| s.window.navigation().route_now() == form_route(filter));
    agents.until("the form", |s| s.exists("team-hub-new-create"));
    // A page in the panel, not a window: the new resource is the list's first row, chosen, and the panel is its form.
    assert_eq!(agents.text("team-hub-panel-title"), title, "{name}");
    assert_eq!(agents.text("team-hub-row-new-name"), title, "{name}");
    assert_eq!(agents.text("team-hub-row-new-line"), "Not created yet", "{name}");
    assert_eq!(
      agents.text("team-hub-panel-version"),
      "not created yet · Create publishes v1",
      "{name}"
    );
    // The team's part and what a computer fills in afterwards are shown apart.
    assert_eq!(agents.text("team-new-team-label"), "For the team", "{name}");
    assert_eq!(
      agents.text("team-new-team-hint"),
      "· goes to the team's server on Create",
      "{name}"
    );
    assert!(agents.exists("team-new-computer"), "{name}");
    // Only Create and Cancel: no draft, no save status.
    assert_eq!(agents.text("team-hub-new-create"), "Create", "{name}");
    assert_eq!(agents.text("team-hub-new-cancel"), "Cancel", "{name}");
    assert_eq!(
      agents.text("team-hub-actions-hint"),
      "Create publishes v1 for the team once the server accepts it",
      "{name}"
    );
    assert!(
      !agents.exists("team-hub-edit") && !agents.exists("agent-save-status"),
      "{name}"
    );
  }
}

#[test]
fn team_hub_create_under_all_a_menu_offers_every_kind_with_a_form() {
  let mut agents = Agents::new(true);
  team(&agents);
  open(&mut agents, HubView::default());
  agents.click("team-hub-new");
  agents.until("the menu", |s| s.exists("team-hub-new-menu"));
  for (filter, ..) in KINDS {
    assert!(agents.exists(&format!("team-hub-new-{}", filter.name())), "{filter:?}");
  }
  agents.click("team-hub-new-mcp_servers");
  agents.until("the MCP server's form", |s| {
    s.window.navigation().route_now() == form_route(HubFilter::McpServers)
  });
}

#[test]
fn team_hub_create_cancel_leaves_nothing_and_goes_back_to_the_kind() {
  let mut agents = Agents::new(true);
  team(&agents);
  let before: Vec<_> = KINDS.iter().map(|(.., kind)| held(&agents, *kind)).collect();
  for (filter, _, _, name, _) in KINDS {
    show_form(&mut agents, filter);
    // Whatever was typed is not kept.
    if filter == HubFilter::Instructions {
      agents.set("team-new-instruction-name", "Release notes");
    }
    agents.click("team-hub-new-cancel");
    agents.until("the kind's list", |s| {
      s.window.navigation().route_now() == hub_route(filter) && !s.exists("team-hub-new-create")
    });
    assert!(!agents.exists("team-hub-row-new"), "{name}");
  }
  let after: Vec<_> = KINDS.iter().map(|(.., kind)| held(&agents, *kind)).collect();
  assert_eq!(
    before, after,
    "nothing was created and nothing waits for the team server"
  );
}

#[test]
fn team_hub_create_a_required_field_says_so_and_nothing_is_made() {
  let mut agents = Agents::new(true);
  team(&agents);
  let before: Vec<_> = KINDS.iter().map(|(.., kind)| held(&agents, *kind)).collect();
  let required = [
    (HubFilter::Profiles, "team-new-profile-name-row-sub"),
    (HubFilter::McpServers, "team-new-mcp-name-row-sub"),
    (HubFilter::Instructions, "team-new-instruction-name-row-sub"),
    (HubFilter::Skills, "team-new-skill-name-row-sub"),
    (HubFilter::Tools, "team-new-tool-name-row-sub"),
  ];
  for (filter, sub) in required {
    show_form(&mut agents, filter);
    // Before Create is pressed the row says what the field is.
    assert!(!agents.text(sub).starts_with("Required"), "{filter:?}");
    agents.click("team-hub-new-create");
    agents.until("the field says it is required", |s| s.text(sub).starts_with("Required"));
    assert_eq!(agents.window.navigation().route_now(), form_route(filter), "{filter:?}");
  }
  // The labels say it too.
  show_form(&mut agents, HubFilter::Instructions);
  assert_eq!(agents.text("team-new-instruction-name-row-label"), "Name · required");
  assert_eq!(agents.text("team-new-instruction-text-row-label"), "Text · required");
  let after: Vec<_> = KINDS.iter().map(|(.., kind)| held(&agents, *kind)).collect();
  assert_eq!(before, after, "nothing was made");
}

#[test]
fn team_hub_create_a_name_the_team_has_keeps_create_off_with_the_reason() {
  let mut agents = Agents::new(true);
  team(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", "github");
  agents.until("the name says why", |s| {
    s.text("team-new-mcp-name-row-sub") == "The team already has an MCP server named github · pick another name"
  });
  assert_eq!(agents.text("team-hub-actions-hint"), "Fix the name to create it");
  let before = held(&agents, ResourceKind::McpServer);
  agents.set("team-new-mcp-program", "npx");
  agents.click("team-hub-new-create");
  agents.window.pass();
  assert_eq!(held(&agents, ResourceKind::McpServer), before);
  // Another name clears it.
  agents.set("team-new-mcp-name", "figma");
  agents.until("the reason is gone", |s| {
    s.text("team-hub-actions-hint") == "Create publishes v1 for the team once the server accepts it"
  });
}

#[test]
fn team_hub_create_a_resource_the_team_server_cannot_take_is_refused_in_place_and_not_created() {
  let mut agents = Agents::new(true);
  team(&agents);
  show_form(&mut agents, HubFilter::Instructions);
  agents.set("team-new-instruction-name", "Release notes");
  agents.set("team-new-instruction-text", "Write them for customers.");
  let before = held(&agents, ResourceKind::Instruction);
  agents.press_until("team-hub-new-create", "the answer", |s| {
    s.exists("team-new-refused") || s.exists("team-new-waiting")
  });
  // Said where the person is, with the form still there to fix; nothing shows as created before the server takes it.
  assert_eq!(
    agents.window.navigation().route_now(),
    form_route(HubFilter::Instructions)
  );
  assert!(agents.exists("team-new-refused") != agents.exists("team-new-waiting"));
  if agents.exists("team-new-refused") {
    assert_eq!(
      held(&agents, ResourceKind::Instruction),
      before,
      "a refused resource is taken back whole"
    );
  }
}

#[test]
fn team_hub_create_what_adds_to_what_agents_may_do_is_a_persons_press() {
  let mut agents = Agents::new(true);
  team(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", "figma");
  agents.set("team-new-mcp-program", "npx");
  let before = held(&agents, ResourceKind::McpServer);
  with_synthetic_input(|| agents.click("team-hub-new-create"));
  agents.until("the refusal", |s| s.exists("team-new-person-only"));
  assert_eq!(
    held(&agents, ResourceKind::McpServer),
    before,
    "an MCP client's press made nothing"
  );
}

#[test]
fn team_hub_create_a_member_has_no_new_and_no_form() {
  let mut agents = Agents::new(true);
  team_as(&agents, Some(MemberRole::Member));
  // A kind the team holds nothing of shows its empty state, which has the head and the filter all the same.
  let shown =
    |s: &mut Agents| s.exists("team-hub-filter") && !s.exists("team-hub-loading") && !s.exists("team-hub-reading");
  for (filter, ..) in KINDS {
    agents.window.navigation().go(hub_route(filter));
    agents.until("the hub", shown);
    assert!(!agents.exists("team-hub-new"), "{filter:?}");
    // A link to a form shows the kind's list, not the form.
    agents.window.navigation().go(form_route(filter));
    agents.until("the hub", shown);
    assert!(
      !agents.exists("team-hub-new-create") && !agents.exists("team-hub-row-new"),
      "{filter:?}"
    );
  }
}

/// Where the element `id` is laid out: its left, top, right and bottom.
fn edges(agents: &mut Agents, id: &str) -> (f32, f32, f32, f32) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height)
}

#[test]
fn team_hub_create_each_form_lays_out_inside_the_panel_with_its_bar_pinned_below() {
  let mut agents = Agents::new(true);
  team(&agents);
  for (filter, _, _, name, _) in KINDS {
    show_form(&mut agents, filter);
    let panel = edges(&mut agents, "team-hub-panel");
    let team_rows = edges(&mut agents, "team-new-team-rows");
    let bar = edges(&mut agents, "team-hub-actions");
    let create = edges(&mut agents, "team-hub-new-create");
    // The rows stand inside the panel, side by side with its edges; the bar is the panel's bottom and holds Create.
    assert!(
      team_rows.0 >= panel.0 && team_rows.2 <= panel.2,
      "{name}: rows {team_rows:?} in {panel:?}"
    );
    assert!(team_rows.1 >= panel.1, "{name}");
    assert!(
      (bar.3 - panel.3).abs() < 1.0 && bar.0 >= panel.0 && bar.2 <= panel.2,
      "{name}: bar {bar:?}"
    );
    assert!(
      create.1 >= bar.1 && create.3 <= bar.3,
      "{name}: Create {create:?} in the bar {bar:?}"
    );
    // Every control of the team's part has room to be typed in.
    let (left, _, right, _) = edges(&mut agents, "team-new-team-rows");
    assert!(right - left > 400.0, "{name}: {left}..{right}");
  }
}
