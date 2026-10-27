//! A profile's page saves its draft on its own; a widening change is asked at its field before it is saved, and
//! Publish asks again only for what was not allowed yet; Discard draft and Remove are asked in place.
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
fn agents_create_first_draft_and_publish() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.click("agents-new");
  screen.until("the new profile's editor", |s| s.exists("agent-role"));
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.set("agent-custom-model", "test-model");
  screen.click("agent-create");
  screen.until("created", |s| {
    s.listed().profiles.len() == 1 && s.exists("agent-publish") && !s.exists("agent-create")
  });
  screen.click("agent-publish");
  screen.until("first version", |s| {
    matches!(s.listed().profiles[0].state, VersionState::Published(_))
  });
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
fn agents_remove_an_unused_profile() {
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
  screen.click("agent-remove");
  screen.until("question", |s| s.exists("hub-remove-confirm"));
  assert!(
    screen.text("hub-remove").contains("No pipeline step uses it"),
    "{}",
    screen.text("hub-remove")
  );
  screen.press_until("hub-remove-confirm", "removed", |s| {
    s.listed().profile(id).is_none() && s.exists("agents-new")
  });
}

#[test]
fn agents_remove_and_reset_a_profile_a_pipeline_step_uses() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let pipeline = assigned_pipeline(&screen, id);
  screen.open(id);
  screen.click("agent-more");
  screen.click("agent-remove");
  screen.until("question with its users", |s| {
    s.exists("hub-remove-confirm") && s.text("hub-remove").contains("Delivery v1 · 01 Implement")
  });
  assert!(screen.text("hub-remove-confirm").contains("Remove and reset"));
  screen.click("hub-remove-keep");
  assert!(!screen.exists("hub-remove-confirm"), "Keep it closes the question");
  assert!(screen.listed().profile(id).is_some(), "Keep it keeps the profile");
  screen.click("agent-more");
  screen.click("agent-remove");
  screen.until("asked again", |s| {
    s.exists("hub-remove") && s.text("hub-remove").contains("Delivery v1")
  });
  screen.press_until("hub-remove-confirm", "removed", |s| {
    s.listed().profile(id).is_none() && s.exists("agents-new")
  });
  let db = ProjectDb::open(&screen.root).expect("database");
  let draft = db.draft::<Pipeline>(pipeline).expect("draft").expect("the reset draft");
  assert!(
    matches!(&draft.body().steps()[0].kind, StepKind::Agent { profile: None, .. }),
    "the step lost its profile"
  );
  let published = db.latest_version::<Pipeline>(pipeline).expect("latest").expect("v1");
  assert!(
    matches!(&published.body().steps()[0].kind, StepKind::Agent { profile: Some(p), .. } if *p == id),
    "v1 stays as it ran"
  );
}

/// A published pipeline whose one step runs `profile`.
fn assigned_pipeline(screen: &Agents, profile: ResourceId) -> ResourceId {
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
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
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
