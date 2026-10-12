//! A client's window-input rights never authorize publishing arbitrary skills or repository files.
use orchester_core::{
  agent_files::RepositoryAgentFiles,
  resource::{InstructionScope, Skill},
  store::{ResourceStore, SettingsStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::skills::editor;
use serde_json::json;

use super::connections_window::controlled;
#[test]
fn skills_window_publish_is_person_only_even_with_permission_automation_opted_in() {
  let (mut s, project, repo) = controlled();
  s.launch
    .as_ref()
    .expect("launch")
    .workspace()
    .expect("workspace")
    .set_automation_changes_permissions(true)
    .expect("opt in");
  let body = Skill::new(
    "Check code".into(),
    "# Check code".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("body");
  let entry = editor::save_in(&repo, None, "checks", &body).expect("draft");
  s.ok(
    "navigate",
    &json!({"route":"skills","project":project.to_string(),"skill":entry.id.to_string()}),
  );
  s.until("editor", |s| s.shows("skill-publish"));
  s.click_by_client("skill-publish").expect("may ask");
  s.until("question", |s| s.shows("skill-confirm-confirm"));
  assert_eq!(
    s.click_by_client("skill-confirm-confirm").expect_err("person only")["code"],
    "person_only"
  );
  let db = ProjectDb::open(&repo).expect("db");
  assert!(db.latest_version::<Skill>(entry.id).expect("version").is_none());
  s.click_by_person("skill-confirm-confirm");
  s.until("published", |_| {
    db.latest_version::<Skill>(entry.id).expect("version").is_some()
  });
}
#[test]
fn skills_window_repository_files_confirmation_locks_window_input() {
  let (mut s, project, repo) = controlled();
  let mut db = ProjectDb::open(&repo).expect("db");
  db.save_repository_agent_files(RepositoryAgentFiles::Off).expect("off");
  s.ok("navigate", &json!({"route":"skills","project":project.to_string()}));
  s.until("empty skills", |s| s.shows("skill-repository-files"));
  s.click_by_client("skill-repository-files").expect("may ask");
  s.until("question", |s| s.shows("skill-files-confirm-confirm"));
  assert_eq!(
    s.click_by_client("skill-files-confirm-confirm")
      .expect_err("person only")["code"],
    "person_only"
  );
  assert!(!db.repository_agent_files().expect("setting").is_on());
  s.click_by_person("skill-files-confirm-confirm");
  s.until("enabled", |_| db.repository_agent_files().expect("setting").is_on());
}
