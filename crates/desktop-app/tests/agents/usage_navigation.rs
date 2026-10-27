//! Cross-feature navigation uses the real row action and independent fixture databases.
use std::collections::BTreeSet;

use orchester_app::routes::{AgentTab, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{AgentProfile, Draft, Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

/// The profile's page, where a held link leaves the editor; since ORC-107 *Used in pipelines* is a section of it.
fn used_by(profile: ResourceId) -> Route {
  Route::Project(project_id(), ProjectPage::Agent(profile, AgentTab::default()))
}

fn used_profile() -> (Agents, ResourceId, ResourceId) {
  let screen = Agents::new(true);
  let profile = screen.profile(true);
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("implement").expect("a valid step key"),
    name: Name::new("Implement").expect("a valid step name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Implement.").expect("valid instructions"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("one assigned pipeline step");
  let mut db = ProjectDb::open(&screen.root).expect("the isolated project opens");
  let resource = db
    .create_resource(
      Slug::new("linked-delivery").expect("valid slug"),
      Name::new("Linked delivery").expect("valid name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("the fixture pipeline saves");
  let version = db
    .publish_draft::<Pipeline>(resource.id(), Timestamp::now())
    .expect("pipeline v1 publishes");
  db.save_draft(
    resource.id(),
    &Draft::new(
      pipeline.with_description("Current draft, not historical version".into()),
      Some(version.number()),
      Timestamp::now(),
    ),
  )
  .expect("a distinct current draft saves");
  (screen, profile, resource.id())
}
fn current_pipeline(screen: &mut Agents, pipeline: ResourceId) {
  assert_eq!(
    screen.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Pipeline(pipeline, Default::default()))
  );
  screen.until("actual current pipeline editor", |s| s.exists("pipeline-canvas"));
  assert!(screen.text("screen-pipelines").contains("Linked delivery"));
  let db = ProjectDb::open(&screen.root).expect("fixture opens after navigation");
  assert_eq!(
    db.draft::<Pipeline>(pipeline)
      .expect("draft reads")
      .expect("draft retained")
      .body()
      .description(),
    "Current draft, not historical version"
  );
  assert_eq!(
    db.latest_version::<Pipeline>(pipeline)
      .expect("version reads")
      .expect("v1 retained")
      .number()
      .get(),
    1
  );
}

#[test]
fn agents_used_link_opens_exact_current_pipeline_from_published_and_draft_rows() {
  for row in [0, 1] {
    let (mut screen, profile, pipeline) = used_profile();
    let original = screen
      .listed()
      .profile(profile)
      .expect("profile exists")
      .profile
      .clone();
    screen.open(profile);
    screen.click(&format!("agent-use-open-{row}"));
    current_pipeline(&mut screen, pipeline);
    assert_eq!(
      screen.listed().profile(profile).expect("profile retained").profile,
      original
    );
  }
}
#[test]
fn agents_used_link_waits_until_the_edit_is_saved() {
  let (mut screen, profile, pipeline) = used_profile();
  screen.open(profile);
  screen.set("agent-responsibility", "Keep my unsaved change");
  // Before the edit rests and saves, the links keep the page where it is.
  screen.click("agent-use-open-0");
  assert_eq!(screen.window.navigation().route_now(), used_by(profile));
  assert!(screen.text("agent-editor").contains("Save this profile’s changes"));
  assert_eq!(screen.value("agent-responsibility"), "Keep my unsaved change");
  screen.saved();
  screen.until("the saved profile's links", |s| s.exists("agent-use-open-1"));
  assert_eq!(
    screen
      .listed()
      .profile(profile)
      .expect("saved profile")
      .profile
      .responsibilities(),
    "Keep my unsaved change"
  );
  // The link opens once the page has read the saved profile again.
  screen.press_until("agent-use-open-1", "the pipeline opens", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::Pipeline(..))
    )
  });
  current_pipeline(&mut screen, pipeline);
}
#[test]
fn agents_used_link_keeps_pending_person_question_and_storage_unchanged() {
  let (mut screen, profile, _) = used_profile();
  let original = screen
    .listed()
    .profile(profile)
    .expect("profile exists")
    .profile
    .clone();
  screen.open(profile);
  screen.click("agent-option-EditWorktree");
  screen.until("person question", |s| s.exists("agent-confirm-confirm"));
  screen.click("agent-use-open-0");
  assert_eq!(screen.window.navigation().route_now(), used_by(profile));
  assert!(screen.exists("agent-confirm-confirm"));
  assert!(screen.window.link.input_gate().protecting());
  assert!(
    screen
      .text("agent-editor")
      .contains("Finish the pending profile action")
  );
  assert_eq!(
    screen
      .listed()
      .profile(profile)
      .expect("unchanged stored profile")
      .profile,
    original
  );
}
#[test]
fn agents_used_link_keeps_clean_stale_publish_question_without_answering_it() {
  let (mut screen, profile, _) = used_profile();
  let mut db = ProjectDb::open(&screen.root).expect("fixture opens");
  let version = db
    .latest_version::<AgentProfile>(profile)
    .expect("read v1")
    .expect("v1 exists");
  let changed = super::harness::changed(version.body());
  db.save_draft(
    profile,
    &Draft::new(changed.clone(), Some(version.number()), Timestamp::now()),
  )
  .expect("draft saves");
  db.publish_draft::<AgentProfile>(profile, Timestamp::now())
    .expect("v2 publishes");
  db.save_draft(profile, &Draft::new(changed, Some(version.number()), Timestamp::now()))
    .expect("stale draft saves");
  screen.open(profile);
  screen.click("agent-publish");
  screen.until("stale question", |s| s.exists("agent-confirm-confirm"));
  screen.click("agent-use-open-1");
  assert_eq!(screen.window.navigation().route_now(), used_by(profile));
  assert!(screen.exists("agent-confirm-confirm"));
  assert!(screen.window.link.input_gate().protecting());
  assert!(
    db.draft::<AgentProfile>(profile)
      .expect("read unchanged draft")
      .is_some()
  );
  assert_eq!(
    db.latest_version::<AgentProfile>(profile)
      .expect("read unchanged version")
      .expect("v2")
      .number()
      .get(),
    2
  );
}
