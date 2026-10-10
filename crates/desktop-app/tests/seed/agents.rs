use orchester_workspace::agents::data::{ProjectAgents, VersionState};

use super::Seed;

#[test]
fn seed_agents_has_saved_stale_and_pipeline_draft_states() {
  let seed = Seed::of("agents");
  let listed = ProjectAgents::for_screen(seed.repo()).expect("profiles");
  let builder = listed
    .profiles
    .iter()
    .find(|p| p.profile.role().as_str() == "Builder")
    .expect("Builder");
  assert!(matches!(builder.state, VersionState::Draft { published: Some(_) }));
  assert!(listed.draft_times.contains_key(&builder.id));
  assert!(
    listed
      .profiles
      .iter()
      .any(|p| matches!(p.state, VersionState::StaleDraft { .. }))
  );
  assert!(listed.usage.steps.iter().any(|step| step.draft));
}

#[test]
fn seed_agents_empty_variants_have_no_profiles() {
  for (scenario, connections) in [("agents-empty", 1), ("agents-no-connections", 0)] {
    let seed = Seed::of(scenario);
    let listed = ProjectAgents::for_screen(seed.repo()).expect("profiles");
    assert!(listed.profiles.is_empty());
    assert_eq!(listed.connections.len(), connections);
  }
}

#[test]
fn seed_agents_failed_read_can_be_repaired_for_retry() {
  let seed = Seed::of("agents-unreadable");
  assert!(ProjectAgents::for_screen(seed.repo()).is_err());
  let db = rusqlite::Connection::open(seed.repo().join(".orchester/orchester.db")).expect("db");
  db.execute("UPDATE resource_drafts SET body = (SELECT body FROM qa_agent_draft_backup WHERE resource_id = resource_drafts.resource_id) WHERE resource_id IN (SELECT resource_id FROM qa_agent_draft_backup)", []).expect("repair");
  assert!(ProjectAgents::for_screen(seed.repo()).is_ok());
}
