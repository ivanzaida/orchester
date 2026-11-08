//! *Codex sandbox* through the domain tools (ORC-789): turning it off widens what agents may do, so
//! `save_profile_draft` and `publish_profile` refuse it as `person_only` until a person lets automation change agent
//! permissions; turning it on again is always taken, permissions given without it keep the profile's own, and a shared
//! project's profiles keep it on.

use std::path::Path;

use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};
use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, its Codex connection's program any file the seed records as checked.
fn served() -> (Served, std::path::PathBuf) {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (served, seeded) = Served::seeded_with("empty", Some(codex));
  (served, seeded.repo)
}

/// Profile Sandboxed's write access with any command in the worktree and network on, its sandbox `sandbox` if given.
fn save(sandbox: Option<&str>) -> Value {
  let mut permissions = json!({ "repository": "write", "commands": "any_in_worktree", "network": "on" });
  if let Some(sandbox) = sandbox {
    permissions["codex_sandbox"] = json!(sandbox);
  }
  json!({
    "project": "Kontur", "profile": "Sandboxed", "role": "Sandboxed", "connection": "Codex · Kontur",
    "model": "gpt-5.5", "permissions": permissions,
  })
}

fn sandbox_of(saved: &Value) -> &Value {
  &saved["profile"]["permissions"]["codex_sandbox"]
}

fn share(repo: &Path) {
  let now = Timestamp::now();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: now,
    cursor: 0,
    last_synced_at: now,
    removed_at: None,
    address: None,
  };
  ProjectDb::open(repo)
    .expect("the project's data")
    .start_sharing(&link, &[], (&[], &Default::default()))
    .expect("share");
}

#[test]
fn codex_sandbox_off_over_mcp_is_person_only() {
  let (mut served, repo) = served();
  let publish = json!({ "project": "Kontur", "profile": "Sandboxed" });
  served.let_automation_widen(true);
  let mut create = save(None);
  create.as_object_mut().expect("arguments").remove("profile");
  let created = served.ok("save_profile_draft", &create);
  assert_eq!(sandbox_of(&created), "on", "a new profile has it on: {created}");
  assert_eq!(
    created["profile"]["state"]["published_version"], 1,
    "created as v1: {created}"
  );
  served.let_automation_widen(false);

  let refused = served.refused("save_profile_draft", &save(Some("off")));
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert!(
    refused["reasons"].to_string().contains("Codex sandbox off"),
    "{refused}"
  );
  let kept = served.ok("get_profile", &publish);
  assert_eq!(sandbox_of(&kept), "on", "{kept}");

  served.let_automation_widen(true);
  assert_eq!(sandbox_of(&served.ok("save_profile_draft", &save(Some("off")))), "off");
  assert_eq!(
    sandbox_of(&served.ok("save_profile_draft", &save(None))),
    "off",
    "permissions without it keep the profile's own"
  );
  served.let_automation_widen(false);
  let unpublished = served.refused("publish_profile", &publish);
  assert_eq!(unpublished["code"], "person_only", "{unpublished}");
  assert_eq!(
    sandbox_of(&served.ok("save_profile_draft", &save(Some("on")))),
    "on",
    "turning it on again narrows"
  );

  share(&repo);
  served.let_automation_widen(true);
  let shared = served.refused("save_profile_draft", &save(Some("off")));
  assert_eq!(shared["code"], "not_offered", "{shared}");
}
