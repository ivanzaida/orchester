//! Skills screen rules are independent of rendering.
use std::fs;

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{InstructionScope, Skill},
  store::SettingsStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::skills::editor::{self, EditorData};
fn project() -> (tempfile::TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("project");
  fs::create_dir(repo.path().join(".git")).expect("git");
  let db = ProjectDb::create(
    repo.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      Name::new("Skills").expect("name"),
    ),
  )
  .expect("db");
  (repo, db)
}
fn skill(description: &str, body: &str) -> Skill {
  Skill::new(description.into(), body.into(), Vec::new(), InstructionScope::Project).expect("skill")
}
#[test]
fn skills_name_validation_and_competing_edits_preserve_the_stored_draft() {
  let (repo, _db) = project();
  let before = editor::save_in(repo.path(), None, "checks", &skill("Check", "Check first")).expect("created");
  let name_taken = editor::save_in(repo.path(), None, "CHECKS", &skill("Other", "Other")).expect_err("name taken");
  assert_eq!(name_taken.key, "skills.editorErrors.nameTaken");
  let newer =
    editor::save_in(repo.path(), Some(&before), "checks", &skill("Check", "Changed outside")).expect("other edit");
  let stale = editor::save_in(repo.path(), Some(&before), "renamed", &skill("New", "Overwrite")).expect_err("stale");
  assert_eq!(stale.key, "errors.draftChanged");
  let current = EditorData::load_in(repo.path())
    .expect("read")
    .listed
    .entry(before.id)
    .expect("entry")
    .clone();
  assert_eq!(current, newer);
  let renamed = editor::save_in(repo.path(), Some(&newer), "renamed", &newer.body).expect("rename");
  assert_eq!(renamed.name.as_ref(), "renamed");
  assert_eq!(renamed.slug, before.slug);
}
#[test]
fn skills_publish_requires_complete_reviewed_saved_content() {
  let (repo, _db) = project();
  let blank = editor::save_in(repo.path(), None, "checks", &skill("", "")).expect("draft");
  assert_eq!(
    editor::publish_reason(&blank).expect("not publishable").key,
    "skills.editorErrors.descriptionRequired"
  );
  assert_eq!(
    editor::publish_in(repo.path(), &blank)
      .expect_err("invalid content")
      .key,
    "skills.editorErrors.descriptionRequired"
  );
  let draft = editor::save_in(repo.path(), Some(&blank), "checks", &skill("Check", "Check first")).expect("saved");
  let outside = editor::save_in(
    repo.path(),
    Some(&draft),
    "checks",
    &skill("Check", "Changed since review"),
  )
  .expect("outside");
  assert_eq!(
    editor::publish_in(repo.path(), &draft).expect_err("stale").key,
    "errors.draftChanged"
  );
  let published = editor::publish_in(repo.path(), &outside).expect("published");
  assert!(!published.state.has_draft());
  assert_eq!(published.state.published().expect("version").get(), 1);
}
#[test]
fn skills_repository_files_match_ordinary_project_settings_semantics() {
  let (repo, db) = project();
  editor::set_agent_files_in(repo.path(), false).expect("ordinary off");
  assert_eq!(db.repository_agent_files().expect("setting"), RepositoryAgentFiles::Off);
  orchester_workspace::project_settings::save_agent_files_in(repo.path(), true).expect("accepted setting API");
  assert_eq!(db.repository_agent_files().expect("setting"), RepositoryAgentFiles::On);
  editor::set_agent_files_in(repo.path(), false).expect("ordinary off");
  editor::set_agent_files_in(repo.path(), true).expect("ordinary on");
  assert_eq!(db.repository_agent_files().expect("setting"), RepositoryAgentFiles::On);
}
#[test]
fn skills_reference_files_are_stored_and_duplicate_paths_are_refused() {
  use orchester_core::store::BlobStore;
  let (repo, db) = project();
  let file = repo.path().join("checks.md");
  fs::write(&file, "Read the check output.").expect("reference");
  let added = editor::add_file_in(repo.path(), &file, &[]).expect("add file");
  assert_eq!(db.blob(&added.content).expect("blob"), b"Read the check output.");
  assert_eq!(
    editor::add_file_in(repo.path(), &file, &[added])
      .expect_err("duplicate")
      .key,
    "skills.editorErrors.duplicateFile"
  );
}
#[test]
fn skills_tool_step_scopes_are_visible_but_cannot_be_selected() {
  use orchester_core::{
    resource::{Pipeline, PipelineEdit, Slug, StepKind},
    store::ResourceStore,
    time::Timestamp,
  };
  let (repo, mut db) = project();
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let key = edit.add_step(Name::new("Checks").expect("name"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: None });
  let pipeline = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &edit.validate().expect("draft"),
      Timestamp::now(),
    )
    .expect("pipeline");
  let data = EditorData::load_in(repo.path()).expect("data");
  let scope = InstructionScope::Step {
    pipeline: pipeline.id(),
    step: key,
  };
  let option = data.scopes.iter().find(|o| o.scope == scope).expect("shown");
  assert!(!option.available);
  assert_eq!(
    data.validate_scope(&scope).expect_err("no agent").key,
    "skills.editorErrors.scopeUnavailable"
  );
}

#[test]
fn skills_unsaved_fields_are_detected_before_name_validation() {
  use editor::SkillForm;
  let (repo, _db) = project();
  let data = EditorData::load_in(repo.path()).expect("data");
  let path = repo.path().join("notes.md");
  fs::write(&path, "Reference text").expect("file");
  let added = editor::add_file_in(repo.path(), &path, &[]).expect("reference");
  let mut form = SkillForm {
    name: "",
    description: "",
    body: "",
    files: Vec::new(),
    scope: InstructionScope::Project,
  };
  assert!(!form.has_edits(None));
  form.files.push(added);
  assert!(form.has_edits(None), "reference-only changes must be kept");
  assert_eq!(
    form.skill(&data, None).expect_err("blank name").key,
    "skills.editorErrors.nameRequired"
  );
  let mut form = SkillForm {
    name: "",
    description: "",
    body: "",
    files: Vec::new(),
    scope: InstructionScope::Pipeline {
      pipeline: orchester_core::id::ResourceId::generate(),
    },
  };
  assert!(form.has_edits(None), "scope-only changes must be kept");
  form.scope = InstructionScope::Project;
  assert!(!form.has_edits(None), "reverting every field clears the leave guard");
  let before = editor::save_in(repo.path(), None, "checks", &skill("Check", "Run checks")).expect("draft");
  assert!(
    form.has_edits(Some(&before)),
    "clearing an existing form remains an edit"
  );
  form.name = &before.name;
  form.description = before.body.description();
  form.body = before.body.body();
  assert!(!form.has_edits(Some(&before)));
}

#[test]
fn skills_publication_rejects_reviewed_snapshot_after_same_content_version_changed() {
  use orchester_core::store::ResourceStore;
  use orchester_workspace::skills::data;
  let (repo, db) = project();
  let body = skill("Check", "Check first");
  let first = editor::save_in(repo.path(), None, "checks", &body).expect("draft");
  let first = editor::publish_in(repo.path(), &first).expect("ordinary v1");
  let reviewed_body = skill("Reviewed", "Reviewed saved content");
  let reviewed = editor::save_in(repo.path(), Some(&first), "checks", &reviewed_body).expect("reviewed saved draft");
  let outside_body = skill("Outside", "A different published version");
  editor::save_in(repo.path(), Some(&reviewed), "checks", &outside_body).expect("outside saved draft");
  assert_eq!(
    data::publish_in(repo.path(), reviewed.id)
      .expect("accepted direct v2")
      .get(),
    2
  );
  let outside = data::ProjectSkills::load_in(repo.path())
    .expect("skills")
    .entry(reviewed.id)
    .expect("entry")
    .clone();
  let fresh = editor::save_in(repo.path(), Some(&outside), "checks", &reviewed_body).expect("fresh v3 draft");
  assert_eq!(
    editor::publish_in(repo.path(), &reviewed)
      .expect_err("old publication snapshot")
      .key,
    "errors.draftChanged"
  );
  assert_eq!(
    db.latest_version::<Skill>(reviewed.id)
      .expect("version")
      .expect("published")
      .number()
      .get(),
    2
  );
  assert_eq!(
    db.draft::<Skill>(reviewed.id).expect("draft").expect("retained").body(),
    &reviewed_body
  );
  let final_entry = editor::publish_in(repo.path(), &fresh).expect("fresh reviewed snapshot");
  assert_eq!(final_entry.state.published().expect("published").get(), 3);
  assert!(!final_entry.state.has_draft());
}
