//! Creating a skill as v1 and publishing a draft over MCP: the person's switch, blank, stale and changed drafts, the
//! mark automation leaves, the editor's own refusals, and a member of a shared project.

use std::collections::HashMap;

use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  resource::{Draft, InstructionScope, ResourceKind, Skill},
  sharing::{MemberRole, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{SharingLink, StoreError};
use orchester_workspace::skills::{
  data::ProjectSkills,
  editor::{self, Refusal, WriteError},
};
use serde_json::json;

use super::{Desktop, json_of, make, publish, save};

#[test]
fn skill_tools_create_and_publish_only_under_the_persons_switch() {
  let desktop = Desktop::new();
  let new = save(json!({ "name": "Checks", "description": "Running checks.", "body": "# Checks" }));
  let refused = desktop.call("save_skill_draft", &new).expect_err("person only");
  let refused = json_of(&refused);
  assert_eq!(refused["error"]["code"], "person_only", "{refused}");
  assert_eq!(
    refused["error"]["reasons"],
    json!(["appMcp.errors.promptPersonOnly", "appMcp.prompts.skill"])
  );
  assert!(
    desktop.ok("list_skills", &json!({ "project": "Kontur" }))["skills"]
      .as_array()
      .expect("skills")
      .is_empty(),
    "nothing was created"
  );

  desktop.let_automation_widen();
  let made = desktop.ok("save_skill_draft", &new);
  assert_eq!(made["skill"]["state"]["state"], "published", "{made}");
  assert_eq!(made["skill"]["state"]["published_version"], 1, "{made}");
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Checks v2" })),
  );
  desktop.stop_automation_widening();
  let refused = desktop
    .call("publish_skill", &publish("Checks"))
    .expect_err("person only");
  assert_eq!(json_of(&refused)["error"]["code"], "person_only");
  assert_eq!(
    desktop.listed("Checks")["state"]["published_version"],
    1,
    "nothing was published"
  );

  desktop.let_automation_widen();
  let published = desktop.ok("publish_skill", &publish("checks"));
  assert_eq!(published["skill"]["state"]["state"], "published", "{published}");
  assert_eq!(published["skill"]["state"]["published_version"], 2, "{published}");
  assert_eq!(published["skill"]["draft_by_automation"], false, "{published}");
}

#[test]
fn skill_tools_refuse_a_blank_a_stale_and_a_changed_draft() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  assert_eq!(
    desktop.refused(
      "save_skill_draft",
      &save(json!({ "name": "Checks", "body": "# Checks" }))
    ),
    "blank_description",
    "a new skill is v1, so it can be published"
  );
  assert_eq!(
    desktop.refused(
      "save_skill_draft",
      &save(json!({ "name": "Checks", "description": "Running checks.", "body": "  " })),
    ),
    "blank_body"
  );
  desktop.ok("save_skill_draft", &make("Checks"));
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "description": " " })),
  );
  assert_eq!(
    desktop.refused("publish_skill", &publish("Checks")),
    "blank_description"
  );
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "description": "Running checks.", "body": "  " })),
  );
  assert_eq!(desktop.refused("publish_skill", &publish("Checks")), "blank_body");
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Checks v2" })),
  );
  desktop.ok("publish_skill", &publish("Checks"));
  assert_eq!(
    desktop.refused("publish_skill", &publish("Checks")),
    "no_draft",
    "the latest version is current"
  );

  // A draft started before v2, saved past it: published, it would undo v2.
  let id = ProjectSkills::load_in(desktop.repo.path()).expect("skills").entries[0].id;
  let body = Skill::new(
    "Running checks.".into(),
    "# Older".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a body");
  desktop
    .db()
    .save_draft(id, &Draft::new(body, None, Timestamp::now()))
    .expect("an old draft");
  assert_eq!(desktop.refused("publish_skill", &publish("Checks")), "stale_draft");
}

#[test]
fn skill_tools_refuse_publishing_a_draft_that_changed_since_it_was_read() {
  // The draft publish_skill read changed before it was published: what was checked is not what would be published.
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "name": "Lint", "description": "Linting.", "body": "# Lint" })),
  );
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Lint", "body": "# Lint v2" })),
  );
  let read = ProjectSkills::load_in(desktop.repo.path())
    .expect("skills")
    .entries
    .iter()
    .find(|entry| &*entry.name == "Lint")
    .expect("saved")
    .clone();
  let changed = Skill::new(
    "Linting.".into(),
    "# Lint, changed".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a body");
  editor::save_in(desktop.repo.path(), Some(&read), "Lint", &changed).expect("a person's save meanwhile");
  match editor::publish_checked_in(desktop.repo.path(), &read) {
    Err(WriteError::Store(StoreError::DraftChanged(resource))) => assert_eq!(resource, read.id),
    other => panic!("a changed draft is refused as draft_changed: {other:?}"),
  }
  assert_eq!(
    desktop.listed("Lint")["state"]["published_version"],
    1,
    "nothing was published"
  );
}

#[test]
fn skill_tools_mark_automations_draft_until_a_person_saves_or_publishes_it() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  desktop.ok("save_skill_draft", &make("Checks"));
  assert_eq!(
    desktop.listed("Checks")["draft_by_automation"],
    false,
    "a new skill is v1 and has no draft"
  );
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Checks v2" })),
  );
  assert_eq!(desktop.listed("Checks")["draft_by_automation"], true);
  let entry = ProjectSkills::load_in(desktop.repo.path()).expect("skills").entries[0].clone();
  let body = Skill::new(
    "Running checks.".into(),
    "# A person's checks".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a body");
  editor::save_in(desktop.repo.path(), Some(&entry), "Checks", &body).expect("a person saves it");
  assert_eq!(
    desktop.listed("Checks")["draft_by_automation"],
    false,
    "the person's draft"
  );
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Again" })),
  );
  assert_eq!(desktop.listed("Checks")["draft_by_automation"], true);
  desktop.ok("publish_skill", &publish("Checks"));
  assert_eq!(
    desktop.listed("Checks")["draft_by_automation"],
    false,
    "published, it has no draft"
  );
}

#[test]
fn skill_tools_name_the_editors_rule_that_refused_a_write() {
  let desktop = Desktop::new();
  let root = desktop.repo.path();
  let blank = Skill::new(String::new(), String::new(), Vec::new(), InstructionScope::Project).expect("a body");
  let refusal = |error: WriteError| match error {
    WriteError::Refused(refusal, _) => refusal,
    other => panic!("refused by a rule: {other:?}"),
  };
  assert_eq!(
    refusal(editor::save_checked_in(root, (None, "Checks", &blank), Timestamp::now()).expect_err("v1 is blank")),
    Refusal::BlankDescription
  );
  let filled = Skill::new(
    "Running checks.".into(),
    "# Checks".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a body");
  let made = editor::save_checked_in(root, (None, "Checks", &filled), Timestamp::now()).expect("v1");
  assert_eq!(
    refusal(editor::save_checked_in(root, (None, "checks", &filled), Timestamp::now()).expect_err("taken")),
    Refusal::NameTaken
  );
  let draft = editor::save_checked_in(root, (Some(&made), "Checks", &blank), Timestamp::now()).expect("a draft");
  assert_eq!(
    refusal(editor::publish_checked_in(root, &draft).expect_err("blank")),
    Refusal::BlankDescription
  );
  assert_eq!(Refusal::BlankBody.code(), "blank_body");
  assert_eq!(Refusal::InvalidFiles.code(), "invalid_files");
}

#[test]
fn skill_tools_and_the_skills_editor_answer_a_member_of_a_shared_project_not_admin() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  desktop.ok("save_skill_draft", &make("Checks"));
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Checks v2" })),
  );
  let team = desktop
    .db()
    .kept_resources(ResourceKind::Skill)
    .expect("resources")
    .remove(0);
  let entry = ProjectSkills::load_in(desktop.repo.path()).expect("skills").entries[0].clone();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Member),
    joined_at: Timestamp::from_unix_millis(1),
    cursor: 0,
    last_synced_at: Timestamp::from_unix_millis(1),
    removed_at: None,
    address: Some("https://team.example/kontur".to_owned()),
  };
  desktop
    .db()
    .start_sharing(&link, &[], (&[], &HashMap::new()))
    .expect("joined as a member");
  desktop
    .db()
    .keep_shared_resource(&team, TaskRevision::FIRST)
    .expect("the skill is the team's");

  for args in [
    save(json!({ "skill": "Checks", "body": "# Mine" })),
    save(json!({ "name": "Lint", "description": "Linting.", "body": "# Lint" })),
  ] {
    assert_eq!(desktop.refused("save_skill_draft", &args), "not_admin", "{args}");
  }
  assert_eq!(desktop.refused("publish_skill", &publish("Checks")), "not_admin");
  let body = Skill::new(
    "Running checks.".into(),
    "# Mine".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a body");
  match editor::save_checked_in(desktop.repo.path(), (Some(&entry), "Checks", &body), Timestamp::now()) {
    Err(WriteError::Store(StoreError::NotAdmin)) => {}
    other => panic!("the editor's save answers not_admin: {other:?}"),
  }
  assert_eq!(desktop.listed("Checks")["state"]["state"], "draft", "nothing changed");
}
