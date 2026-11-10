//! What keeps a pipeline from running, as the Pipelines hub says it (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): a row's
//! line, the chips under the head and *Ready?* under the flow, with *Choose →* or *Open →* on each thing missing.
//! `cargo test -p orchester-app --test agents pipeline_hub_ready`.

use orchester_app::routes::{HubFilter, HubView, PipelineView, PipelinesFilter, ProjectPage, Route};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug, StepKey},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
  support::project_id,
};

/// The key of the second step of `pipeline`'s draft or version, *Build*.
fn build_step(agents: &Agents, pipeline: ResourceId) -> StepKey {
  let db = ProjectDb::open(&agents.root).expect("database");
  let body = match db.draft::<Pipeline>(pipeline).expect("read") {
    Some(draft) => draft.body().clone(),
    None => db
      .latest_version::<Pipeline>(pipeline)
      .expect("read")
      .expect("a version")
      .body()
      .clone(),
  };
  body.steps()[1].key.clone()
}

fn page(page: ProjectPage) -> Route {
  Route::Project(project_id(), page)
}

#[test]
fn pipeline_hub_ready_a_draft_with_one_step_not_ready_says_so_in_its_row_chips_and_ready() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let draft = pipeline_of(&agents, "Dependency update", [Some(builder), None], false);
  let key = build_step(&agents, draft);
  open(&mut agents, PipelinesFilter::All, Some(draft));

  assert_eq!(agents.text("pipeline-hub-section-needs-count"), "1");
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{draft}-line")),
    "02 Build has no agent profile"
  );
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "draft");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "1 step not ready");
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  let row = format!("team-hub-ready-step-{key}");
  assert_eq!(agents.text(&format!("{row}-words")), "No agent profile");
  assert_eq!(agents.text(&format!("{row}-choose")), "Choose →");

  agents.click(&format!("{row}-choose"));
  agents.until("the step", |s| {
    s.window.navigation().route_now() == page(ProjectPage::Pipeline(draft, PipelineView::Step(key.clone())))
  });
}

#[test]
fn pipeline_hub_ready_a_pipeline_that_can_run_is_all_set_with_one_summary_row() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let ready = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)], true);
  open(&mut agents, PipelinesFilter::All, Some(ready));

  assert_eq!(agents.text("team-hub-readiness-count"), "all set");
  assert_eq!(agents.text("team-hub-ready-summary-words"), "Builder · ready");
  assert!(
    !agents.exists("team-hub-ready-summary-choose") && !agents.exists("team-hub-ready-summary-open"),
    "nothing to fix"
  );
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "Ready");
  assert_eq!(agents.text("pipeline-hub-section-ready-count"), "1");
}

#[test]
fn pipeline_hub_ready_a_step_whose_profile_is_not_published_opens_it_in_the_agents_hub() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let draft = agents.profile(false);
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(draft)], false);
  let key = build_step(&agents, stuck);
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "1 step not ready");
  let row = format!("team-hub-ready-step-{key}");
  assert_eq!(agents.text(&format!("{row}-words")), "Profile not published yet");
  assert_eq!(agents.text(&format!("{row}-open")), "Open →");
  agents.click(&format!("{row}-open"));
  agents.until("the profile", |s| {
    s.window.navigation().route_now() == page(ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(draft))))
  });
}

#[test]
fn pipeline_hub_ready_a_profile_that_lost_its_connection_says_so_and_opens_the_profile() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let lost = {
    let profile = AgentProfile::new(
      Name::new("Lost").expect("name"),
      String::new(),
      PromptText::new("Build.").expect("instructions"),
      ConnectionId::generate(),
      ModelId::new("test-model").expect("model"),
      ProviderSettings::default(),
    );
    let mut db = ProjectDb::open(&agents.root).expect("database");
    // The store takes a version whose connection is gone, as when the connection is removed after it was published.
    let id = db
      .create_resource(
        Slug::new("lost").expect("slug"),
        Name::new("Lost").expect("name"),
        &profile,
        Timestamp::now(),
      )
      .expect("profile")
      .id();
    db.publish_draft::<AgentProfile>(id, Timestamp::now())
      .expect("published");
    id
  };
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(lost)], true);
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  let said = "A profile its steps run has no connection";
  assert_eq!(agents.text(&format!("pipeline-hub-row-{stuck}-line")), said);
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), said);
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(agents.text("team-hub-ready-undecided-words"), said);
  agents.click("team-hub-ready-undecided-open");
  agents.until("the profile", |s| {
    s.window.navigation().route_now() == page(ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(lost))))
  });
}

#[test]
fn pipeline_hub_ready_an_archived_pipeline_has_no_ready_group() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let old = pipeline_of(&agents, "Old", [Some(builder), Some(builder)], true);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(old, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::Archived, Some(old));
  assert!(!agents.exists("team-hub-readiness-count"));
}
