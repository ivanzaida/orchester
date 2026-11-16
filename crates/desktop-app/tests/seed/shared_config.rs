//! `shared-config` and `shared-config-member`: the catalogue as the team's configuration of a shared project, as the
//! stream keeps it (ORC-390): every resource the team's with its publisher; nothing of the team's waits on this
//! computer and no draft of it is kept here, as the owner or as a member.

use orchester_core::{
  resource::{AgentProfile, ResourceKind},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ResourceSharing};

use super::Seed;

/// Every profile of the catalogue is the team's, with a version and no draft kept here.
fn the_teams_profiles(db: &ProjectDb) {
  let profiles = db.resources(ResourceKind::AgentProfile).expect("the profiles");
  assert!(profiles.len() > 2, "{profiles:?}");
  for profile in profiles {
    assert!(
      matches!(
        db.resource_sharing(profile.id()).expect("read"),
        Some(ResourceSharing::Shared(_))
      ),
      "{} is the team's",
      profile.name().as_str()
    );
    assert!(db.latest_version::<AgentProfile>(profile.id()).expect("read").is_some());
    assert!(db.draft::<AgentProfile>(profile.id()).expect("read").is_none());
  }
}

#[test]
fn seed_shared_config_holds_the_teams_resources_for_the_owner() {
  let db = Seed::of("shared-config").db();
  assert!(db.may_change_configuration().expect("read"), "the owner changes it");
  the_teams_profiles(&db);
}

#[test]
fn seed_shared_config_member_reads_every_resource_and_changes_none() {
  let db = Seed::of("shared-config-member").db();
  assert!(!db.may_change_configuration().expect("read"), "a member");
  the_teams_profiles(&db);
}
