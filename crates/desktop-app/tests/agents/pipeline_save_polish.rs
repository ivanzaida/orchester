//! What the Save bar of a local pipeline's panel says and allows around a save (PV2-17): text a step's check refuses is
//! unsaved and *Save* says why at the button; a saved version says whether runs can take it, pinned over the bar; a
//! draft kept names every step not ready; a refusal of an unchanged edit can be cancelled; a draft published from
//! *Drafts* stays in view under *All*; after *Restore* the bar and the *Allow* question name the version.
//! `cargo test -p orchester-app --test agents pipeline_save_polish`.

use std::time::{Duration, Instant};

use orchester_app::{
  routes::{PipelinesFilter, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  id::ResourceId,
  resource::{Draft, Pipeline, PromptText},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, save_in},
};

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of, view},
  pipeline_hub_ready::{body, lost, store},
  pipeline_step::{keys, open_step},
};

const SAVE: &str = "pipeline-hub-save";
const CANCEL: &str = "pipeline-hub-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";
const NOTICE: &str = "pipeline-hub-bar-notice";
const CONFIRM: &str = "pipeline-hub-confirm-confirm";

/// How many versions `id` has.
fn versions(agents: &Agents, id: ResourceId) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .versions::<Pipeline>(id)
    .expect("versions")
    .len()
}

/// Rests the pointer on `id` until its tooltip shows, and gives the tooltip's words.
fn tooltip(agents: &mut Agents, id: &str, tip: &str) -> String {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .expect("laid out");
  let scale = agents.window.tree.scale_factor();
  agents.window.tree.mouse_move(
    (bounds.x + bounds.width / 2.0) * scale,
    (bounds.y + bounds.height / 2.0) * scale,
  );
  agents.window.pass();
  agents
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  agents.window.pass();
  agents.text(tip)
}

#[test]
fn pipeline_save_polish_text_the_check_refuses_is_unsaved_and_save_says_why() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let ready = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)], true);
  let [plan, _] = <[_; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);
  agents.until("the budget", |s| s.exists("pipeline-hub-step-budget"));

  agents.set("pipeline-hub-step-budget", "many");
  agents.until("the bar says what to fix", |s| s.text(HINT).starts_with("Fix"));
  let fix = agents.text(HINT);
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes"),
    "the row says the panel holds what is not saved"
  );
  assert_eq!(
    tooltip(&mut agents, SAVE, "pipeline-hub-save-why-tooltip"),
    fix,
    "Save says why at the button"
  );

  // Leaving would drop the text, so it asks.
  agents.window.navigation().go(Route::AllProjects);
  agents.until("the question", |s| s.exists("pipeline-hub-leave-discard"));
  agents.click("pipeline-hub-leave-cancel");
  agents.until("kept", |s| !s.exists("pipeline-hub-leave-discard"));

  agents.click(CANCEL);
  agents.until("nothing to fix", |s| !s.text(HINT).starts_with("Fix"));
  agents.until("the row as stored", |s| {
    !s.text(&format!("pipeline-hub-row-{ready}-line")).starts_with("Unsaved")
  });
}

#[test]
fn pipeline_save_polish_a_saved_version_that_cannot_run_says_runs_wait_over_the_bar() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let lost = lost(&agents, "Explorer");
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(lost)], true);
  open(&mut agents, PipelinesFilter::All, Some(stuck));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Still waiting for a connection");
  agents.press_until(SAVE, "v2", |s| versions(s, stuck) == 2);
  agents.until("the notice", |s| s.exists(NOTICE));
  assert_eq!(
    agents.text(NOTICE),
    "Saved as v2 · runs can start once every step is ready"
  );
  let notice = agents
    .window
    .tree
    .get_element_by_id_mut(NOTICE)
    .and_then(|found| found.bounds())
    .expect("the notice");
  let scroll = agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-panel-scroll")
    .and_then(|found| found.bounds())
    .expect("the scroll");
  assert!(
    notice.y >= scroll.y + scroll.height - 1.0,
    "pinned under the scroll area, over the bar: {notice:?} {scroll:?}"
  );
}

#[test]
fn pipeline_save_polish_a_draft_kept_names_every_step_not_ready() {
  let mut agents = Agents::new(true);
  let draft = pipeline_of(&agents, "Dependency update", [None, None], false);
  open(&mut agents, PipelinesFilter::All, Some(draft));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Waiting for profiles");
  agents.press_until(SAVE, "the draft kept", |s| s.exists(NOTICE));
  let said = agents.text(NOTICE);
  assert!(said.contains("2 steps aren't ready: Plan, Build"), "{said}");
}

#[test]
fn pipeline_save_polish_a_refusal_of_an_unchanged_draft_is_cancelled() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let fresh = store(&agents, "Fresh", &body([Some(builder), Some(builder)]));
  open(&mut agents, PipelinesFilter::All, Some(fresh));
  agents.until("the panel", |s| s.exists(SAVE));
  // Another editor stores the draft again behind the page.
  let before = OpenedPipeline::load_in(&agents.root, fresh).expect("pipeline");
  let theirs = before.body.clone().with_description("Theirs".into());
  save_in(&agents.root, &before, &theirs, &Allowed::default()).expect("their version");

  agents.click(SAVE);
  agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));
  agents.click(CANCEL);
  agents.until("the refusal is gone", |s| !s.exists("pipeline-hub-refused"));
}

#[test]
fn pipeline_save_polish_a_draft_published_from_drafts_stays_in_view_under_all() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let fresh = store(&agents, "Fresh", &body([Some(builder), Some(builder)]));
  open(&mut agents, PipelinesFilter::Drafts, Some(fresh));
  agents.until("the panel", |s| s.exists(SAVE));
  assert_eq!(agents.text(HINT), "Save publishes it as v1");

  agents.click(SAVE);
  agents.until("the question", |s| s.exists(CONFIRM));
  agents.press_until(CONFIRM, "v1", |s| versions(s, fresh) == 1);
  agents.until("the page follows it", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(fresh))
  });
  agents.until("what the save said", |s| {
    s.exists(NOTICE) && s.text(NOTICE) == "Saved as v1 · the next run uses it"
  });
}

#[test]
fn pipeline_save_polish_restore_names_the_version_until_saved_and_in_the_question() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)], true);
  // v2 tells Plan other instructions: restoring v1 changes them back, which a person allows.
  {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    let latest = db.latest_version::<Pipeline>(id).expect("read").expect("v1");
    let mut steps = latest.body().clone().into_steps();
    steps[0].instructions = PromptText::new("Plan twice.").expect("instructions");
    let body = Pipeline::new(steps).expect("a pipeline");
    db.publish_pipeline_edit(id, &body, latest.number(), None, Timestamp::now())
      .expect("v2");
  }
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  agents.click("pipeline-hub-versions-button");
  agents.until("the list", |s| s.exists("pipeline-hub-version-1"));
  agents.click("pipeline-hub-version-1");
  agents.until("v1 is shown", |s| s.exists("pipeline-hub-version-restore"));
  agents.click("pipeline-hub-version-restore");
  agents.until("the edit holds v1", |s| s.exists(SAVE));
  assert_eq!(
    agents.text(HINT),
    "Restored v1 · not saved · Save makes v3 · runs keep the version they started on"
  );

  agents.click(SAVE);
  agents.until("the question", |s| s.exists(CONFIRM));
  let asked = agents.text("pipeline-hub-confirm");
  assert!(
    asked.contains("Restoring v1 changes the pipeline: allow these changes?"),
    "the question names the restore: {asked}"
  );
  agents.press_until(CONFIRM, "v3", |s| versions(s, id) == 3);
  agents.until("saved", |s| !s.text(HINT).starts_with("Restored"));
}

/// The height of `id`.
fn height(agents: &mut Agents, id: &str) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"))
    .height
}

#[test]
fn pipeline_save_polish_save_and_cancel_stand_as_tall_whether_save_waits_or_not() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let waits = pipeline_of(&agents, "Waits", [Some(builder), None], false);
  let ready = pipeline_of(&agents, "Ready", [Some(builder), Some(builder)], true);
  for id in [waits, ready] {
    open(&mut agents, PipelinesFilter::All, Some(id));
    agents.until("the bar", |s| s.exists(SAVE));
    agents.window.pass();
    let (save, cancel) = (height(&mut agents, SAVE), height(&mut agents, CANCEL));
    assert!((save - cancel).abs() < 0.5, "Save {save} and Cancel {cancel}");
    assert!((save - 30.0).abs() < 0.5, "the design's 30: {save}");
  }
}

#[test]
fn pipeline_save_polish_an_unchanged_published_pipeline_that_cannot_run_says_nothing_changed() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let retired = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(retired)], true);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));

  assert_eq!(
    agents.text(HINT),
    "No changes since v1",
    "only what is true now: Ready? says what keeps v1 from running"
  );
  agents.set(DESCRIPTION, "Changed");
  agents.until("the edit", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    agents.text(HINT),
    "Unsaved changes · Save keeps it as a draft · v2 comes once every step is ready"
  );
}
