//! *Role* of a local profile's panel (L17 `b7Tbxb`): its name, what it answers for and its role instructions are
//! edited in place, *Save* publishes them as the next version and *Cancel* puts back what is stored; what the save
//! changes in the role's prompts is a person's decision. `cargo test -p orchester-app --test agents local_hub_role`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, VersionNumber},
};
use orchester_workspace::agents::data::VersionState;

use super::{harness::Agents, local_hub::open};

const NAME: &str = "local-hub-profile-role-name-input";
const RESPONSIBILITIES: &str = "local-hub-profile-role-responsibilities-input";
const INSTRUCTIONS: &str = "local-hub-profile-role-instructions-input";
const SAVE: &str = "local-hub-profile-save";
const CANCEL: &str = "local-hub-profile-cancel";

/// Opens the hub on Profiles with `profile` in the panel.
fn show(agents: &mut Agents, profile: ResourceId) {
  open(agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists(NAME));
}

/// What is stored of the profile now: its draft, else its latest version.
fn stored(agents: &Agents, profile: ResourceId) -> (AgentProfile, VersionState) {
  let listed = agents.listed();
  let entry = listed.profile(profile).expect("profile");
  (entry.profile.clone(), entry.state)
}

/// Types a new role into the three fields.
fn type_role(agents: &mut Agents) {
  agents.set(NAME, "Reviewer");
  agents.set(RESPONSIBILITIES, "Reviews the change.");
  agents.set(INSTRUCTIONS, "Read the diff before you answer.");
}

/// Lets the window draw, and what it waits on end, a few times over.
fn passes(agents: &mut Agents) {
  for _ in 0..20 {
    agents.until("a pass", |_| true);
  }
}

fn first() -> VersionState {
  VersionState::Published(VersionNumber::FIRST)
}

#[test]
fn local_hub_role_shows_the_role_first_and_save_publishes_what_was_typed() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  assert_eq!(agents.value(NAME), "Builder");
  assert_eq!(agents.value(RESPONSIBILITIES), "Builds the change");
  assert_eq!(agents.value(INSTRUCTIONS), "Implement the approved plan.");

  type_role(&mut agents);
  let second = VersionState::Published(VersionNumber::FIRST.next().expect("v2"));
  agents.press_until(SAVE, "published", |s| stored(s, profile).1 == second);

  let (body, _) = stored(&agents, profile);
  assert_eq!(body.role().as_str(), "Reviewer");
  assert_eq!(body.responsibilities(), "Reviews the change.");
  assert_eq!(body.instructions().as_str(), "Read the diff before you answer.");
  passes(&mut agents);
  assert_eq!(agents.value(NAME), "Reviewer", "the panel shows what it published");
  assert!(!agents.exists("local-hub-profile-error"));
}

#[test]
fn local_hub_role_cancel_puts_the_stored_role_back() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  type_role(&mut agents);
  agents.press_until(CANCEL, "the stored role again", |s| s.value(NAME) == "Builder");

  assert_eq!(agents.value(RESPONSIBILITIES), "Builds the change");
  assert_eq!(agents.value(INSTRUCTIONS), "Implement the approved plan.");
  let (body, state) = stored(&agents, profile);
  assert_eq!(state, first(), "nothing was published");
  assert_eq!(body.role().as_str(), "Builder");
}

#[test]
fn local_hub_role_a_blank_name_says_why_in_place_and_save_waits() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  agents.set(NAME, "  ");

  agents.until("it says why", |s| s.exists("local-hub-profile-role-name-sub"));
  assert!(!agents.text("local-hub-profile-role-name-sub").is_empty());
  agents.click(SAVE);
  passes(&mut agents);
  assert_eq!(
    stored(&agents, profile).1,
    first(),
    "Save waits while the name is blank"
  );
}

#[test]
fn local_hub_role_automation_cannot_publish_what_the_role_tells_its_agents() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  type_role(&mut agents);
  passes(&mut agents);

  with_synthetic_input(|| agents.click(SAVE));

  agents.until("it says why", |s| s.exists("local-hub-profile-error"));
  assert!(
    agents.text("local-hub-profile-error").contains("a person decides"),
    "{}",
    agents.text("local-hub-profile-error")
  );
  assert_eq!(stored(&agents, profile).1, first());
}
