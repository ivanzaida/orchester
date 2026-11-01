//! `save_skill_draft` and `publish_skill` (ORC-79): the Skills editor through Orchester's MCP server. A new skill and a
//! replaced draft take every field the editor has and are refused as the editor refuses them (name, scope, reference
//! file paths and limits, expected_revision); a draft automation saved is marked until a person saves it or it is
//! published; publishing reaches agents, so it waits for a person's switch and is refused for a blank description or
//! body, a stale draft and a draft that changed. In a shared project a member has no drafts of the team's skills
//! (`not_admin`).

use std::{collections::HashMap, fs, sync::Arc};

use orchester_core::{
  id::{ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Draft, InstructionScope, Pipeline, PipelineEdit, Skill, Slug, StepKind},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
  store::{BlobStore, ResourceStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, SharingLink, StoreError};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  skills::{
    data::ProjectSkills,
    editor::{self, Refusal, WriteError},
  },
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// The app's session with one local project, Kontur, whose tools are called as the app's MCP server calls them.
struct Desktop {
  workspace: Workspace,
  services: app_mcp::Services,
  runtime: Runtime,
  repo: TempDir,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let repo = TempDir::new().expect("repository");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    ProjectDb::create(repo.path(), project).expect("project");
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(repo.path()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      repo.path().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)));
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      workspace,
      services,
      runtime,
      repo,
      _data: data,
    }
  }

  fn call(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
    let translate = Translate::new(|message| message.key.to_owned());
    let Value::Object(args) = args.clone() else {
      panic!("arguments are an object");
    };
    self
      .runtime
      .block_on(app_mcp::setup::call(&self.services, &translate, name, args))
      .expect("a setup tool")
  }

  fn ok(&self, name: &str, args: &Value) -> Value {
    self
      .call(name, args)
      .unwrap_or_else(|error| panic!("{name} refused: {}", json_of(&error)))
  }

  /// The code `name` is refused with.
  fn refused(&self, name: &str, args: &Value) -> String {
    match self.call(name, args) {
      Ok(value) => panic!("{name} did what it should refuse: {value}"),
      Err(error) => json_of(&error)["error"]["code"].as_str().expect("a code").to_owned(),
    }
  }

  fn let_automation_widen(&self) {
    self
      .workspace
      .set_automation_changes_permissions(true)
      .expect("a person turns it on");
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// The skill `name` as list_skills lists it.
  fn listed(&self, name: &str) -> Value {
    let listed = self.ok("list_skills", &json!({ "project": "Kontur" }));
    listed["skills"]
      .as_array()
      .and_then(|skills| skills.iter().find(|skill| skill["name"] == name))
      .cloned()
      .unwrap_or_else(|| panic!("{name} is listed: {listed}"))
  }

  /// A pipeline Delivery whose one step, Checks, runs a tool, so no agent is there to load a skill.
  fn tool_pipeline(&self) {
    let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
    let key = edit.add_step(Name::new("Checks").expect("name"), None);
    edit.update(&key, |step| step.kind = StepKind::Tool { tool: None });
    self
      .db()
      .create_resource(
        Slug::new("delivery").expect("slug"),
        Name::new("Delivery").expect("name"),
        &edit.validate().expect("draft"),
        Timestamp::now(),
      )
      .expect("pipeline");
  }
}

fn json_of(error: &ToolError) -> Value {
  error.to_json(|message| message.key.to_owned())
}

fn save(fields: Value) -> Value {
  let Value::Object(fields) = fields else {
    panic!("fields are an object");
  };
  let mut args = json!({ "project": "Kontur" });
  args.as_object_mut().expect("an object").extend(fields);
  args
}

fn publish(skill: &str) -> Value {
  json!({ "project": "Kontur", "skill": skill })
}

#[test]
fn skill_tools_make_a_skill_and_replace_its_draft_with_every_field() {
  let desktop = Desktop::new();
  desktop.tool_pipeline();
  let made = desktop.ok(
    "save_skill_draft",
    &save(json!({
      "name": "Release notes",
      "description": "Writing release notes.",
      "body": "# Release notes",
      "files": [{ "path": "references/format.md", "content": "One line per change." }],
    })),
  );
  let skill = &made["skill"];
  assert_eq!(skill["slug"], "release-notes", "{skill}");
  assert_eq!(skill["applies_in"]["kind"], "project", "{skill}");
  assert_eq!(skill["state"]["state"], "draft", "{skill}");
  assert_eq!(skill["draft_by_automation"], true, "{skill}");
  assert_eq!(skill["files"], json!([{ "path": "references/format.md", "size": 20 }]));
  assert!(skill["revision"].is_string(), "{skill}");

  let other = desktop.ok("save_skill_draft", &save(json!({ "name": "Release-notes" })));
  assert_eq!(
    other["skill"]["slug"], "release-notes-2",
    "the slug is unique, as the editor makes it"
  );
  assert_eq!(other["skill"]["description"], "", "a new skill starts blank");

  let replaced = desktop.ok(
    "save_skill_draft",
    &save(json!({
      "skill": "release-notes",
      "name": "Changelog",
      "body": "# Changelog",
      "applies_in": "pipeline",
      "pipeline": "delivery",
      "files": [
        { "path": "references/format.md" },
        { "path": "examples.md", "content": "- Fixed the login." },
      ],
    })),
  );
  let skill = &replaced["skill"];
  assert_eq!(skill["name"], "Changelog", "{skill}");
  assert_eq!(skill["slug"], "release-notes", "a renamed skill keeps its slug");
  assert_eq!(
    skill["description"], "Writing release notes.",
    "a field left out keeps its value"
  );
  assert_eq!(skill["body"], "# Changelog");
  assert_eq!(skill["applies_in"]["kind"], "pipeline", "{skill}");
  assert_eq!(skill["applies_in"]["pipeline"]["name"], "Delivery", "{skill}");
  let entry = ProjectSkills::load_in(desktop.repo.path())
    .expect("skills")
    .entries
    .iter()
    .find(|entry| &*entry.name == "Changelog")
    .expect("saved")
    .clone();
  let contents: Vec<(String, Vec<u8>)> = entry
    .body
    .files()
    .iter()
    .map(|file| {
      let content = desktop.db().blob(&file.content).expect("its content");
      (file.path.as_str().to_owned(), content)
    })
    .collect();
  assert_eq!(
    contents,
    vec![
      ("examples.md".to_owned(), b"- Fixed the login.".to_vec()),
      ("references/format.md".to_owned(), b"One line per change.".to_vec()),
    ],
    "a file given without content is kept"
  );
}

#[test]
fn skill_tools_refuse_what_the_skills_editor_refuses() {
  let desktop = Desktop::new();
  desktop.tool_pipeline();
  desktop.ok("save_skill_draft", &save(json!({ "name": "Checks" })));
  assert_eq!(
    desktop.refused("save_skill_draft", &save(json!({ "name": "CHECKS" }))),
    "name_taken"
  );
  assert_eq!(
    desktop.refused("save_skill_draft", &save(json!({ "name": " " }))),
    "invalid_name"
  );
  assert_eq!(
    desktop.refused("save_skill_draft", &save(json!({}))),
    "invalid_name",
    "a new skill needs a name"
  );
  for path in [
    "../outside.md",
    "/absolute.md",
    "C:/drive.md",
    "a/./b.md",
    "a/../b.md",
    "a\\b.md",
    "SKILL.md",
    "",
  ] {
    let files = json!([{ "path": path, "content": "text" }]);
    assert_eq!(
      desktop.refused("save_skill_draft", &save(json!({ "skill": "Checks", "files": files }))),
      "invalid_files",
      "{path:?} is outside the skill or not a file of it"
    );
  }
  let twice = json!([{ "path": "a.md", "content": "1" }, { "path": "a.md", "content": "2" }]);
  let large = json!([{ "path": "a.md", "content": "x".repeat(1024 * 1024 + 1) }]);
  let missing = json!([{ "path": "kept.md" }]);
  let many: Vec<Value> = (0..=editor::MAX_FILES)
    .map(|index| json!({ "path": format!("f{index}.md"), "content": "x" }))
    .collect();
  for files in [twice, large, missing, json!(many)] {
    assert_eq!(
      desktop.refused("save_skill_draft", &save(json!({ "skill": "Checks", "files": files }))),
      "invalid_files"
    );
  }
  let step = json!({ "skill": "Checks", "applies_in": "step", "pipeline": "Delivery", "step": "checks" });
  assert_eq!(
    desktop.refused("save_skill_draft", &save(step)),
    "scope_unavailable",
    "a tool step has no agent to load a skill"
  );
  let role = json!({ "skill": "Checks", "applies_in": "agent_role", "role": "Builder" });
  assert_eq!(desktop.refused("save_skill_draft", &save(role)), "unknown_role");
  let pipeline = json!({ "skill": "Checks", "applies_in": "pipeline", "pipeline": "Nope" });
  assert_eq!(
    desktop.refused("save_skill_draft", &save(pipeline)),
    "pipeline_not_found"
  );
  let checks = desktop.listed("Checks");
  assert_eq!(
    checks["applies_in"]["kind"], "project",
    "nothing refused was saved: {checks}"
  );
  assert_eq!(desktop.ok("get_skill", &publish("Checks"))["skill"]["files"], json!([]));
}

#[test]
fn skill_tools_save_over_the_expected_revision_only() {
  let desktop = Desktop::new();
  desktop.ok("save_skill_draft", &save(json!({ "name": "Checks" })));
  let read = desktop.ok("get_skill", &publish("checks"));
  let revision = read["skill"]["revision"].as_str().expect("a revision").to_owned();
  let saved = desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "First", "expected_revision": revision })),
  );
  assert_ne!(
    saved["skill"]["revision"],
    json!(revision),
    "a save makes a new revision"
  );
  assert_eq!(
    desktop.refused(
      "save_skill_draft",
      &save(json!({ "skill": "Checks", "body": "Over it", "expected_revision": revision })),
    ),
    "draft_changed"
  );
  assert_eq!(desktop.listed("Checks")["description"], "");
  assert_eq!(
    desktop.ok("get_skill", &publish("Checks"))["skill"]["body"],
    "First",
    "the save made on an older revision changed nothing"
  );
  assert_eq!(
    desktop.refused(
      "save_skill_draft",
      &save(json!({ "name": "New", "expected_revision": revision })),
    ),
    "invalid_arguments"
  );
}

#[test]
fn skill_tools_publish_only_under_the_persons_switch() {
  let desktop = Desktop::new();
  let draft = json!({ "name": "Checks", "description": "Running checks.", "body": "# Checks" });
  desktop.ok("save_skill_draft", &save(draft));
  let refused = desktop
    .call("publish_skill", &publish("Checks"))
    .expect_err("person only");
  let refused = json_of(&refused);
  assert_eq!(refused["error"]["code"], "person_only", "{refused}");
  assert_eq!(
    refused["error"]["reasons"],
    json!(["appMcp.errors.promptPersonOnly", "appMcp.prompts.skill"])
  );
  assert_eq!(
    desktop.listed("Checks")["state"]["state"],
    "draft",
    "nothing was published"
  );

  desktop.let_automation_widen();
  let published = desktop.ok("publish_skill", &publish("checks"));
  assert_eq!(published["skill"]["state"]["state"], "published", "{published}");
  assert_eq!(published["skill"]["state"]["published_version"], 1, "{published}");
  assert_eq!(published["skill"]["draft_by_automation"], false, "{published}");
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "skill": "Checks", "body": "# Checks v2" })),
  );
  let next = desktop.ok("publish_skill", &publish("Checks"));
  assert_eq!(
    next["skill"]["state"]["published_version"], 2,
    "the next version: {next}"
  );
}

#[test]
fn skill_tools_refuse_a_blank_a_stale_and_a_changed_draft() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "name": "Checks", "body": "# Checks" })),
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
    &save(json!({ "skill": "Checks", "body": "# Checks" })),
  );
  desktop.ok("publish_skill", &publish("Checks"));
  assert_eq!(
    desktop.refused("publish_skill", &publish("Checks")),
    "no_draft",
    "the latest version is current"
  );

  // A draft started before v1, saved past it: published, it would undo v1.
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

  // The draft publish_skill read changed before it was published: what was checked is not what would be published.
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "name": "Lint", "description": "Linting.", "body": "# Lint" })),
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
    desktop.listed("Lint")["state"]["state"],
    "draft",
    "nothing was published"
  );
}

#[test]
fn skill_tools_mark_automations_draft_until_a_person_saves_or_publishes_it() {
  let desktop = Desktop::new();
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "name": "Checks", "description": "Running checks.", "body": "# Checks" })),
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
  desktop.let_automation_widen();
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
  let made = editor::save_checked_in(root, (None, "Checks", &blank), Timestamp::now()).expect("a draft");
  assert_eq!(
    refusal(editor::save_checked_in(root, (None, "checks", &blank), Timestamp::now()).expect_err("taken")),
    Refusal::NameTaken
  );
  assert_eq!(
    refusal(editor::publish_checked_in(root, &made).expect_err("blank")),
    Refusal::BlankDescription
  );
  assert_eq!(Refusal::BlankBody.code(), "blank_body");
  assert_eq!(Refusal::InvalidFiles.code(), "invalid_files");
}

#[test]
fn skill_tools_and_the_skills_editor_answer_a_member_of_a_shared_project_not_admin() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  desktop.ok(
    "save_skill_draft",
    &save(json!({ "name": "Checks", "description": "Running checks.", "body": "# Checks" })),
  );
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
