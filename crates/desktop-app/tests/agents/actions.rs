//! A profile's page saves its draft on its own; a widening change is asked at its field before it is saved, and
//! Publish asks again only for what was not allowed yet; Discard draft and Archive are asked in place.
use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Pipeline, PromptText, RepositoryAccess, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data::VersionState;

use super::harness::Agents;

#[test]
fn agents_create_publishes_the_first_version() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.click("local-hub-new");
  screen.until("the new profile's editor", |s| s.exists("agent-role"));
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.set("agent-custom-model", "test-model");
  screen.click("agent-create");
  screen.until("created as v1", |s| {
    s.listed().profiles.len() == 1 && !s.exists("agent-create")
  });
  assert!(matches!(
    screen.listed().profiles[0].state,
    VersionState::Published(version) if version.get() == 1
  ));
  assert!(!screen.exists("agent-publish"), "a new profile has no draft to publish");
}

#[test]
fn agents_create_with_role_instructions_is_refused_for_synthetic_input() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.click("local-hub-new");
  screen.until("the new profile's editor", |s| s.exists("agent-role"));
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.set("agent-custom-model", "test-model");
  lurq::app::events::with_synthetic_input(|| screen.click("agent-create"));
  screen.until("the refusal shown", |s| s.exists("agent-save-error"));
  assert!(
    screen.listed().profiles.is_empty(),
    "automation does not publish what the role is told"
  );
  assert!(screen.exists("agent-create"), "the profile is still unsaved");
}

#[test]
fn agents_profile_saves_its_draft_on_its_own() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  assert!(!screen.exists("agent-save"), "no Save draft button");
  screen.set("agent-responsibility", "Checks the complete change");
  screen.until("saved on its own", |s| {
    s.listed().profile(id).expect("profile").profile.responsibilities() == "Checks the complete change"
  });
  screen.saved();
  screen.press_until(
    "agent-publish",
    "published",
    |s| matches!(s.listed().profile(id).expect("profile").state, VersionState::Published(v) if v.get() == 2),
  );
}

#[test]
fn agents_widening_is_asked_at_its_field_and_waits_for_the_person() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-option-EditWorktree");
  screen.until("question at the field", |s| s.exists("agent-confirm-confirm"));
  assert!(
    screen.window.link.input_gate().protecting(),
    "window input from MCP is locked"
  );
  let repository = |s: &Agents| {
    s.listed()
      .profile(id)
      .expect("profile")
      .profile
      .permissions()
      .repository()
  };
  assert_eq!(
    repository(&screen),
    RepositoryAccess::ReadOnly,
    "nothing saved before the answer"
  );
  screen.click("agent-confirm-keep");
  assert!(!screen.exists("agent-confirm-confirm"));
  assert!(!screen.window.link.input_gate().protecting());
  assert_eq!(repository(&screen), RepositoryAccess::ReadOnly);
  // The next edit saves again, and the widening it still holds is asked again.
  screen.set("agent-responsibility", "Writes the change");
  screen.until("asked again", |s| s.exists("agent-confirm-confirm"));
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("write draft", |s| repository(s) == RepositoryAccess::Write);
  screen.saved();
  screen.press_until("agent-publish", "published without asking again", |s| {
    assert!(
      !s.exists("agent-confirm-confirm"),
      "Publish asks nothing the field allowed"
    );
    matches!(s.listed().profile(id).expect("profile").state, VersionState::Published(v) if v.get() == 2)
  });
}

#[test]
fn agents_editing_dismisses_the_question_for_the_previous_candidate() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-option-EditWorktree");
  screen.until("question", |s| s.exists("agent-confirm-confirm"));
  screen.set("agent-responsibility", "A new candidate");
  screen.until("a new question for the new candidate", |s| s.exists("agent-confirm"));
  assert!(matches!(
    screen.listed().profile(id).expect("profile").state,
    VersionState::Published(_)
  ));
}

#[test]
fn agents_discard_draft_is_asked_in_place() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-responsibility", "A draft to throw away");
  screen.until("draft saved", |s| {
    s.listed().profile(id).expect("profile").state.has_draft()
  });
  screen.saved();
  screen.press_until("agent-discard", "question", |s| s.exists("hub-discard-confirm"));
  screen.click("hub-discard-keep");
  assert!(
    screen.listed().profile(id).expect("profile").state.has_draft(),
    "Keep draft keeps it"
  );
  screen.press_until("agent-discard", "question", |s| s.exists("hub-discard-confirm"));
  screen.press_until(
    "hub-discard-confirm",
    "discarded",
    |s| matches!(s.listed().profile(id).expect("profile").state, VersionState::Published(v) if v.get() == 1),
  );
  screen.until("fields from v1", |s| {
    s.exists("agent-responsibility") && s.value("agent-responsibility") == "Builds the change"
  });
}

#[test]
fn agents_archive_a_profile_nothing_uses() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-more");
  let menu = screen
    .window
    .tree
    .get_element_by_id_mut("agent-more-menu")
    .and_then(|e| e.bounds())
    .expect("menu laid out");
  assert!(menu.width < 300.0, "the … menu stays beside its button: {menu:?}");
  screen.click("agent-archive");
  screen.until("the confirmation, with what uses it read", |s| {
    s.exists("team-hub-archive-confirm") && s.exists("team-hub-archive-used-list")
  });
  assert!(
    screen.text("team-hub-archive-body").contains("leaves every list"),
    "{}",
    screen.text("team-hub-archive-body")
  );
  screen.press_until("team-hub-archive-confirm", "archived", |s| {
    s.listed().profile(id).is_none() && s.exists("local-hub-filter")
  });
}

#[test]
fn agents_archive_a_profile_a_pipeline_step_uses_leaves_the_step_naming_it() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let pipeline = assigned_pipeline(&screen, id);
  screen.open(id);
  screen.click("agent-more");
  screen.click("agent-archive");
  screen.until("the confirmation lists the pipeline", |s| {
    s.exists("team-hub-archive-confirm") && s.exists("team-hub-archive-used-0-name")
  });
  assert!(screen.text("team-hub-archive-used-0-name").contains("Delivery"));
  screen.click("team-hub-archive-cancel");
  assert!(!screen.exists("team-hub-archive-confirm"), "Cancel closes the question");
  assert!(screen.listed().profile(id).is_some(), "Cancel keeps the profile");
  screen.click("agent-more");
  screen.click("agent-archive");
  screen.press_until("team-hub-archive-confirm", "archived", |s| {
    s.listed().profile(id).is_none() && s.exists("local-hub-filter")
  });
  // The step keeps naming the profile, in the draft and in v1: readiness says the pipeline is not ready instead.
  let db = ProjectDb::open(&screen.root).expect("database");
  assert!(
    db.draft::<Pipeline>(pipeline).expect("draft").is_none(),
    "no pipeline is reset"
  );
  let published = db.latest_version::<Pipeline>(pipeline).expect("latest").expect("v1");
  assert!(
    matches!(&published.body().steps()[0].kind, StepKind::Agent { profile: Some(p), .. } if *p == id),
    "the step still names the profile"
  );
}

/// A published Delivery pipeline whose one step runs `profile`.
fn assigned_pipeline(screen: &Agents, profile: ResourceId) -> ResourceId {
  assigned_pipeline_as(screen, profile, ("delivery", "Delivery"))
}

/// A published pipeline named `title` whose one step runs `profile`.
fn assigned_pipeline_as(screen: &Agents, profile: ResourceId, (slug, title): (&str, &str)) -> ResourceId {
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("implement").expect("step key"),
    name: Name::new("Implement").expect("step name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Implement.").expect("instructions"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let mut db = ProjectDb::open(&screen.root).expect("database");
  let resource = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(title).expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline saves");
  db.publish_draft::<Pipeline>(resource.id(), Timestamp::now())
    .expect("pipeline v1");
  resource.id()
}

#[test]
fn agents_invalid_fields_and_concurrent_changes_are_visible() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-role", "");
  screen.until("invalid", |s| s.exists("agent-validation-error"));
  let entry = screen.listed().profile(id).expect("profile").clone();
  orchester_workspace::agents::data::apply_in(
    &screen.root,
    orchester_workspace::agents::data::ProfileChange::SaveDraft(id, super::harness::changed(&entry.profile)),
  )
  .expect("outside save");
  screen.set("agent-role", "Builder changed");
  screen.until("conflict", |s| {
    s.exists("agent-save-error") || s.exists("agent-outside-change")
  });
}
