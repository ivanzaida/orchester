//! A local profile's panel edits in place (`Agents · Profile` L1, ORC-842): the CLI, the model, the reasoning effort and
//! the MCP servers are selects, each choice saves the profile's draft, and the panel then says so, publishes it or
//! discards it. `cargo test -p orchester-app --test agents local_hub_profile`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{id::ResourceId, resource::VersionNumber};
use orchester_workspace::agents::{data::VersionState, form::stored_effort};

use super::{
  harness::Agents,
  local_hub::{open, server},
};

/// Opens the hub on Profiles with `profile` in the panel.
fn show(agents: &mut Agents, profile: ResourceId) {
  open(agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists("local-hub-profile-effort-pick"));
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
    "local-hub-profile-cli-pick",
    "local-hub-profile-model-pick",
    "local-hub-profile-effort-pick",
    "local-hub-profile-more-settings",
  ] {
    assert!(agents.exists(part), "{part} is in the panel");
  }
  assert!(
    !agents.exists("team-hub-edit"),
    "no Edit button: the fields are in place"
  );
  assert!(!agents.exists("local-hub-profile-draft"), "no draft before a change");

  choose(&mut agents, "local-hub-profile-effort-pick", "High");

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
  choose(&mut agents, "local-hub-profile-effort-pick", "Low");
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
  choose(&mut agents, "local-hub-profile-effort-pick", "High");
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
