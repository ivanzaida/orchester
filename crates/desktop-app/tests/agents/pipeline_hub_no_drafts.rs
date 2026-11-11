//! The local Pipelines hub without drafts (PV2-31, D-032; `Pipelines · v2` P1 `s7LMyQ`, P7 `mFNXi`, P8 `vujkk`, P12
//! `nSr5q`): no *Drafts* tab, and a route that names it opens *All*; *Save* is off and says why while the edit cannot
//! be published, and nothing is stored; a refused save stores nothing; an unsaved edit with a step not ready stays
//! under *Ready* and says so in its row, its chips, its head and *Ready?*; *Back* from a version names the edit or the
//! latest version, and what the last save said waits while a version is shown; leaving an unsaved edit asks first, and
//! says to press *Save* only while *Save* can keep it. `cargo test -p orchester-app --test agents
//! pipeline_hub_no_drafts`.

use std::time::{Duration, Instant};

use orchester_app::{
  routes::{PipelinesFilter, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore, time::Timestamp};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, save_here_in},
};

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of, view},
  pipeline_reorder::open_menu,
  pipeline_step::keys,
  pipeline_versions::{BUTTON, open_list, publish_second},
  support::project_id,
};

const SAVE: &str = "pipeline-hub-save";
const CANCEL: &str = "pipeline-hub-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";
const ADD_STEP: &str = "pipeline-hub-add-step";

/// How many versions `id` has.
fn versions(agents: &Agents, id: ResourceId) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .versions::<Pipeline>(id)
    .expect("versions")
    .len()
}

/// A project with *Feature delivery*, whose steps *Plan* and *Build* run a published profile, open in the hub.
fn ready() -> (Agents, ResourceId) {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));
  (agents, id)
}

/// Rests the pointer on *Save* until its tooltip shows, and gives the tooltip's words.
fn save_tooltip(agents: &mut Agents) -> String {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(SAVE)
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
  agents.text("pipeline-hub-save-why-tooltip")
}

/// Presses *Save* while it is off and lets the page settle: nothing is asked, said or stored.
fn press_save(agents: &mut Agents, id: ResourceId) {
  let before = versions(agents, id);
  agents.click(SAVE);
  for _ in 0..5 {
    agents.window.pass();
  }
  assert!(
    !agents.exists("pipeline-hub-refused"),
    "a Save that is off asks nothing"
  );
  assert!(!agents.exists("pipeline-hub-bar-notice"), "and says nothing was saved");
  assert_eq!(versions(agents, id), before, "nothing is stored");
}

#[test]
fn pipeline_hub_no_drafts_the_filter_is_all_and_archived_and_a_drafts_route_opens_all() {
  let (mut agents, id) = ready();
  assert!(agents.exists("pipeline-hub-filter-all"));
  assert!(agents.exists("pipeline-hub-filter-archived"));
  assert!(!agents.exists("pipeline-hub-filter-drafts"), "no Drafts tab");
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "1");

  let path = format!(
    "{}?filter=drafts&item={id}",
    Route::Project(project_id(), ProjectPage::Pipelines).path()
  );
  let kept = Route::parse(&path).expect("a route kept from before still reads");
  agents.window.navigation().go(kept);
  agents.until("All with the pipeline", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(id))
  });
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
  assert!(!agents.exists("pipeline-hub-filter-drafts"));
}

#[test]
fn pipeline_hub_no_drafts_a_tab_keeps_the_pipeline_it_holds_and_a_stale_route_is_corrected() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let kept = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  let gone = pipeline_of(&agents, "Old flow", [Some(builder), Some(builder)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(gone, Timestamp::now())
    .expect("archived");

  open(&mut agents, PipelinesFilter::All, Some(kept));
  agents.click("pipeline-hub-filter-archived");
  agents.until("Archived, which does not hold it", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::Archived, None)
  });
  assert!(agents.exists(&format!("pipeline-hub-row-{gone}")));
  assert!(!agents.exists(&format!("pipeline-hub-row-{kept}")));

  open(&mut agents, PipelinesFilter::Archived, Some(kept));
  agents.until("the corrected route", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::Archived, Some(gone))
  });
  agents.click("pipeline-hub-filter-all");
  agents.until("All, with the first pipeline", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, None)
  });
}

#[test]
fn pipeline_hub_no_drafts_an_unsaved_edit_with_a_step_not_ready_stays_under_ready_and_save_is_off() {
  let (mut agents, id) = ready();
  agents.click(ADD_STEP);
  agents.until("the edit holds the new step", |s| {
    s.text("pipeline-hub-steps-count") == "3"
  });

  let row = format!("pipeline-hub-row-{id}");
  assert!(agents.exists("pipeline-hub-section-ready"));
  assert!(
    !agents.exists("pipeline-hub-section-needs"),
    "an edit moves no pipeline between groups"
  );
  assert_eq!(
    agents.text(&format!("{row}-line")),
    "Unsaved changes · 1 step not ready"
  );
  assert_eq!(agents.text(&format!("{row}-version")), "v1");
  assert_eq!(agents.text("team-hub-panel-version"), "v1 · unsaved changes");
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "1 step not ready");
  assert!(
    !agents.exists("pipeline-hub-chip-1"),
    "the edit has no version of its own yet"
  );
  assert_eq!(agents.text("team-hub-readiness-label"), "Ready?");
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing · Save stays off");

  let why = "Choose a profile for 03 New step to save";
  assert_eq!(agents.text(HINT), why);
  assert_eq!(save_tooltip(&mut agents), why, "Save says why at the button");
  press_save(&mut agents, id);

  agents.click(CANCEL);
  agents.until("the stored version", |s| s.text("pipeline-hub-steps-count") == "2");
  assert_eq!(agents.text(&format!("{row}-line")), "2 steps · not used yet");
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "v1");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "Ready");
  assert_eq!(agents.text("team-hub-readiness-count"), "all set");
}

#[test]
fn pipeline_hub_no_drafts_save_says_every_reason_it_is_off() {
  let (mut agents, id) = ready();
  agents.click(ADD_STEP);
  agents.until("one new step", |s| s.text("pipeline-hub-steps-count") == "3");
  agents.click(ADD_STEP);
  agents.until("two new steps", |s| s.text("pipeline-hub-steps-count") == "4");
  assert_eq!(agents.text(HINT), "Make 2 steps ready to save");
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "2 steps not ready");
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{id}-line")),
    "Unsaved changes · 2 steps not ready"
  );
  press_save(&mut agents, id);
  agents.click(CANCEL);
  agents.until("the stored version", |s| s.text("pipeline-hub-steps-count") == "2");

  for key in keys(&agents, id) {
    open_menu(&mut agents, &key);
    agents.click("pipeline-hub-flow-menu-remove");
    agents.until("the step gone", |s| !s.exists(&format!("pipeline-hub-flow-step-{key}")));
  }
  assert_eq!(agents.text(HINT), "Add a step to save");
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing · Save stays off");
  assert_eq!(agents.text("team-hub-ready-steps-words"), "No steps yet");
  press_save(&mut agents, id);
}

#[test]
fn pipeline_hub_no_drafts_a_change_to_a_version_whose_profile_is_gone_cannot_be_saved() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(
    &agents,
    "Dependency update",
    [Some(builder), Some(ResourceId::generate())],
  );
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(SAVE));
  assert!(
    agents.exists("pipeline-hub-section-needs"),
    "the stored version needs the person"
  );
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{id}-line")),
    "02 Build runs a profile that is no longer available"
  );
  assert_eq!(agents.text(HINT), "No changes since v1");

  agents.set(DESCRIPTION, "Bump one dependency");
  agents.until("the change", |s| s.text(HINT) != "No changes since v1");
  assert_eq!(agents.text(HINT), "Choose a profile for 02 Build to save");
  press_save(&mut agents, id);
}

#[test]
fn pipeline_hub_no_drafts_a_refused_save_stores_nothing_and_keeps_the_edit() {
  let (mut agents, id) = ready();
  agents.set(DESCRIPTION, "Mine");
  agents.until("the change", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    agents.text(HINT),
    "Unsaved changes · Save makes v2 · runs keep the version they started on"
  );

  // Another writer saves a version behind the page: the edit started from v1, so its save is refused.
  let before = OpenedPipeline::load_in(&agents.root, id).expect("pipeline");
  let theirs = before.body.clone().with_description("Theirs".into());
  let name = orchester_core::name::Name::new(&before.name).expect("a stored name");
  save_here_in(&agents.root, &before, &theirs, &name, &Allowed::default()).expect("their version");
  assert_eq!(versions(&agents, id), 2);

  agents.click(SAVE);
  agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));
  assert!(
    agents
      .text("pipeline-hub-refused-title")
      .starts_with("Couldn't save Feature delivery: "),
    "{}",
    agents.text("pipeline-hub-refused-title")
  );
  assert_eq!(versions(&agents, id), 2, "nothing was stored");
  assert_eq!(agents.value(DESCRIPTION), "Mine", "what was typed stays");
}

#[test]
fn pipeline_hub_no_drafts_back_from_a_version_names_the_edit_or_the_latest_version() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  publish_second(&agents, id, "Plan, then build");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel with Versions", |s| s.exists(SAVE) && s.exists(BUTTON));
  assert!(agents.text("team-hub-panel-version").starts_with("v2 · published "));

  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("v1 read-only", |s| s.exists("pipeline-hub-version-back"));
  assert_eq!(agents.text("pipeline-hub-version-back-label"), "Back to v2");
  agents.click("pipeline-hub-version-back");
  agents.until("the panel", |s| s.exists(SAVE));

  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("held", |s| s.text(HINT).starts_with("Unsaved changes"));
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("v1 read-only", |s| s.exists("pipeline-hub-version-back"));
  assert_eq!(agents.text("pipeline-hub-version-back-label"), "Back to your changes");
}

/// Leaves the hub for All projects and gives what the question says it loses and how to keep it.
fn leave_question(agents: &mut Agents) -> String {
  agents.window.navigation().go(Route::AllProjects);
  agents.until("the question", |s| s.exists("pipeline-hub-leave-discard"));
  assert_eq!(
    agents.text("pipeline-hub-leave-title"),
    "Discard the unsaved changes to Feature delivery?"
  );
  agents.text("pipeline-hub-leave-what")
}

#[test]
fn pipeline_hub_no_drafts_leaving_an_edit_save_can_keep_asks_to_press_save() {
  let (mut agents, id) = ready();
  agents.set(DESCRIPTION, "Mine");
  agents.until("the change", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(
    leave_question(&mut agents),
    "Opening anything else now puts Feature delivery back as it was saved. Keep editing and press Save to keep the \
     changes."
  );
  agents.click("pipeline-hub-leave-discard");
  agents.until("gone", |s| s.window.navigation().route_now() == Route::AllProjects);
  assert_eq!(versions(&agents, id), 1, "nothing was stored");
}

#[test]
fn pipeline_hub_no_drafts_leaving_an_edit_save_waits_for_asks_to_fix_it_first() {
  let (mut agents, id) = ready();
  agents.click(ADD_STEP);
  agents.until("the edit", |s| s.text("pipeline-hub-steps-count") == "3");
  assert_eq!(
    leave_question(&mut agents),
    "Opening anything else now puts Feature delivery back as it was saved. Keep editing and fix what Save waits for \
     to keep the changes.",
    "Save is off, so the question does not send the person to it"
  );
  agents.click("pipeline-hub-leave-discard");
  agents.until("gone", |s| s.window.navigation().route_now() == Route::AllProjects);
  assert_eq!(versions(&agents, id), 1, "nothing was stored");
}

#[test]
fn pipeline_hub_no_drafts_what_the_last_save_said_waits_while_a_version_is_shown() {
  const NOTICE: &str = "pipeline-hub-bar-notice";
  let (mut agents, id) = ready();
  agents.set(DESCRIPTION, "Plan, then build");
  agents.until("the change", |s| s.text(HINT).starts_with("Unsaved changes"));
  agents.press_until(SAVE, "v2", |s| versions(s, id) == 2);
  agents.until("what the save said", |s| s.exists(NOTICE));

  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("v1 read-only", |s| s.exists("pipeline-hub-version-back"));
  assert!(
    !agents.exists(NOTICE),
    "the bar is not there, nor what the last save said of it"
  );

  agents.click("pipeline-hub-version-back");
  agents.until("the panel", |s| s.exists(SAVE));
  assert!(agents.exists(NOTICE), "back with the bar");
}
