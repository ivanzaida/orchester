//! A local profile's panel edits in place (`Agents · Profile` L1, ORC-842): the CLI, the model, the reasoning effort
//! and the MCP servers are selects, each choice saves the profile's draft, and the panel then says so, publishes it or
//! discards it. `cargo test -p orchester-app --test agents local_hub_profile`.

use std::{thread, time::Duration};

use lurq::app::events::{MouseButton, with_synthetic_input};
use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, RepositoryAccess, VersionNumber},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::{
  data::{self, VersionState},
  form::stored_effort,
  local_edit::{self, Edit},
};

use super::{
  harness::{Agents, changed},
  local_hub::{builder_without_connection, open, server},
  support::link_press::{Presser, press_control},
};

/// Opens the hub on Profiles with `profile` in the panel.
fn show(agents: &mut Agents, profile: ResourceId) {
  open(agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists("local-hub-profile-effort"));
}

/// Chooses the option `label` of the select `id`, as a person's press on it would.
fn choose(agents: &mut Agents, id: &str, label: &str) {
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

fn effort(agents: &Agents, profile: ResourceId) -> Option<String> {
  stored_effort(&agents.listed().profile(profile).expect("profile").profile)
}

#[test]
fn local_hub_profile_a_changed_field_is_a_draft_that_publishes_as_the_next_version() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  for part in [
    "local-hub-profile-cli",
    "local-hub-profile-model",
    "local-hub-profile-effort",
  ] {
    assert!(agents.exists(part), "{part} is in the panel");
  }
  for link in ["local-hub-edit", "local-hub-profile-more-settings"] {
    assert!(
      !agents.exists(link),
      "{link}: no link to the editor's page, the fields are in place"
    );
  }
  assert!(!agents.exists("local-hub-profile-draft"), "no draft before a change");

  choose(&mut agents, "local-hub-profile-effort", "High");

  agents.until("the draft", |s| s.exists("local-hub-profile-draft"));
  assert_eq!(effort(&agents, profile).as_deref(), Some("high"));
  assert!(agents.listed().profile(profile).expect("profile").state.has_draft());
  assert!(agents.text("local-hub-profile-draft-status").contains("Draft on v1"));
  assert_eq!(agents.text("local-hub-profile-publish"), "Publish v2");
  assert!(agents.exists("local-hub-profile-discard"));

  agents.click("local-hub-profile-publish");
  agents.until("published", |s| {
    s.listed().profile(profile).expect("profile").state
      == VersionState::Published(VersionNumber::FIRST.next().expect("v2"))
  });
  agents.until("no draft", |s| !s.exists("local-hub-profile-draft"));
  assert_eq!(effort(&agents, profile).as_deref(), Some("high"));
}

#[test]
fn local_hub_profile_discarding_the_draft_asks_first_and_puts_the_version_back() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "Low");
  agents.until("the draft", |s| s.exists("local-hub-profile-discard"));

  agents.click("local-hub-profile-discard");
  agents.until("the question", |s| s.exists("hub-discard-confirm"));
  assert_eq!(
    effort(&agents, profile).as_deref(),
    Some("low"),
    "nothing is lost while it asks"
  );
  agents.click("hub-discard-confirm");

  agents.until("the version again", |s| {
    !s.exists("local-hub-profile-draft") && !s.exists("hub-discard-confirm")
  });
  assert_eq!(effort(&agents, profile), None);
  assert!(!agents.listed().profile(profile).expect("profile").state.has_draft());
}

#[test]
fn local_hub_profile_choosing_a_server_asks_first_and_only_a_press_allows_it() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let docs = server(&agents, "docs");
  show(&mut agents, profile);
  agents.click("local-hub-profile-add-server-button");
  agents.until("the menu", |s| s.exists(&format!("local-hub-profile-server-{docs}")));

  agents.click(&format!("local-hub-profile-server-{docs}"));

  agents.until("the question", |s| s.exists("local-hub-profile-confirm-confirm"));
  assert!(
    agents
      .listed()
      .profile(profile)
      .expect("profile")
      .profile
      .mcp_servers()
      .is_empty(),
    "nothing saved before the answer"
  );
  assert!(!agents.exists("local-hub-profile-draft"));
  // Its confirm takes no press before the question has been drawn once.
  agents.press_until("local-hub-profile-confirm-confirm", "the draft chooses it", |s| {
    s.listed()
      .profile(profile)
      .expect("profile")
      .profile
      .mcp_servers()
      .contains(docs)
  });
  agents.until("the panel says so", |s| s.exists("local-hub-profile-draft"));
  assert!(agents.text("local-hub-profile-mcp-value-0").contains("docs"));
}

#[test]
fn local_hub_profile_a_draft_replaces_the_pinned_fix_with_publish_and_discard() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "High");
  agents.until("the draft", |s| s.exists("local-hub-profile-draft"));

  assert!(
    !agents.exists("local-hub-fix-use"),
    "the pinned bar is the draft's: Publish and Discard"
  );
  assert!(agents.exists("local-hub-profile-publish"));
}

#[test]
fn local_hub_profile_a_list_row_ends_in_its_version_under_a_kinds_filter_only() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  let version = format!("local-hub-row-{profile}-version");
  assert_eq!(agents.text(&version), "v1");

  open(&mut agents, HubView::new(HubFilter::All, Some(profile)));
  agents.until("the row", |s| s.exists(&format!("local-hub-row-{profile}")));
  assert!(
    !agents.exists(&version),
    "under All the row ends in its kind's tag instead"
  );
}

/// Changes the stored draft as another window or the profile's editor would, after the panel showed it.
fn change_elsewhere(agents: &Agents, profile: ResourceId, effort: &str) {
  // The draft's time moves on by at least a millisecond.
  thread::sleep(Duration::from_millis(5));
  local_edit::edit(&agents.root, profile, &Edit::Effort(Some(effort.into())), &[]).expect("saved elsewhere");
}

#[test]
fn local_hub_profile_discard_keep_editing_leaves_the_draft() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "Low");
  agents.until("the draft", |s| s.exists("local-hub-profile-discard"));
  agents.click("local-hub-profile-discard");
  agents.until("the question", |s| s.exists("hub-discard-keep"));

  agents.click("hub-discard-keep");

  agents.until("the question is gone", |s| !s.exists("hub-discard-keep"));
  assert_eq!(effort(&agents, profile).as_deref(), Some("low"));
  assert!(
    agents.exists("local-hub-profile-draft"),
    "the draft is still the panel's"
  );
}

#[test]
fn local_hub_profile_discard_refuses_a_draft_that_is_not_the_one_it_showed() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "Low");
  agents.until("the draft", |s| s.exists("local-hub-profile-discard"));
  agents.click("local-hub-profile-discard");
  agents.until("the question", |s| s.exists("hub-discard-confirm"));

  change_elsewhere(&agents, profile, "high");
  agents.click("hub-discard-confirm");

  agents.until("it says why", |s| s.exists("hub-discard-failure"));
  assert!(agents.text("hub-discard-failure").contains("changed since it was read"));
  assert_eq!(
    effort(&agents, profile).as_deref(),
    Some("high"),
    "the draft that moved is kept"
  );
  assert!(agents.listed().profile(profile).expect("profile").state.has_draft());
}

#[test]
fn local_hub_profile_publish_refuses_a_draft_that_is_not_the_one_it_showed() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "Low");
  agents.until("the draft", |s| s.exists("local-hub-profile-publish"));

  change_elsewhere(&agents, profile, "high");
  agents.click("local-hub-profile-publish");

  agents.until("it says why", |s| s.exists("local-hub-profile-error"));
  assert!(
    agents
      .text("local-hub-profile-error")
      .contains("changed since it was read")
  );
  let stored = agents.listed().profile(profile).expect("profile").clone();
  assert!(
    matches!(stored.state, VersionState::Draft { .. }),
    "nothing was published: {:?}",
    stored.state
  );
}

#[test]
fn local_hub_profile_publish_of_a_draft_that_changes_the_prompts_is_left_to_the_editor() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let body = changed(&agents.listed().profile(profile).expect("profile").profile);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  data::save_draft(&mut db, profile, body, Timestamp::now()).expect("a draft that tells its agents more");
  show(&mut agents, profile);
  agents.until("the draft", |s| s.exists("local-hub-profile-publish"));

  agents.click("local-hub-profile-publish");

  agents.until("it says where to publish", |s| s.exists("local-hub-profile-error"));
  assert!(
    agents.text("local-hub-profile-error").contains("More settings"),
    "{}",
    agents.text("local-hub-profile-error")
  );
  assert!(
    matches!(
      agents.listed().profile(profile).expect("profile").state,
      VersionState::Draft { .. }
    ),
    "the press published nothing"
  );
  assert!(agents.exists("local-hub-profile-publish"), "the draft is still there");
}

/// A Claude Code connection whose last check found `claude-a` (with two efforts) and `claude-b`.
fn claude(agents: &Agents) -> ConnectionId {
  claude_signed(agents, SignIn::SignedIn)
}

/// [`claude`] whose last check found the CLI as `sign_in` says.
fn claude_signed(agents: &Agents, sign_in: SignIn) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude 2").expect("name"),
    AgentProvider::ClaudeCodeCli,
    agents.root.join("claude.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  let model = |id: &str, efforts: &[&str]| {
    CatalogModel::new(
      ModelId::new(id).expect("model"),
      efforts.iter().map(|effort| (*effort).to_owned()).collect(),
    )
  };
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![model("claude-a", &["low", "high"]), model("claude-b", &[])],
        ),
      },
    })
    .expect("check");
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_connection(&connection)
    .expect("stored");
  connection.id()
}

/// Builder on `connection`'s `claude-a`, published as v1, with what every CLI enforces so that nothing widens.
fn builder_on_claude(agents: &Agents, connection: ConnectionId) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new("Builder").expect("name"),
    "Builds the change".into(),
    PromptText::new("Implement the approved plan.").expect("instructions"),
    connection,
    ModelId::new("claude-a").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ));
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = data::create_profile(&mut db, &profile, Timestamp::now()).expect("profile");
  data::publish(&mut db, id, Timestamp::now()).expect("publish");
  id
}

#[test]
fn local_hub_profile_the_model_select_changes_the_draft_and_starts_at_the_default_effort() {
  let mut agents = Agents::new(true);
  let profile = builder_on_claude(&agents, claude(&agents));
  show(&mut agents, profile);
  choose(&mut agents, "local-hub-profile-effort", "High");
  agents.until("the draft", |s| s.exists("local-hub-profile-draft"));

  choose(&mut agents, "local-hub-profile-model", "claude-b");

  agents.until("the model is the draft's", |s| {
    s.listed().profile(profile).expect("profile").profile.model().as_str() == "claude-b"
  });
  assert_eq!(
    effort(&agents, profile),
    None,
    "a new model starts at the CLI's default"
  );
  assert_eq!(agents.text("local-hub-profile-publish"), "Publish v2");
}

#[test]
fn local_hub_profile_the_cli_select_runs_it_on_the_other_clis_connection() {
  let mut agents = Agents::new(true);
  let connection = claude(&agents);
  let profile = builder_on_claude(&agents, connection);
  show(&mut agents, profile);
  agents.until("two CLIs to choose", |s| s.exists("local-hub-profile-cli"));

  choose(&mut agents, "local-hub-profile-cli", "Codex");
  // What Codex can enforce differs from Claude Code's, so the change may ask before it is saved: only a press answers.
  agents.until("the draft or its question", |s| {
    s.exists("local-hub-profile-draft") || s.exists("local-hub-profile-confirm-confirm")
  });
  if agents.exists("local-hub-profile-confirm-confirm") {
    agents.press_until("local-hub-profile-confirm-confirm", "the draft", |s| {
      s.exists("local-hub-profile-draft")
    });
  }

  let stored = agents.listed().profile(profile).expect("profile").profile.clone();
  assert_ne!(stored.connection(), connection, "it runs on the Codex connection now");
  assert_eq!(
    stored.model().as_str(),
    "test-model",
    "on that connection's first model"
  );
}

#[test]
fn local_hub_profile_the_connection_select_says_to_publish_or_discard_first_while_a_draft_exists() {
  let mut agents = Agents::new(true);
  let (profile, _) = builder_without_connection(&agents);
  let body = changed(&agents.listed().profile(profile).expect("profile").profile);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  data::save_draft(&mut db, profile, body, Timestamp::now()).expect("a draft");
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists("local-hub-profile-connection"));

  assert!(
    agents
      .text("local-hub-profile-connection-row")
      .contains("Publish or discard"),
    "{}",
    agents.text("local-hub-profile-connection-row")
  );
  assert!(
    !agents.exists("local-hub-fix-use"),
    "the draft's bar replaces the connection fix"
  );
}

#[test]
fn local_hub_profile_automation_cannot_answer_allow_or_take_an_account_for_a_profile() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let docs = server(&agents, "docs");
  show(&mut agents, profile);
  agents.click("local-hub-profile-add-server-button");
  agents.until("the menu", |s| s.exists(&format!("local-hub-profile-server-{docs}")));
  agents.click(&format!("local-hub-profile-server-{docs}"));
  agents.until("the question", |s| s.exists("local-hub-profile-confirm-confirm"));
  // The confirm takes no press before the question lived through a pass: press it as a client would, until it answers.
  agents.until("the refusal", |s| {
    with_synthetic_input(|| s.click("local-hub-profile-confirm-confirm"));
    s.exists("local-hub-profile-confirm-person-only")
  });
  for _ in 0..20 {
    agents.until("a pass", |_| true);
    thread::sleep(Duration::from_millis(5));
  }

  assert!(
    agents
      .listed()
      .profile(profile)
      .expect("profile")
      .profile
      .mcp_servers()
      .is_empty(),
    "automation allowed nothing"
  );
  assert!(agents.exists("local-hub-profile-confirm-confirm"), "it still asks");
  assert!(
    agents
      .text("local-hub-profile-confirm-person-only")
      .starts_with("Only a person does this here"),
    "the refusal is said under the question"
  );
  agents.press_until("local-hub-profile-confirm-confirm", "a person allows it", |s| {
    s.listed()
      .profile(profile)
      .expect("profile")
      .profile
      .mcp_servers()
      .contains(docs)
  });
}

#[test]
fn local_hub_profile_automation_cannot_take_an_account_for_a_profile_that_lost_its_own() {
  let mut agents = Agents::new(true);
  let (profile, [claude_2, home]) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the fix", |s| s.exists("local-hub-fix-use"));

  with_synthetic_input(|| agents.click("local-hub-fix-use"));
  for _ in 0..20 {
    agents.until("a pass", |_| true);
    thread::sleep(Duration::from_millis(5));
  }

  let connection = agents.listed().profile(profile).expect("profile").profile.connection();
  assert!(
    connection != claude_2 && connection != home,
    "automation chose no account for it"
  );
  assert!(
    agents
      .text("local-hub-fix-use-person-only")
      .starts_with("Only a person does this here"),
    "the refused press says why in the panel"
  );
  agents.press_until("local-hub-fix-use", "a person's press takes it", |s| {
    s.listed()
      .profile(profile)
      .is_some_and(|entry| entry.profile.connection() == claude_2)
  });
}

#[test]
fn local_hub_profile_a_persons_pointer_press_takes_the_account_and_choosing_in_the_select_changes_nothing() {
  let mut agents = Agents::new(true);
  let (profile, [claude_2, home]) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the fix", |s| s.exists("local-hub-fix-use"));

  choose(&mut agents, "local-hub-profile-connection", "Claude Code · Home");
  agents.until("the bar names the choice", |s| {
    s.text("local-hub-fix-use").contains("Home")
  });
  let stored = agents.listed().profile(profile).expect("profile").profile.connection();
  assert!(
    stored != claude_2 && stored != home,
    "a choice in the select stores nothing"
  );
  assert!(!agents.exists("local-hub-fix-use-person-only"));

  press_control(
    &mut agents.window,
    "local-hub-fix-use",
    (Presser::Person, MouseButton::Left),
    (false, false),
  );
  agents.until("a person's pointer takes it", |s| {
    s.listed()
      .profile(profile)
      .is_some_and(|entry| entry.profile.connection() == home)
  });
  assert!(!agents.exists("local-hub-fix-use-person-only"));
}

/// A profile on a connection whose CLI is signed out is not ready (ORC-848 D11): *Ready?* says so and the pinned fix is
/// to sign in.
#[test]
fn local_hub_profile_a_signed_out_connection_is_not_ready_and_the_fix_is_to_sign_in() {
  let mut agents = Agents::new(true);
  let connection = claude_signed(&agents, SignIn::SignedOut);
  let profile = builder_on_claude(&agents, connection);
  show(&mut agents, profile);

  agents.until("the sign-in fix", |s| s.exists("local-hub-fix-sign-in"));
  assert!(agents.text("local-hub-fix-sign-in").contains("Sign in to Claude Code"));
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "Claude Code is signed out here"
  );
}
