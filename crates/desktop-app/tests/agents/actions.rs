use orchester_workspace::agents::data::VersionState;

#[test]
fn agents_create_first_draft_and_publish() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.click("agents-new");
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.set("agent-custom-model", "test-model");
  screen.click("agent-save");
  screen.until("created", |s| {
    s.listed().profiles.len() == 1
      && s.exists("agent-publish")
      && !s.exists("agent-save")
      && !s.exists("agents-loading")
  });
  screen.click("agent-publish");
  screen.until("first version", |s| {
    matches!(s.listed().profiles[0].state, VersionState::Published(_))
  });
}

#[test]
fn agents_editing_dismisses_confirmation_for_previous_candidate() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-tab-permissions");
  screen.click("agent-option-EditWorktree");
  screen.click("agent-save");
  screen.until("question", |s| s.exists("agent-confirm-confirm"));
  screen.click("agent-tab-profile");
  screen.set("agent-responsibility", "A new candidate");
  assert!(!screen.exists("agent-confirm"));
  assert!(!screen.window.link.input_gate().protecting());
  assert!(matches!(
    screen.listed().profile(id).expect("profile").state,
    VersionState::Published(_)
  ));
}

use super::harness::Agents;
#[test]
fn agents_save_draft_and_publish() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-responsibility", "Checks the complete change");
  assert!(screen.text("agent-save-status").contains("Unsaved changes"));
  screen.click("agent-save");
  screen.until("saved", |s| s.text("screen-agents").contains("Draft saved"));
  assert_eq!(
    screen.listed().profile(id).expect("profile").profile.responsibilities(),
    "Checks the complete change"
  );
  screen.click("agent-publish");
  screen.until("published", |s| {
    matches!(s.listed().profile(id).expect("profile").state,
    VersionState::Published(v) if v.get()==2)
  });
}
#[test]
fn agents_widening_waits_for_person_and_cancel_keeps_storage() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-tab-permissions");
  screen.click("agent-option-EditWorktree");
  screen.click("agent-save");
  screen.until("question", |s| s.exists("agent-confirm-confirm"));
  assert!(
    screen.window.link.input_gate().protecting(),
    "window input from MCP is locked"
  );
  assert_eq!(
    screen
      .listed()
      .profile(id)
      .expect("profile")
      .profile
      .permissions()
      .repository(),
    orchester_core::resource::RepositoryAccess::ReadOnly
  );
  screen.click("agent-confirm-keep");
  assert!(!screen.exists("agent-confirm-confirm"));
  assert!(!screen.window.link.input_gate().protecting());
  screen.click("agent-save");
  screen.until("question", |s| s.exists("agent-confirm-confirm"));
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("write draft", |s| {
    s.listed()
      .profile(id)
      .expect("profile")
      .profile
      .permissions()
      .repository()
      == orchester_core::resource::RepositoryAccess::Write
  });
}
#[test]
fn agents_invalid_fields_and_concurrent_changes_are_visible() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-role", "");
  screen.click("agent-save");
  assert!(screen.exists("agent-validation-error"));
  screen.set("agent-role", "Builder changed");
  let entry = screen.listed().profile(id).expect("profile").clone();
  orchester_workspace::agents::data::apply_in(
    &screen.root,
    orchester_workspace::agents::data::ProfileChange::SaveDraft(id, super::harness::changed(&entry.profile)),
  )
  .expect("outside save");
  screen.click("agent-save");
  screen.until("conflict", |s| s.exists("agent-save-error"));
  assert!(screen.text("agent-save-error").contains("draft changed"));
}
