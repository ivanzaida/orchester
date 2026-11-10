//! Every problem the Pipelines hub names has a way to its fix, in the same words wherever it is named (PV2-17): steps a
//! line cannot draw still name the step the list line names; *Save →* on a draft that is only not published saves
//! as the bar's *Save* does; a card
//! whose profile lost its connection stands in amber with *Open profile*; under *Ready? · this draft* what the
//! published version runs and the draft does not is left out, and a row about that version names it.
//! `cargo test -p orchester-app --test agents pipeline_hub_ready_fixes`.

use orchester_app::routes::{HubFilter, HubView, PipelinesFilter, ProjectPage, Route};
use orchester_core::{
  name::Name,
  resource::{Pipeline, StepKey},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
  pipeline_hub_ready::{body, draft_over, lost, step_at, store},
  support::project_id,
};

const CONFIRM: &str = "pipeline-hub-confirm-confirm";

#[test]
fn pipeline_hub_ready_fixes_steps_a_line_cannot_draw_still_name_the_step_the_list_line_names() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut steps = body([Some(builder), None]).into_steps();
  let mut third = steps[0].clone();
  third.key = StepKey::new("third").expect("key");
  third.name = Name::new("Third").expect("name");
  // *Build* waits for *Plan* while *Third* starts the run too: the steps branch.
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  third.depends_on.clear();
  steps.push(third);
  let branching = store(&agents, "Branching", &Pipeline::new(steps).expect("a pipeline"));
  let build = step_at(&agents, branching, 1);
  open(&mut agents, PipelinesFilter::All, Some(branching));

  let line = agents.text(&format!("pipeline-hub-row-{branching}-line"));
  assert_eq!(line, "02 Build has no agent profile");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "1 step not ready");
  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  assert_eq!(
    agents.text("team-hub-ready-draft-words"),
    "These steps can't be drawn as one line"
  );
  let row = format!("team-hub-ready-step-{build}");
  assert_eq!(
    agents.text(&format!("{row}-label")),
    "Step 02 Build",
    "numbered as the line numbers it"
  );
  assert_eq!(agents.text(&format!("{row}-words")), "No agent profile");
  assert_eq!(agents.text(&format!("{row}-choose")), "Choose →");
}

#[test]
fn pipeline_hub_ready_fixes_a_draft_only_not_published_offers_save_which_saves_as_the_bars_save() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let fresh = store(&agents, "Fresh", &body([Some(builder), Some(builder)]));
  open(&mut agents, PipelinesFilter::All, Some(fresh));

  assert_eq!(agents.text("team-hub-ready-draft-words"), "Not published yet");
  assert_eq!(agents.text("team-hub-ready-draft-save"), "Save →");
  agents.click("team-hub-ready-draft-save");
  // A first version widens what nobody allowed yet: the bar's question asks, as after the bar's own Save.
  agents.until("the question", |s| s.exists(CONFIRM));
  agents.press_until(CONFIRM, "v1", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .latest_version::<Pipeline>(fresh)
      .expect("read")
      .is_some()
  });
}

#[test]
fn pipeline_hub_ready_fixes_a_card_whose_profile_lost_its_connection_is_amber_and_opens_it() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let lost = lost(&agents, "Explorer");
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(lost)], true);
  let build = step_at(&agents, stuck, 1);
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  let card = format!("pipeline-hub-flow-step-{build}");
  assert_eq!(agents.text(&format!("{card}-runs")), "Explorer has no connection");
  assert_eq!(agents.text(&format!("{card}-fix-choose-words")), "Open profile");
  agents.click(&format!("{card}-fix"));
  agents.until("the profile", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(lost))),
      )
  });
}

#[test]
fn pipeline_hub_ready_fixes_this_draft_leaves_out_what_only_the_version_runs_and_names_the_version() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let old = agents.profile(true);
  let dropped = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(old)], true);
  let kept = pipeline_of(&agents, "Quick fix", [Some(builder), Some(old)], true);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(old, Timestamp::now())
    .expect("archived");
  // One draft no longer runs the archived profile, the other still does.
  draft_over(&agents, dropped, body([Some(builder), Some(builder)]));
  draft_over(&agents, kept, body([Some(old), Some(builder)]));

  open(&mut agents, PipelinesFilter::All, Some(dropped));
  assert_eq!(agents.text("team-hub-readiness-label"), "Ready? · this draft");
  assert_eq!(
    agents.text("team-hub-readiness-count"),
    "all set",
    "what keeps the version from running is not the draft's"
  );

  open(&mut agents, PipelinesFilter::All, Some(kept));
  agents.until("the version's row", |s| s.exists("team-hub-ready-undecided"));
  assert_eq!(
    agents.text("team-hub-ready-undecided-label"),
    "Profiles and tools · v1",
    "a row about the version names it"
  );
}
