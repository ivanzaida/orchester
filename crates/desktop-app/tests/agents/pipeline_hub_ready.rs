//! What keeps a pipeline from running, as the Pipelines hub says it (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): a row's
//! line, the chips under the head and *Ready?* under the flow, with *Choose →* or *Open →* on each thing missing.
//! `cargo test -p orchester-app --test agents pipeline_hub_ready`.

use std::time::{Duration, Instant};

use orchester_app::{
  routes::{HubFilter, HubView, PipelineView, PipelinesFilter, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug,
    StepKey, StepKind,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
  support::project_id,
};

/// The key of step `index` of `pipeline`'s draft or version.
pub(super) fn step_at(agents: &Agents, pipeline: ResourceId, index: usize) -> StepKey {
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
  body.steps()[index].key.clone()
}

/// The key of the first step of `pipeline`'s draft or version, *Plan*.
fn plan_step(agents: &Agents, pipeline: ResourceId) -> StepKey {
  step_at(agents, pipeline, 0)
}

/// The key of the second step of `pipeline`'s draft or version, *Build*.
fn build_step(agents: &Agents, pipeline: ResourceId) -> StepKey {
  step_at(agents, pipeline, 1)
}

fn page(page: ProjectPage) -> Route {
  Route::Project(project_id(), page)
}

#[test]
fn pipeline_hub_ready_a_draft_with_one_step_not_ready_says_so_in_its_row_chips_and_ready() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let draft = pipeline_of(&agents, "Dependency update", [Some(builder), None]);
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
  let ready = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
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
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(draft)]);
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

/// A published profile that runs on a connection the project does not have, as when the connection is removed after
/// the profile was published.
pub(super) fn lost(agents: &Agents, name: &str) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new(name).expect("name"),
    String::new(),
    PromptText::new("Build.").expect("instructions"),
    ConnectionId::generate(),
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  );
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::from_name(name).expect("slug"),
      Name::new(name).expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  id
}

/// A pipeline body whose steps *Plan* and *Build* run `profiles`.
pub(super) fn body(profiles: [Option<ResourceId>; 2]) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  for (step, profile) in ["Plan", "Build"].into_iter().zip(profiles) {
    let key = edit.add_step(Name::new(step).expect("name"), None);
    edit.update(&key, |step| {
      step.kind = StepKind::Agent {
        profile,
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      };
    });
  }
  edit.validate().expect("a pipeline")
}

/// Pipeline `name` created as its first version `body` (a pipeline is stored only as published versions, D-032).
pub(super) fn store(agents: &Agents, name: &str, body: &Pipeline) -> ResourceId {
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_published(
      Slug::from_name(name).expect("slug"),
      Name::new(name).expect("name"),
      body,
      Timestamp::now(),
    )
    .expect("the pipeline")
    .0
    .id()
}

/// Saves `draft` as the next draft of the published pipeline `id`.
pub(super) fn draft_over(agents: &Agents, id: ResourceId, draft: Pipeline) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let version = db
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("a version")
    .number();
  db.save_draft(id, &Draft::new(draft, Some(version), Timestamp::now()))
    .expect("a draft");
}

#[test]
fn pipeline_hub_ready_a_step_whose_profile_lost_its_connection_is_named_and_opens() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let lost = lost(&agents, "Explorer");
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(lost)]);
  let key = build_step(&agents, stuck);
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  let line = "02 Build runs Explorer, which has no connection";
  assert_eq!(agents.text(&format!("pipeline-hub-row-{stuck}-line")), line);
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), line);
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  let row = format!("team-hub-ready-step-{key}");
  assert_eq!(
    agents.text(&format!("{row}-words")),
    "Explorer has no connection",
    "the step that runs it"
  );
  assert!(
    !agents.exists(&format!("team-hub-ready-member-{lost}")),
    "the profile is not named again"
  );
  assert_eq!(agents.text(&format!("{row}-open")), "Open →");
  agents.click(&format!("{row}-open"));
  agents.until("the profile", |s| {
    s.window.navigation().route_now() == page(ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(lost))))
  });
}

#[test]
fn pipeline_hub_ready_every_step_whose_profile_lost_its_connection_has_a_row_and_the_line_counts_them() {
  let mut agents = Agents::new(true);
  let (first, second) = (lost(&agents, "Explorer"), lost(&agents, "Builder"));
  let stuck = pipeline_of(&agents, "Quick fix", [Some(first), Some(second)]);
  let (plan, build) = (plan_step(&agents, stuck), build_step(&agents, stuck));
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  let said = "2 steps not ready";
  assert_eq!(agents.text(&format!("pipeline-hub-row-{stuck}-line")), said);
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), said);
  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  for (key, name) in [(plan, "Explorer"), (build, "Builder")] {
    assert_eq!(
      agents.text(&format!("team-hub-ready-step-{key}-words")),
      format!("{name} has no connection")
    );
  }
}

#[test]
fn pipeline_hub_ready_a_draft_step_problem_is_listed_beside_a_step_whose_profile_lost_its_connection() {
  let mut agents = Agents::new(true);
  let lost = lost(&agents, "Explorer");
  let stuck = pipeline_of(&agents, "Quick fix", [Some(lost), Some(lost)]);
  draft_over(&agents, stuck, body([Some(lost), None]));
  let (plan, build) = (plan_step(&agents, stuck), build_step(&agents, stuck));
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  assert_eq!(
    agents.text(&format!("team-hub-ready-step-{build}-words")),
    "No agent profile"
  );
  assert_eq!(
    agents.text(&format!("team-hub-ready-step-{plan}-words")),
    "Explorer has no connection",
    "the step's problem does not hide the profile's"
  );
}

#[test]
fn pipeline_hub_ready_a_draft_over_a_published_version_says_so_and_leaves_the_version_chips_alone() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  draft_over(&agents, id, body([Some(builder), None]));
  let key = build_step(&agents, id);
  open(&mut agents, PipelinesFilter::All, Some(id));

  assert_eq!(
    agents.text("pipeline-hub-section-ready-count"),
    "1",
    "the list speaks of the version tasks use"
  );
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "v1");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "draft");
  assert_eq!(
    agents.text("pipeline-hub-chip-2-label"),
    "Ready",
    "the draft's step does not turn the published version's chip amber"
  );
  assert!(!agents.exists("pipeline-hub-chip-3"));
  assert_eq!(agents.text("team-hub-readiness-label"), "Ready? · this draft");
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(
    agents.text(&format!("team-hub-ready-step-{key}-words")),
    "No agent profile"
  );
}

#[test]
fn pipeline_hub_ready_a_published_pipeline_without_a_draft_keeps_the_plain_title() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(builder)]);
  open(&mut agents, PipelinesFilter::All, Some(id));

  assert_eq!(agents.text("team-hub-readiness-label"), "Ready?");
  assert!(!agents.exists("pipeline-hub-chip-2"), "no draft chip");
}

#[test]
fn pipeline_hub_ready_a_draft_with_no_steps_adds_one_in_place_as_the_heads_add_step() {
  let mut agents = Agents::new(true);
  let empty = store(&agents, "Empty", &Pipeline::new(Vec::new()).expect("a pipeline"));
  open(&mut agents, PipelinesFilter::All, Some(empty));
  let shown = agents.window.navigation().route_now();

  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(agents.text("team-hub-ready-draft-words"), "No steps yet");
  let head = agents.text("pipeline-hub-add-step");
  assert_eq!(
    agents.text("team-hub-ready-draft-add-step"),
    format!("{head} →"),
    "one name for one action: the head's"
  );
  agents.click("team-hub-ready-draft-add-step");
  agents.until("the step, in place", |s| {
    !s.exists("team-hub-ready-draft-add-step") && s.text("team-hub-readiness-count") == "1 missing"
  });
  assert_eq!(
    agents.window.navigation().route_now(),
    shown,
    "the panel stays where it is: nothing else opened"
  );
  assert!(
    agents.text("team-hub-actions-hint").starts_with("Unsaved changes"),
    "the step is the edit's, not stored: {}",
    agents.text("team-hub-actions-hint")
  );
}

#[test]
fn pipeline_hub_ready_a_draft_that_is_only_not_published_yet_is_not_all_set() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let fresh = store(&agents, "Fresh", &body([Some(builder), Some(builder)]));
  open(&mut agents, PipelinesFilter::All, Some(fresh));

  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{fresh}-line")),
    "Not published yet"
  );
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(agents.text("team-hub-ready-draft-words"), "Not published yet");
  assert!(
    !agents.exists("team-hub-ready-draft-add-step") && !agents.exists("team-hub-ready-draft-edit-steps"),
    "nothing to fix in the steps"
  );
}

#[test]
fn pipeline_hub_ready_steps_a_line_cannot_draw_offer_the_editor() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut steps = body([Some(builder), Some(builder)]).into_steps();
  let mut third = steps[1].clone();
  third.key = StepKey::new("third").expect("key");
  third.name = Name::new("Third").expect("name");
  // The second step waits for the first while the third starts the run too: the steps branch.
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  third.depends_on.clear();
  steps.push(third);
  let branching = store(&agents, "Branching", &Pipeline::new(steps).expect("a pipeline"));
  open(&mut agents, PipelinesFilter::All, Some(branching));

  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(
    agents.text("team-hub-ready-draft-words"),
    "These steps can't be drawn as one line"
  );
  assert_eq!(agents.text("team-hub-ready-draft-edit-steps"), "Edit steps →");
  agents.click("team-hub-ready-draft-edit-steps");
  agents.until("the editor", |s| {
    s.window.navigation().route_now() == page(ProjectPage::Pipeline(branching, PipelineView::default()))
  });
}

#[test]
fn pipeline_hub_ready_an_archived_pipeline_has_no_ready_group() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let old = pipeline_of(&agents, "Old", [Some(builder), Some(builder)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(old, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::Archived, Some(old));
  assert!(!agents.exists("team-hub-readiness-count"));
}

#[test]
fn pipeline_hub_ready_a_row_line_that_is_cut_shows_its_full_text_in_a_tooltip() {
  let mut agents = Agents::new(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  edit.add_step(
    Name::new("Reconcile the dependency lockfile with the vendored manifests").expect("name"),
    None,
  );
  let long = store(&agents, "Long", &edit.validate().expect("a pipeline"));
  let short = store(&agents, "Short", &body([None, None]));
  open(&mut agents, PipelinesFilter::All, Some(long));

  let line = format!("pipeline-hub-row-{long}-line");
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(&line)
    .and_then(|found| found.bounds())
    .expect("the line is laid out");
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

  let said = agents.text(&line);
  assert_eq!(
    agents.text(&format!("{line}-tooltip")),
    "01 Reconcile the dependency lockfile with the vendored manifests has no agent profile",
    "the tooltip holds the whole line, which the row cut to {said:?}"
  );
  assert!(
    !agents.exists(&format!("pipeline-hub-row-{short}-line-tooltip")),
    "a line that fits has no tooltip"
  );
}
