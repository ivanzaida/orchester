//! A draft is always saved; only publishing refuses, and the bar says what it would refuse before the press, also while
//! the edit has changes (PV2-19): a step whose profile is gone, a step that writes with a profile that only reads, a
//! pipeline at its last version number. `cargo test -p orchester-app --test agents pipeline_save_refusals`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, RepositoryAccess, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
  pipeline_hub_ready::{body, draft_over},
};

const SAVE: &str = "pipeline-hub-save";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";
const NOTICE: &str = "pipeline-hub-bar-notice";
const CONFIRM: &str = "pipeline-hub-confirm-confirm";
const CANNOT_WRITE: &str =
  "Write access needs a profile that writes to the repository; a step cannot allow more than its profile.";

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
fn pipeline_save_refusals_a_draft_with_a_gone_profile_is_saved_and_names_the_step() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let retired = agents.profile(true);
  let id = pipeline_of(&agents, "Agents tour", [Some(builder), Some(retired)], true);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Changed");
  agents.until("the bar", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    agents.text(HINT),
    "Unsaved changes · Save keeps it as a draft · v2 comes once every step is ready"
  );
  save_until(&mut agents, "the draft kept", |s| stored(s, id) == (1, true));
  agents.until("it says so", |s| s.exists(NOTICE));
  let said = agents.text(NOTICE);
  assert!(
    said.contains("Saved as a draft of Agents tour, not published"),
    "kept, not refused: {said}"
  );
  assert!(
    said.contains("Step “Build” runs an agent profile that is no longer available. Choose another."),
    "the step by its name: {said}"
  );
  assert!(!said.contains(&retired.to_string()), "no raw id: {said}");
}

/// A published pipeline whose *Build* reads, with a draft over it in which *Build* asks to write while its profile only
/// reads: publishing refuses it.
fn writing_draft(agents: &Agents) -> ResourceId {
  let reader = agents.profile(true);
  let id = pipeline_of(agents, "Feature delivery", [Some(reader), Some(reader)], true);
  let mut steps = body([Some(reader), Some(reader)]).into_steps();
  if let StepKind::Agent { repository, .. } = &mut steps[1].kind {
    *repository = RepositoryAccess::Write;
  }
  draft_over(agents, id, Pipeline::new(steps).expect("a pipeline"));
  id
}

#[test]
fn pipeline_save_refusals_the_bar_says_what_publishing_refuses_before_the_press_and_while_editing() {
  let mut agents = Agents::new(true);
  let id = writing_draft(&agents);
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  assert_eq!(
    agents.text(HINT),
    CANNOT_WRITE,
    "the draft as stored would be refused again"
  );

  agents.set(DESCRIPTION, "Changed");
  agents.until("the bar", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    agents.text(HINT),
    format!("Unsaved changes · Save keeps it as a draft · {CANNOT_WRITE}"),
    "the edit as it stands"
  );
  save_until(&mut agents, "the draft kept", |s| s.exists(NOTICE));
  assert!(agents.text(NOTICE).contains(CANNOT_WRITE), "{}", agents.text(NOTICE));
  assert_eq!(stored(&agents, id), (1, true), "the draft is stored, nothing published");
}

#[test]
fn pipeline_save_refusals_a_pipeline_at_its_last_version_number_says_so() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)], true);
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
