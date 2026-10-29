//! The approval mode through the profile editor's actions (`docs/design/approval-modes.md`): Manual → Autopilot is
//! a widening the person confirms, Autopilot → Manual never asks, and the mode is kept with each version.
use orchester_core::{permission::ApprovalMode, resource::AgentProfile, store::ResourceStore};
use orchester_workspace::{
  agents::{
    data::ProfileEntry,
    editor::{self, Action, Outcome, Request},
  },
  messages::Message,
};

use super::*;

fn with_mode(profile: &AgentProfile, mode: ApprovalMode) -> AgentProfile {
  let mut permissions = profile.permissions().clone();
  permissions.set_approval_mode(mode);
  profile.clone().with_permissions(permissions)
}

#[test]
fn approval_mode_profile_autopilot_is_confirmed_and_manual_never_asks() {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let manual = form(&connection, &ModelSelection::Listed(model("gpt-5.5")), None)
    .profile()
    .expect("profile");
  assert_eq!(
    manual.permissions().approval_mode(),
    ApprovalMode::Manual,
    "a new profile is Manual"
  );
  let id = create_profile(&mut db, &manual, now()).expect("create");
  db.publish_draft::<AgentProfile>(id, now()).expect("v1");
  let entry = |db: &ProjectDb| -> ProfileEntry {
    ProjectAgents::load(db)
      .expect("load")
      .profile(id)
      .expect("profile")
      .clone()
  };
  let request = |existing: ProfileEntry, profile: AgentProfile, action: Action| Request {
    existing: Some(existing),
    profile,
    action,
    allowed: vec![],
    prompt_decided: true,
  };
  let autopilot = with_mode(&manual, ApprovalMode::Autopilot);
  let mut widened = request(entry(&db), autopilot.clone(), Action::Save);
  let Outcome::Confirm(reasons) = editor::apply(repo.path(), &widened).expect("offered") else {
    panic!("Autopilot asks the person")
  };
  assert_eq!(reasons, [Message::new("appMcp.widens.autopilot")]);
  assert_eq!(entry(&db).profile, manual, "nothing saved before the person confirms");
  widened.allowed = reasons.clone();
  assert_eq!(
    editor::apply(repo.path(), &widened).expect("confirmed"),
    Outcome::Saved(id)
  );
  let mut publish = request(entry(&db), autopilot, Action::Publish);
  publish.allowed = reasons;
  assert_eq!(
    editor::apply(repo.path(), &publish).expect("published"),
    Outcome::Saved(id)
  );
  let v2 = db.latest_version::<AgentProfile>(id).expect("read").expect("v2");
  assert_eq!(v2.number(), version(2));
  assert_eq!(v2.body().permissions().approval_mode(), ApprovalMode::Autopilot);
  let v1 = db.version::<AgentProfile>(id, version(1)).expect("read").expect("v1");
  assert_eq!(
    v1.body().permissions().approval_mode(),
    ApprovalMode::Manual,
    "v1 keeps its mode"
  );
  let back = request(entry(&db), with_mode(&manual, ApprovalMode::Manual), Action::Save);
  assert_eq!(
    editor::apply(repo.path(), &back).expect("narrowing"),
    Outcome::Saved(id),
    "switching to Manual never asks"
  );
  assert_eq!(entry(&db).profile.permissions().approval_mode(), ApprovalMode::Manual);
}
