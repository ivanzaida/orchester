//! Ordinary saved project data uses authorized window input and the accepted domain APIs; review stays explicit.
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
fn skills_window_publication_matches_ordinary_saved_domain_data_without_permission_consent() {
  let (mut s, project, repo) = controlled();
  assert!(
    !s.launch
      .as_ref()
      .expect("launch")
      .workspace()
      .expect("workspace")
      .automation_changes_permissions()
      .expect("default-off consent")
  );
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
  s.click_by_client("skill-confirm-keep").expect("ordinary Keep");
  s.until("review canceled", |s| !s.shows("skill-confirm-confirm"));
  let db = ProjectDb::open(&repo).expect("db");
  assert!(db.latest_version::<Skill>(entry.id).expect("version").is_none());
  s.click_by_client("skill-publish").expect("renew review");
  s.until("review renewed", |s| s.shows("skill-confirm-confirm"));
  s.click_by_client("skill-confirm-confirm")
    .expect("ordinary reviewed publication");
  s.until("published", |_| {
    db.latest_version::<Skill>(entry.id).expect("version").is_some()
  });
  let native = db
    .latest_version::<Skill>(entry.id)
    .expect("version")
    .expect("published");
  assert_eq!(native.body(), &body);
  let domain = editor::save_in(&repo, None, "domain-checks", &body).expect("domain draft");
  orchester_workspace::skills::data::publish_in(&repo, domain.id).expect("accepted direct publication");
  let direct = db
    .latest_version::<Skill>(domain.id)
    .expect("version")
    .expect("published");
  assert_eq!(native.number(), direct.number());
  assert_eq!(native.body(), direct.body());
}
#[test]
fn skills_window_repository_files_match_ordinary_authenticated_domain_setting() {
  let (mut s, project, repo) = controlled();
  let mut db = ProjectDb::open(&repo).expect("db");
  db.save_repository_agent_files(RepositoryAgentFiles::Off).expect("off");
  s.ok("navigate", &json!({"route":"skills","project":project.to_string()}));
  s.until("empty skills", |s| s.shows("skill-repository-files"));
  s.click_by_client("skill-repository-files").expect("may ask");
  s.until("question", |s| s.shows("skill-files-confirm-confirm"));
  assert!(!db.repository_agent_files().expect("setting").is_on());
  s.click_by_client("skill-files-confirm-keep").expect("ordinary discard");
  s.until("discarded", |s| !s.shows("skill-files-confirm-confirm"));
  assert!(!db.repository_agent_files().expect("setting").is_on());
  s.click_by_client("skill-repository-files").expect("renew review");
  s.until("question again", |s| s.shows("skill-files-confirm-confirm"));
  s.click_by_client("skill-files-confirm-confirm")
    .expect("ordinary setting review");
  s.until("enabled", |_| db.repository_agent_files().expect("setting").is_on());
  let off = s.ok(
    "set_repository_agent_files",
    &json!({"project":project.to_string(),"enabled":false}),
  );
  assert_eq!(off["repository_agent_files"], false);
  let on = s.ok(
    "set_repository_agent_files",
    &json!({"project":project.to_string(),"enabled":true}),
  );
  assert_eq!(on["repository_agent_files"], true);
  assert!(db.repository_agent_files().expect("setting").is_on());
}

#[test]
fn skills_window_publish_retry_reviews_saved_snapshot_and_retains_actual_publish_operation() {
  let (mut s, project, repo) = controlled();
  assert!(
    !s.launch
      .as_ref()
      .expect("launch")
      .workspace()
      .expect("workspace")
      .automation_changes_permissions()
      .expect("default-off consent")
  );
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
  let changed = Skill::new(
    "Changed outside".into(),
    "# Changed outside".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("body");
  let outside = editor::save_in(&repo, Some(&entry), "checks", &changed).expect("competing save");
  s.click_by_client("skill-confirm-confirm")
    .expect("ordinary reviewed confirmation");
  s.until("publication failed", |s| s.shows("skill-save-error"));
  let db = ProjectDb::open(&repo).expect("db");
  assert!(db.latest_version::<Skill>(entry.id).expect("version").is_none());
  assert_eq!(
    db.draft::<Skill>(entry.id).expect("draft").expect("kept").body(),
    &changed
  );
  editor::save_in(&repo, Some(&outside), "checks", &body).expect("resolve failure");
  s.click_by_client("skill-retry").expect("retry may ask");
  s.until("retry confirmation", |s| s.shows("skill-confirm-confirm"));
  assert!(db.latest_version::<Skill>(entry.id).expect("version").is_none());
  s.click_by_client("skill-confirm-keep").expect("keep reviewed draft");
  s.until("keep draft", |s| !s.shows("skill-confirm-confirm"));
  assert!(db.latest_version::<Skill>(entry.id).expect("version").is_none());
  s.click_by_client("skill-retry").expect("retry may ask again");
  s.until("another retry confirmation", |s| s.shows("skill-confirm-confirm"));
  s.click_by_person("skill-confirm-confirm");
  s.until("retry publishes a version", |_| {
    db.latest_version::<Skill>(entry.id).expect("version").is_some()
  });
  s.until("failure cleared", |s| !s.shows("skill-save-error"));
  let version = db
    .latest_version::<Skill>(entry.id)
    .expect("version")
    .expect("published");
  assert_eq!(version.number().get(), 1);
  assert_eq!(version.body(), &body);
  assert!(db.draft::<Skill>(entry.id).expect("draft").is_none());
}
