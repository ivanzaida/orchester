//! The create forms of a local project's Agents hub (`Agents · v2` `acFkz`, L3–L7, ORC-843): New in the head opens the
//! kind's form in the panel with only Create and Cancel; Create makes v1 at once (no draft), or for an MCP server,
//! which has no versions, the server with its program path and the values of its secrets; Cancel makes nothing; a field
//! that is wrong says so in place. `cargo test -p orchester-app --test agents local_hub_create`.

use std::collections::BTreeMap;

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView, NewConnection, ProjectPage, Route};
use orchester_core::{
  connection::mcp::{McpEnvValue, McpTransport},
  id::SecretId,
  resource::{AgentProfile, Instruction, ResourceKind, Skill, Tool, VariableName},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, local_hub::open, support::project_id};

/// The kinds that have a form: their filter, the head's New, the form's title, the section and the row ids' name.
const KINDS: [(HubFilter, &str, &str, &str, &str); 5] = [
  (
    HubFilter::Profiles,
    "New agent",
    "New agent profile",
    "Profile",
    "profile",
  ),
  (
    HubFilter::McpServers,
    "New MCP server",
    "New MCP server",
    "Server",
    "mcp",
  ),
  (
    HubFilter::Instructions,
    "New instruction",
    "New instruction",
    "Instruction",
    "instruction",
  ),
  (HubFilter::Skills, "New skill", "New skill", "Skill", "skill"),
  (HubFilter::Tools, "New tool", "New tool", "Tool", "tool"),
];

/// The name field of the kind whose rows are named after `name`: a profile's is its *Role* section's, an MCP server's
/// is its create form's own, and the others' are the form an edit has too.
fn name_field(name: &str) -> String {
  match name {
    "profile" => "team-new-profile-role-name-input".to_owned(),
    "mcp" => "team-new-mcp-name".to_owned(),
    _ => format!("team-new-{name}-name-input"),
  }
}

/// What the name's row says under its label, of the kind whose rows are named after `name`.
fn name_sub(name: &str) -> String {
  match name {
    "profile" => "team-new-profile-role-name-sub".to_owned(),
    "mcp" => "team-new-mcp-name-row-sub".to_owned(),
    _ => format!("team-new-{name}-name-sub"),
  }
}

pub(super) const TYPED: &str = "hunter2-typed-value";

pub(super) fn form_route(filter: HubFilter) -> Route {
  Route::Project(project_id(), ProjectPage::hub(HubView::adding(filter)))
}

fn hub_route(filter: HubFilter) -> Route {
  Route::Project(project_id(), ProjectPage::hub(HubView::new(filter, None)))
}

pub(super) fn show_form(agents: &mut Agents, filter: HubFilter) {
  agents.window.navigation().go(form_route(filter));
  agents.until("the form", |s| s.exists("local-hub-new-create"));
}

/// How many resources of `kind` this computer holds.
pub(super) fn held(agents: &Agents, kind: ResourceKind) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .resources(kind)
    .expect("resources")
    .len()
}

pub(super) fn servers(agents: &Agents) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .mcp_servers()
    .expect("servers")
    .len()
}

/// Chooses the option `label` of the select `id`, as a person's press on it would, once the select lists it.
pub(super) fn choose(agents: &mut Agents, id: &str, label: &str) {
  agents.until("the option", |s| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .is_some_and(|select| select.labels().iter().any(|option| &**option == label))
  });
  let mut select = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .unwrap_or_else(|| panic!("an option {label}"));
  select.commit(index);
  agents.window.pass();
}

/// Says the resource of `kind` named by the route the page went to has its first version and no draft, which Create
/// published at once.
fn assert_v1_and_no_draft<B: orchester_core::resource::ResourceBody>(agents: &Agents) {
  let db = ProjectDb::open(&agents.root).expect("database");
  let resource = db
    .resources(B::KIND)
    .expect("resources")
    .into_iter()
    .next()
    .expect("the new resource");
  assert_eq!(
    db.versions::<B>(resource.id()).expect("versions").len(),
    1,
    "Create makes v1"
  );
  assert!(
    db.draft::<B>(resource.id()).expect("draft").is_none(),
    "no draft is left"
  );
}

/// The project's secret `FIGMA_TOKEN`, which no value is kept for.
pub(super) fn figma_token(agents: &Agents) -> Secret {
  project_secret(agents, "FIGMA_TOKEN")
}

/// A secret of the project named `name`, which no value is kept for.
pub(super) fn project_secret(agents: &Agents, name: &str) -> Secret {
  let token = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(name).expect("name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_secret(&token)
    .expect("secret");
  token
}

/// Types `TYPED` as the value of `FIGMA_TOKEN`, which the variable typed in the form takes: its row, its dialog, then
/// *Use this value*.
pub(super) fn type_figma_value(agents: &mut Agents) {
  agents.until("the secret's row", |s| s.exists("local-hub-secret-FIGMA_TOKEN-replace"));
  agents.click("local-hub-secret-FIGMA_TOKEN-replace");
  agents.until("its dialog", |s| s.exists("local-hub-secret-value"));
  agents.set("local-hub-secret-value", TYPED);
  agents.click("local-hub-secret-keep");
}

#[test]
fn local_hub_create_new_opens_the_form_of_each_kind_with_only_create_and_cancel() {
  let mut agents = Agents::new(true);
  for (filter, new, title, section, name) in KINDS {
    open(&mut agents, HubView::new(filter, None));
    agents.until("the head", |s| {
      s.exists("local-hub-new") && s.text("local-hub-new") == new
    });
    agents.click("local-hub-new");
    agents.until("the form", |s| s.window.navigation().route_now() == form_route(filter));
    agents.until("the form", |s| s.exists("local-hub-new-create"));
    // A page in the panel, not a window: the new resource is the list's first row, chosen, and the panel is its form.
    assert_eq!(agents.text("team-hub-panel-title"), title, "{name}");
    assert_eq!(agents.text("team-hub-row-new-name"), title, "{name}");
    assert_eq!(agents.text("team-hub-row-new-line"), "Not created yet", "{name}");
    assert_eq!(agents.text("local-new-section-label"), section, "{name}");
    // Everything is the person's own: no *For the team*, no note on what each computer fills in.
    assert!(
      !agents.exists("team-new-team") && !agents.exists("team-new-computer"),
      "{name}"
    );
    assert_eq!(agents.text("local-hub-new-create"), "Create", "{name}");
    assert_eq!(agents.text("local-hub-new-cancel"), "Cancel", "{name}");
    assert!(!agents.exists("local-hub-new-save"), "{name}");
  }
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  agents.click("local-hub-new");
  agents.until("the MCP server's form", |s| s.exists("local-hub-new-create"));
  assert_eq!(
    agents.text("team-hub-panel-version"),
    "not created yet · Create adds it to this project",
    "a local MCP server has no versions"
  );
  assert_eq!(agents.text("team-hub-actions-hint"), "Create adds it on this computer");
}

/// All and Archived hold no one kind: their *New* makes an agent profile, its form at once, with no menu between.
#[test]
fn local_hub_create_under_all_and_archived_new_opens_the_profile_form() {
  let mut agents = Agents::new(true);
  for filter in [HubFilter::All, HubFilter::Archived] {
    open(&mut agents, HubView::new(filter, None));
    assert_eq!(agents.text("local-hub-new"), "New agent", "{}", filter.name());
    agents.click("local-hub-new");
    agents.until("the profile's form", |s| {
      s.window.navigation().route_now() == form_route(HubFilter::Profiles) && s.exists("local-hub-new-create")
    });
    assert_eq!(
      agents.text("team-hub-panel-title"),
      "New agent profile",
      "{}",
      filter.name()
    );
  }
}

#[test]
fn local_hub_create_an_instruction_is_v1_at_once_with_no_draft() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Instructions);
  agents.set("team-new-instruction-name-input", "Handoff notes");
  agents.set(
    "team-new-instruction-text-input",
    "Write a handoff note at the end of every run.",
  );
  agents.click("local-hub-new-create");
  agents.until("the new instruction's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.item.is_some() && !view.adding
    ) && !s.exists("local-hub-new-create")
  });
  assert_v1_and_no_draft::<Instruction>(&agents);
}

#[test]
fn local_hub_create_a_skill_is_v1_at_once_with_no_draft() {
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
  agents.click("local-hub-new-create");
  agents.until("the new skill's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.item.is_some() && !view.adding
    )
  });
  assert_v1_and_no_draft::<Skill>(&agents);
}

#[test]
fn local_hub_create_a_command_tool_is_v1_at_once_with_no_draft() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Tools);
  agents.set("team-new-tool-name-input", "cargo-check");
  agents.set(
    "team-new-tool-program-input",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  agents.click("local-hub-new-create");
  agents.until("the new tool's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.item.is_some() && !view.adding
    )
  });
  assert_v1_and_no_draft::<Tool>(&agents);
}

#[test]
fn local_hub_create_a_profile_names_its_connection_and_is_v1_at_once() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Profiles);
  agents.set("team-new-profile-role-name-input", "Reviewer");
  choose(&mut agents, "team-new-profile-cli", "Codex");
  assert!(
    agents
      .text("team-new-profile-connection-row-label")
      .contains("Connection"),
    "a local profile has a connection row"
  );
  choose(&mut agents, "team-new-profile-model", "test-model");
  agents.click("local-hub-new-create");
  agents.until("the new profile's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.item.is_some() && !view.adding
    )
  });
  assert_v1_and_no_draft::<AgentProfile>(&agents);
  let listed = agents.listed();
  let made = listed
    .profiles
    .iter()
    .find(|entry| entry.profile.role().as_str() == "Reviewer");
  let connection = listed.connections[0].id();
  assert_eq!(
    made.map(|entry| entry.profile.connection()),
    Some(connection),
    "the profile runs on the connection of its CLI"
  );
}

#[test]
fn local_hub_create_an_mcp_server_has_its_program_path_and_the_value_of_its_secret() {
  let mut agents = Agents::new(true);
  let token = figma_token(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.until("the server's rows", |s| s.exists("connection-mcp-program"));
  let program = std::env::current_exe().expect("the test program");
  agents.set("team-new-mcp-name", "figma");
  agents.set("connection-mcp-program", &program.to_string_lossy());
  agents.set(
    "connection-mcp-variables",
    "FIGMA_API_KEY=${secret:FIGMA_TOKEN@development}",
  );
  type_figma_value(&mut agents);
  agents.until("the value waits for Create", |s| {
    s.text("local-hub-secret-FIGMA_TOKEN-sub").contains("not saved yet")
  });
  assert!(
    !agents.text("local-hub-secret-FIGMA_TOKEN-value").contains(TYPED),
    "the box holds dots, whatever was typed"
  );

  agents.click("local-hub-new-create");
  agents.until("the new server's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.server.is_some() && !view.adding
    )
  });

  let db = ProjectDb::open(&agents.root).expect("database");
  let stored = db
    .mcp_servers()
    .expect("servers")
    .into_iter()
    .next()
    .expect("the new server");
  assert_eq!(stored.name().as_str(), "figma");
  assert!(stored.enabled());
  let McpTransport::Stdio(stdio) = stored.transport() else {
    panic!("a program");
  };
  assert_eq!(
    stdio.program().as_str(),
    program.to_string_lossy(),
    "the program path is stored as the fixed file it is"
  );
  let taken: BTreeMap<&VariableName, &McpEnvValue> = stored.environment().iter().collect();
  assert!(
    taken
      .values()
      .any(|value| matches!(value, McpEnvValue::Secret(id) if *id == token.id()))
  );
  let services = agents.window.launch.services().expect("services");
  let key = token.vault_key(project_id());
  assert_eq!(
    services
      .vault
      .get()
      .read(&key)
      .expect("kept in the secret store")
      .expose(),
    TYPED
  );
  assert_eq!(
    held(&agents, ResourceKind::McpServer),
    0,
    "a server is not a versioned resource"
  );
}

#[test]
fn local_hub_create_an_mcp_server_the_secret_store_refuses_is_not_created_and_says_which_secret() {
  let mut agents = Agents::refusing_secrets();
  figma_token(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.until("the server's rows", |s| s.exists("connection-mcp-program"));
  agents.set("team-new-mcp-name", "figma");
  agents.set(
    "connection-mcp-program",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  agents.set(
    "connection-mcp-variables",
    "FIGMA_API_KEY=${secret:FIGMA_TOKEN@development}",
  );
  type_figma_value(&mut agents);

  agents.click("local-hub-new-create");
  agents.until("the refusal", |s| s.exists("team-new-refused"));

  let said = agents.text("team-new-refused");
  assert!(said.contains("FIGMA_TOKEN"), "{said}");
  assert!(!said.contains(TYPED), "no value in the words");
  assert_eq!(servers(&agents), 0, "nothing was created");
  assert_eq!(
    agents.window.navigation().route_now(),
    form_route(HubFilter::McpServers),
    "the form stays, with what was typed"
  );
}

#[test]
fn local_hub_create_cancel_makes_nothing_and_goes_back_to_the_kind() {
  let mut agents = Agents::new(true);
  let before: Vec<_> = KINDS
    .iter()
    .map(|(filter, ..)| {
      if *filter == HubFilter::McpServers {
        servers(&agents)
      } else {
        held(
          &agents,
          match filter {
            HubFilter::Profiles => ResourceKind::AgentProfile,
            HubFilter::Instructions => ResourceKind::Instruction,
            HubFilter::Skills => ResourceKind::Skill,
            _ => ResourceKind::Tool,
          },
        )
      }
    })
    .collect();
  for (filter, _, _, _, name) in KINDS {
    show_form(&mut agents, filter);
    agents.set(&name_field(name), "Whatever");
    agents.click("local-hub-new-cancel");
    agents.until("the kind's list", |s| {
      s.window.navigation().route_now() == hub_route(filter) && !s.exists("local-hub-new-create")
    });
    assert!(!agents.exists("team-hub-row-new"), "{name}");
  }
  let after = [
    held(&agents, ResourceKind::AgentProfile),
    servers(&agents),
    held(&agents, ResourceKind::Instruction),
    held(&agents, ResourceKind::Skill),
    held(&agents, ResourceKind::Tool),
  ];
  assert_eq!(before, after, "nothing was created");
}

#[test]
fn local_hub_create_a_required_field_says_so_and_nothing_is_made() {
  let mut agents = Agents::new(true);
  for (filter, _, _, _, name) in KINDS {
    show_form(&mut agents, filter);
    let sub = name_sub(name);
    assert!(
      !agents.exists(&sub) || !agents.text(&sub).starts_with("Required"),
      "{name}"
    );
    agents.click("local-hub-new-create");
    agents.until("the field says it is required", |s| {
      s.text(&sub).starts_with("Required")
    });
    assert_eq!(agents.window.navigation().route_now(), form_route(filter), "{name}");
  }
  assert_eq!(held(&agents, ResourceKind::Instruction), 0);
  assert_eq!(servers(&agents), 0);
}

#[test]
fn local_hub_create_a_name_the_project_has_keeps_create_off_with_the_reason() {
  let mut agents = Agents::new(true);
  agents.profile(true);
  show_form(&mut agents, HubFilter::Profiles);
  agents.set("team-new-profile-role-name-input", "Builder");
  agents.until("the name says why", |s| {
    s.text("team-new-profile-role-name-sub")
      == "This project already has an agent profile named Builder · pick another name"
  });
  assert_eq!(agents.text("team-hub-actions-hint"), "Fix the name to create it");
  agents.set("team-new-profile-role-name-input", "Planner");
  agents.until("the reason is gone", |s| {
    s.text("team-hub-actions-hint") == "Create makes v1 on this computer"
  });
}

#[test]
fn local_hub_create_what_adds_to_what_agents_may_do_is_a_persons_press() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::McpServers);
  agents.until("the server's rows", |s| s.exists("connection-mcp-program"));
  agents.set("team-new-mcp-name", "figma");
  agents.set(
    "connection-mcp-program",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  with_synthetic_input(|| agents.click("local-hub-new-create"));
  agents.until("the refusal", |s| s.exists("team-new-person-only"));
  assert_eq!(servers(&agents), 0, "an MCP client's press made nothing");
}

#[test]
fn local_hub_create_the_old_new_pages_of_a_profile_and_an_mcp_server_open_the_hub_form() {
  let mut agents = Agents::new(true);
  let pages = [
    (
      ProjectPage::NewAgent(orchester_app::routes::AgentTab::default()),
      HubFilter::Profiles,
    ),
    (
      ProjectPage::NewConnection(NewConnection::McpServer),
      HubFilter::McpServers,
    ),
  ];
  for (page, filter) in pages {
    agents.window.navigation().go(Route::Project(project_id(), page));
    agents.until("the hub's form", |s| {
      s.window.navigation().route_now() == form_route(filter) && s.exists("local-hub-new-create")
    });
  }
}

#[test]
fn local_hub_create_a_profile_is_a_persons_press() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Profiles);
  agents.set("team-new-profile-role-name-input", "Reviewer");
  choose(&mut agents, "team-new-profile-cli", "Codex");
  choose(&mut agents, "team-new-profile-model", "test-model");
  with_synthetic_input(|| agents.click("local-hub-new-create"));
  agents.until("the refusal", |s| s.exists("team-new-person-only"));
  assert_eq!(
    held(&agents, ResourceKind::AgentProfile),
    0,
    "automation does not publish what the role is told"
  );
}
