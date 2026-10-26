//! A profile's page saves its draft on its own; a widening change is asked at its field before it is saved, and
//! Publish asks again only for what was not allowed yet; Discard draft and Remove are asked in place.
use orchester_core::resource::RepositoryAccess;
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
  screen.press_until("agent-publish", "published", |s| {
    matches!(s.listed().profile(id).expect("profile").state, VersionState::Published(v) if v.get() == 2)
  });
}

#[test]
fn agents_widening_is_asked_at_its_field_and_waits_for_the_person() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-option-EditWorktree");
  screen.until("question at the field", |s| s.exists("agent-confirm-confirm"));
  assert!(screen.window.link.input_gate().protecting(), "window input from MCP is locked");
  let repository = |s: &Agents| s.listed().profile(id).expect("profile").profile.permissions().repository();
  assert_eq!(repository(&screen), RepositoryAccess::ReadOnly, "nothing saved before the answer");
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
    assert!(!s.exists("agent-confirm-confirm"), "Publish asks nothing the field allowed");
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
  screen.until("draft saved", |s| s.listed().profile(id).expect("profile").state.has_draft());
  screen.saved();
  screen.press_until("agent-discard", "question", |s| s.exists("hub-discard-confirm"));
  screen.click("hub-discard-keep");
  assert!(screen.listed().profile(id).expect("profile").state.has_draft(), "Keep draft keeps it");
  screen.press_until("agent-discard", "question", |s| s.exists("hub-discard-confirm"));
  screen.press_until("hub-discard-confirm", "discarded", |s| {
    matches!(s.listed().profile(id).expect("profile").state, VersionState::Published(v) if v.get() == 1)
  });
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
  screen.click("agent-remove");
  screen.until("question", |s| s.exists("hub-remove-confirm"));
  assert!(screen.text("hub-remove").contains("No pipeline step uses it"), "{}", screen.text("hub-remove"));
  screen.press_until("hub-remove-confirm", "removed", |s| {
    s.listed().profile(id).is_none() && s.exists("agents-new")
  });
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
  screen.until("conflict", |s| s.exists("agent-save-error") || s.exists("agent-outside-change"));
}
