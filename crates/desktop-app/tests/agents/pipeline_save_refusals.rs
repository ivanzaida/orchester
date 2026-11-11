//! A save that cannot publish is refused and stores nothing: a pipeline holds no draft (D-032). The refusal names the
//! step whose profile is gone, or every step not ready; a pipeline at its last version number says so before the
//! press. `cargo test -p orchester-app --test agents pipeline_save_refusals`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore, time::Timestamp};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
};

const SAVE: &str = "pipeline-hub-save";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";
const REFUSED: &str = "pipeline-hub-refused";
const CONFIRM: &str = "pipeline-hub-confirm-confirm";

/// How many versions `id` has, and whether a draft is stored.
fn stored(agents: &Agents, id: ResourceId) -> (usize, bool) {
  let db = ProjectDb::open(&agents.root).expect("database");
  (
    db.versions::<Pipeline>(id).expect("versions").len(),
    db.draft::<Pipeline>(id).expect("draft").is_some(),
  )
}

/// Presses Save, allowing what it widens when it asks, until `done`.
fn save_until(agents: &mut Agents, what: &str, done: impl Fn(&mut Agents) -> bool) {
  agents.click(SAVE);
  agents.until(what, |s| {
    if s.exists(CONFIRM) {
      s.click(CONFIRM);
    }
    done(s)
  });
}

#[test]
fn pipeline_save_refusals_a_change_while_a_profile_is_gone_is_refused_and_names_the_step() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let retired = agents.profile(true);
  let id = pipeline_of(&agents, "Agents tour", [Some(builder), Some(retired)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Changed");
  agents.until("the bar", |s| s.text(HINT).starts_with("Unsaved changes"));
  save_until(&mut agents, "the refusal", |s| s.exists(REFUSED));
  let said = agents.text(REFUSED);
  assert!(
    said.contains("Step “Build” runs an agent profile that is no longer available. Choose another."),
    "the step by its name: {said}"
  );
  assert!(!said.contains(&retired.to_string()), "no raw id: {said}");
  assert_eq!(stored(&agents, id), (1, false), "nothing stored: no draft, no version");
  assert_eq!(agents.value(DESCRIPTION), "Changed", "what was typed stays");
}

#[test]
fn pipeline_save_refusals_a_refused_save_names_every_step_not_ready() {
  let mut agents = Agents::new(true);
  let (planner, builder) = (agents.profile(true), agents.profile(true));
  let id = pipeline_of(&agents, "Dependency update", [Some(planner), Some(builder)]);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  for retired in [planner, builder] {
    db.archive_local_resource(retired, Timestamp::now())
      .expect("archived");
  }
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Waiting for profiles");
  agents.until("the bar", |s| s.text(HINT).starts_with("Unsaved changes"));
  save_until(&mut agents, "the refusal", |s| s.exists(REFUSED));
  let said = agents.text(REFUSED);
  assert!(said.contains("2 steps aren't ready: Plan, Build"), "{said}");
  assert_eq!(stored(&agents, id), (1, false), "nothing stored");
}

#[test]
fn pipeline_save_refusals_a_pipeline_at_its_last_version_number_says_so() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  // The last version number a pipeline can have: put in by hand, as no test publishes four billion versions.
  let db = rusqlite::Connection::open(agents.root.join(".orchester/orchester.db")).expect("fixture db");
  db.execute(
    "INSERT INTO resource_versions (id, resource_id, number, body, published_at)
     SELECT lower(hex(randomblob(16))), resource_id, 4294967295, body, published_at
     FROM resource_versions WHERE resource_id = ?1",
    [id.to_string()],
  )
  .expect("the last version");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  assert_eq!(agents.text(HINT), "No changes since v4294967295");
  agents.set(DESCRIPTION, "Changed");
  agents.until("the bar", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    agents.text(HINT),
    "Unsaved changes · Save keeps it as a draft · This pipeline has reached the highest version number."
  );
}
