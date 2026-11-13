//! A change that cannot be published is not saved: a pipeline holds no draft (D-032). *Save* is off and says why
//! before the press: the step whose profile is gone, by its number and name, how many steps are not ready, or that the
//! pipeline has its last version number; a press stores nothing. `cargo test -p orchester-app --test agents
//! pipeline_save_refusals`.

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

#[test]
fn pipeline_save_refusals_a_change_while_a_profile_is_gone_cannot_be_saved_and_names_the_step() {
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

  // The change cannot be published (D-032): Save is off and names the step, by its number and name, never an id.
  agents.set(DESCRIPTION, "Changed");
  agents.until("the bar", |s| s.text(HINT) == "Choose a profile for 02 Build to save");
  pressed_off(&mut agents);
  assert_eq!(stored(&agents, id), (1, false), "nothing stored: no draft, no version");
  assert_eq!(agents.value(DESCRIPTION), "Changed", "what was typed stays");
}

#[test]
fn pipeline_save_refusals_save_off_counts_every_step_not_ready() {
  let mut agents = Agents::new(true);
  let (planner, builder) = (agents.profile(true), agents.profile(true));
  let id = pipeline_of(&agents, "Dependency update", [Some(planner), Some(builder)]);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  for retired in [planner, builder] {
    db.archive_local_resource(retired, Timestamp::now()).expect("archived");
  }
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Waiting for profiles");
  agents.until("the bar", |s| s.text(HINT) == "Make 2 steps ready to save");
  pressed_off(&mut agents);
  assert_eq!(stored(&agents, id), (1, false), "nothing stored");
}

/// Presses *Save* while it is off: nothing is asked or refused, as nothing is tried.
fn pressed_off(agents: &mut Agents) {
  agents.click(SAVE);
  agents.window.pass();
  assert!(!agents.exists(REFUSED), "a Save that is off tries nothing");
  assert!(!agents.exists(CONFIRM), "and asks nothing");
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
  agents.until("the bar", |s| s.text(HINT) != "No changes since v4294967295");
  assert_eq!(
    agents.text(HINT),
    "This pipeline has reached the highest version number.",
    "Save is off and says why: nothing is stored"
  );
}
