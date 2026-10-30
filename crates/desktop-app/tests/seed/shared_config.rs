//! `shared-config` and `shared-config-member`: the catalogue as the team's configuration of a shared project, as its
//! libraries read it (ORC-390): every resource the team's with its publisher; as the owner, a draft, a version waiting
//! for the server and one that met a teammate's; as a member, nothing to change.

use orchester_core::{resource::AgentProfile, store::ResourceStore};
use orchester_store::{ProjectDb, PublishState, SharedStanding};

use super::Seed;

/// How the profile called `role` stands.
fn profile(db: &ProjectDb, role: &str) -> SharedStanding {
  let standings = db.shared_standings().expect("the standings");
  db.resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("the profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == role)
    .and_then(|resource| standings.into_iter().find(|s| s.resource == resource.id()))
    .unwrap_or_else(|| panic!("{role} is the team's"))
}

#[test]
fn seed_shared_config_leaves_the_owners_draft_a_waiting_version_and_a_met_one() {
  let db = Seed::of("shared-config").db();
  assert!(db.may_change_configuration().expect("read"), "the owner changes it");
  let standings = db.shared_standings().expect("the standings");
  let published = standings.iter().filter(|standing| standing.confirmed.is_some()).count();
  assert!(
    published > 5,
    "the catalogue's versions, with their publishers: {standings:?}"
  );

  let builder = profile(&db, "Builder");
  assert!(builder.waiting.is_empty());
  let draft = db.draft::<AgentProfile>(builder.resource).expect("read");
  assert!(draft.is_some(), "the owner's draft on Builder");

  let planner = profile(&db, "Planner");
  let states: Vec<_> = planner.waiting.iter().map(|waiting| waiting.state.clone()).collect();
  assert_eq!(states, [PublishState::Waiting], "published here, not sent");

  let reviewer = profile(&db, "Reviewer");
  let confirmed = reviewer.confirmed.as_ref().expect("olha's version");
  assert_eq!(confirmed.publisher.as_deref(), Some("olha"));
  let states: Vec<_> = reviewer.waiting.iter().map(|waiting| waiting.state.clone()).collect();
  assert_eq!(
    states,
    [PublishState::Conflict {
      latest: confirmed.number
    }]
  );
}

#[test]
fn seed_shared_config_member_reads_every_resource_and_changes_none() {
  let db = Seed::of("shared-config-member").db();
  assert!(!db.may_change_configuration().expect("read"), "a member");
  let standings = db.shared_standings().expect("the standings");
  assert!(!standings.is_empty());
  assert!(
    standings.iter().all(|standing| standing.waiting.is_empty()),
    "nothing of a member's waits to be sent: {standings:?}"
  );
}
