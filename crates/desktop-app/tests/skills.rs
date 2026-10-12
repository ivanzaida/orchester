//! Skills in the real headless app, with isolated project data and no agent process.
#[path = "skills/recovery.rs"]
mod recovery;
mod support;
#[path = "skills/visual.rs"]
mod visual;
use std::{fs, path::PathBuf};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  agent_files::RepositoryAgentFiles,
  id::ResourceId,
  resource::{InstructionScope, Skill},
  store::{ResourceStore, SettingsStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::skills::{data::ProjectSkills, editor};
use support::{Window, project_id};
struct Skills {
  window: Window,
  repo: PathBuf,
}
impl Skills {
  fn new() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self { window, repo }
  }
  fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(project_id(), page));
    self.window.pass();
    self.window.until("Skills page", |w| {
      w.tree.get_element_by_id("skills-empty").is_some()
        || w.tree.get_element_by_id("skills-list").is_some()
        || w.tree.get_element_by_id("skill-editor").is_some()
    });
  }
  fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} shown"))
      .click();
    self.window.pass();
  }
  fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .set_value(value);
    self.window.pass();
  }
  fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} shown")).join(" ")
  }
  fn seed(&self, name: &str, published: bool) -> ResourceId {
    let body = Skill::new(
      "Run checks before handing over.".into(),
      "# Run checks\nRead the output.".into(),
      Vec::new(),
      InstructionScope::Project,
    )
    .expect("skill");
    let e = editor::save_in(&self.repo, None, name, &body).expect("create");
    if published {
      editor::publish_in(&self.repo, &e).expect("publish");
    }
    e.id
  }
  fn entry(&self, id: ResourceId) -> orchester_workspace::skills::data::SkillEntry {
    ProjectSkills::load_in(&self.repo)
      .expect("read")
      .entry(id)
      .expect("entry")
      .clone()
  }
  fn settle(&mut self, id: ResourceId, description: &str) {
    let root = self.repo.clone();
    let description = description.to_owned();
    self.window.until("draft saved", move |_| {
      ProjectSkills::load_in(&root)
        .expect("read")
        .entry(id)
        .is_some_and(|e| e.body.description() == description)
    });
    self.window.until("editor acknowledges save", |w| {
      w.texts("skill-save-status")
        .is_some_and(|t| t.join(" ").contains("Draft"))
    });
  }
}
#[test]
fn skills_empty_states_list_repository_sources_and_import_preview_and_result() {
  let mut s = Skills::new();
  let folder = s.repo.join(".claude/skills/checks");
  fs::create_dir_all(&folder).expect("folder");
  fs::write(
    folder.join("SKILL.md"),
    "---\nname: checks\ndescription: Check the project\n---\n# Checks\nRun cargo check.",
  )
  .expect("skill");
  fs::write(folder.join("notes.md"), "Reference content").expect("reference");
  fs::write(s.repo.join("AGENTS.md"), "Read the project rules.").expect("instructions");
  fs::write(s.repo.join("CLAUDE.md"), "@AGENTS.md\n").expect("pointer");
  s.go(ProjectPage::Skills);
  let empty = s.text("skills-empty");
  assert!(empty.contains(".claude/skills/checks") && empty.contains("AGENTS.md"));
  s.click("skills-import-open");
  assert!(s.text("skills-import-review").contains("nothing to import"));
  s.click("skill-import-preview-2");
  assert!(s.text("skill-import-content").contains("Run cargo check"));
  assert!(s.text("skill-import-preview").contains("notes.md"));
  assert!(s.text("skill-import-preview").contains("Check the project"));
  let preview = s
    .window
    .tree
    .get_element_by_id_mut("skill-import-preview")
    .and_then(|e| e.bounds())
    .expect("preview bounds");
  assert!(
    (preview.width - 732.0).abs() < 1.0 && preview.height > 400.0,
    "preview stays beside repository items: {preview:?}"
  );
  s.click("skill-import-create");
  s.window.until("import result", |w| {
    w.tree.get_element_by_id("skills-import-result").is_some()
  });
  assert!(s.text("skills-import-result").contains("2 drafts created"));
  let entries = ProjectSkills::load_in(&s.repo).expect("read");
  assert_eq!(entries.entries.len(), 1);
  assert_eq!(entries.entries[0].body.files().len(), 1);
  assert!(entries.entries[0].state.published().is_none());
  assert_eq!(
    fs::read_to_string(s.repo.join("AGENTS.md")).expect("file"),
    "Read the project rules."
  );
  s.click("skills-back");
  s.window.until("library after import", |w| {
    w.tree.get_element_by_id("skills-list").is_some()
  });
  assert!(s.text("skills-list").contains("Draft v1"));
  s.click(&format!("skill-row-{}", entries.entries[0].id));
  assert!(s.text("skill-inspector").contains("notes.md"));
  assert!(s.text("skill-origin-source").contains(".claude/skills/checks"));
}
#[test]
fn skills_new_edit_autosave_and_publish_keep_versions_separate() {
  let mut s = Skills::new();
  s.go(ProjectPage::Skills);
  assert!(s.text("skills-empty").contains("No skills in Kontur yet"));
  s.click("skills-new");
  s.set("skill-name", "run-checks");
  s.click("skill-create");
  s.window.until("created skill editor", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(_)))
  });
  let Route::Project(_, ProjectPage::Skill(id)) = s.window.navigation().route_now() else {
    panic!("editor route")
  };
  s.window.until("editor ready", |w| {
    w.tree.get_element_by_id("skill-description").is_some()
  });
  s.set("skill-description", "Run checks after changing code.");
  s.set("skill-body", "# Checks\nRun the checks, then read their output.");
  s.settle(id, "Run checks after changing code.");
  let root = s.repo.clone();
  s.window.until("body saved", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.body.body().contains("read their output"))
  });
  s.window.until("editor save settled", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ").contains("Draft"))
  });
  s.click("skill-publish");
  s.window.until("person-only question armed", |w| {
    w.tree.tick_futures();
    w.tree.get_element_by_id("skill-confirm-confirm").is_some()
  });
  s.click("skill-confirm-confirm");
  let root = s.repo.clone();
  s.window.until("published v1", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.state.published().is_some())
  });
  s.window.until("published editor", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ").contains("Published v1"))
  });
  let original = s.entry(id).body;
  s.set("skill-description", "New instructions for later runs.");
  s.settle(id, "New instructions for later runs.");
  assert_eq!(
    ProjectDb::open(&s.repo)
      .expect("db")
      .latest_version::<Skill>(id)
      .expect("version")
      .expect("published")
      .body(),
    &original
  );
  assert!(s.entry(id).state.has_draft());
}
#[test]
fn skills_duplicate_name_is_not_saved_and_unsaved_departure_asks_in_place() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  s.seed("migration", false);
  s.go(ProjectPage::Skill(id));
  s.set("skill-name", "migration");
  assert!(s.text("skill-name-note").contains("already a skill"));
  assert_eq!(s.entry(id).name.as_ref(), "checks");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  s.click("skill-leave-keep");
  assert!(s.window.tree.get_element_by_id("skill-editor").is_some());
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  s.click("skill-leave-confirm");
  s.window
    .until("library", |w| w.tree.get_element_by_id("skills-list").is_some());
}
#[test]
fn skills_repository_files_enabling_asks_and_off_applies_without_publishing() {
  let mut s = Skills::new();
  let mut db = ProjectDb::open(&s.repo).expect("db");
  db.save_repository_agent_files(RepositoryAgentFiles::Off).expect("off");
  s.go(ProjectPage::Skills);
  s.click("skill-repository-files");
  s.window.until("question armed", |w| {
    w.tree.tick_futures();
    w.tree.get_element_by_id("skill-files-confirm-confirm").is_some()
  });
  assert!(!db.repository_agent_files().expect("setting").is_on());
  s.click("skill-files-confirm-confirm");
  s.window
    .until("setting on", |_| db.repository_agent_files().expect("setting").is_on());
  s.window.until("on label", |w| {
    w.texts("skill-repository-files")
      .is_some_and(|t| t.join(" ").contains("On"))
  });
  s.click("skill-repository-files");
  s.window.until("setting off", |_| {
    !db.repository_agent_files().expect("setting").is_on()
  });
}
#[test]
fn skills_removing_a_reference_changes_only_the_draft() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  let before = s.entry(id);
  let path = s.repo.join("notes.md");
  fs::write(&path, "Reference text").expect("file");
  let file = editor::add_file_in(&s.repo, &path, &[]).expect("add reference");
  let body = Skill::new(
    before.body.description().into(),
    before.body.body().into(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("body");
  let saved = editor::save_in(&s.repo, Some(&before), "checks", &body).expect("save");
  s.go(ProjectPage::Skill(id));
  s.click("skill-publish");
  s.window.until("publish confirmation", |w| {
    w.tree.get_element_by_id("skill-confirm-confirm").is_some()
  });
  s.click("skill-confirm-confirm");
  s.click("skill-remove-file-0");
  assert!(
    s.text("skill-files").contains("notes.md"),
    "removal stays locked during the write"
  );
  let root = s.repo.clone();
  s.window.until("reference published", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.state.published().is_some())
  });
  s.window.until("publication acknowledged", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ").contains("Published"))
  });
  assert_eq!(s.entry(id).body, saved.body);
  s.click("skill-remove-file-0");
  let root = s.repo.clone();
  s.window.until("reference removal saved", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.body.files().is_empty())
  });
  assert_eq!(
    ProjectDb::open(&s.repo)
      .expect("db")
      .latest_version::<Skill>(id)
      .expect("version")
      .expect("published")
      .body()
      .files()
      .len(),
    1
  );
}
#[test]
fn skills_competing_edit_can_be_reloaded_after_save_fails() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  s.go(ProjectPage::Skill(id));
  let before = s.entry(id);
  let body = Skill::new(
    "Saved by another editor.".into(),
    "# External edit".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("body");
  editor::save_in(&s.repo, Some(&before), "checks", &body).expect("competing save");
  s.set("skill-description", "My competing draft.");
  s.window.until("save conflict", |w| {
    w.tree.get_element_by_id("skill-save-error").is_some()
  });
  s.click("skill-reload");
  s.click("skill-reload-confirm");
  s.window.until("reloaded editor", |w| {
    w.tree
      .get_element_by_id_mut("skill-body")
      .and_then(|e| e.as_text_input())
      .is_some_and(|input| input.value() == "# External edit")
  });
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_none());
  assert_eq!(s.entry(id).body.description(), "Saved by another editor.");
}
#[test]
fn skills_grouped_scope_picker_saves_an_agent_step_and_filters_the_library() {
  use orchester_core::{
    name::Name,
    resource::{Pipeline, PipelineEdit, Slug, StepKind},
    time::Timestamp,
  };
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("draft"));
  let agent = edit.add_step(Name::new("Implement").expect("name"), None);
  let tool = edit.add_step(Name::new("Checks").expect("name"), None);
  edit.update(&tool, |step| step.kind = StepKind::Tool { tool: None });
  let mut db = ProjectDb::open(&s.repo).expect("db");
  let pipeline = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &edit.validate().expect("pipeline"),
      Timestamp::now(),
    )
    .expect("create");
  s.go(ProjectPage::Skill(id));
  s.click("skill-scope");
  s.window
    .until("scope menu", |w| w.tree.get_element_by_id("skill-scope-menu").is_some());
  assert!(s.text("skill-scope-menu").contains("STEPS · DELIVERY"));
  s.click("skill-scope-option-3");
  assert_eq!(s.entry(id).body.scope(), &InstructionScope::Project);
  s.click("skill-scope-option-2");
  let root = s.repo.clone();
  let scope = InstructionScope::Step {
    pipeline: pipeline.id(),
    step: agent,
  };
  s.window.until("scope saved", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.body.scope() == &scope)
  });
  s.settle(id, "Run checks before handing over.");
  s.go(ProjectPage::Skills);
  s.window.until("library reflects the saved scope", |w| {
    w.texts("skills-list").is_some_and(|t| t.join(" ").contains("Delivery"))
  });
  s.click("skills-tab-Project");
  assert!(s.text("skills-list").contains("No skills in this scope"));
  s.click("skills-tab-Pipelines");
  assert!(s.text("skills-list").contains("Delivery"));
}

#[test]
fn skills_new_scope_without_a_name_asks_before_leaving_and_reverting_clears_the_guard() {
  use orchester_core::{
    name::Name,
    resource::{Pipeline, Slug},
    time::Timestamp,
  };
  let mut s = Skills::new();
  let mut db = ProjectDb::open(&s.repo).expect("db");
  db.create_resource(
    Slug::new("delivery").expect("slug"),
    Name::new("Delivery").expect("name"),
    &Pipeline::new(Vec::new()).expect("pipeline"),
    Timestamp::now(),
  )
  .expect("pipeline");
  s.go(ProjectPage::NewSkill);
  s.click("skill-scope");
  s.click("skill-scope-option-1");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  assert!(ProjectSkills::load_in(&s.repo).expect("skills").entries.is_empty());
  s.click("skill-leave-keep");
  assert!(s.text("skill-scope").contains("Delivery"));
  assert!(!s.text("skill-editor").contains("for the whole project"));
  s.click("skill-scope");
  s.click("skill-scope-option-0");
  s.go(ProjectPage::Skills);
  assert!(s.window.tree.get_element_by_id("skills-empty").is_some());
  assert!(s.window.tree.get_element_by_id("skill-leave-question").is_none());
  s.go(ProjectPage::NewSkill);
  s.click("skill-scope");
  s.click("skill-scope-option-1");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  s.click("skill-leave-keep");
  assert!(s.text("skill-scope").contains("Delivery"));
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  s.click("skill-leave-confirm");
  s.window.until("left scope-only form", |w| {
    w.tree.get_element_by_id("skills-empty").is_some()
  });
  assert!(ProjectSkills::load_in(&s.repo).expect("skills").entries.is_empty());
}
#[test]
fn skills_clearing_all_existing_fields_still_asks_before_leaving() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  s.go(ProjectPage::Skill(id));
  s.set("skill-name", "");
  s.set("skill-description", "");
  s.set("skill-body", "");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  assert_eq!(s.entry(id).name.as_ref(), "checks");
  s.click("skill-leave-keep");
  assert!(s.window.tree.get_element_by_id("skill-editor").is_some());
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills));
  s.window.pass();
  s.click("skill-leave-confirm");
  s.window
    .until("library", |w| w.tree.get_element_by_id("skills-list").is_some());
}

#[test]
fn skills_save_retry_saves_the_local_draft_without_a_publish_question() {
  let mut s = Skills::new();
  let id = s.seed("checks", false);
  s.go(ProjectPage::Skill(id));
  let before = s.entry(id);
  let changed = Skill::new(
    "Outside".into(),
    "# Outside".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("body");
  let outside = editor::save_in(&s.repo, Some(&before), "checks", &changed).expect("competing save");
  s.set("skill-description", "My local draft.");
  s.window.until("save failed", |w| {
    w.tree.get_element_by_id("skill-save-error").is_some()
  });
  editor::save_in(&s.repo, Some(&outside), "checks", &before.body).expect("resolve conflict");
  s.click("skill-retry");
  assert!(s.window.tree.get_element_by_id("skill-confirm-confirm").is_none());
  s.settle(id, "My local draft.");
  assert!(s.window.tree.get_element_by_id("skill-save-error").is_none());
  assert!(s.entry(id).state.published().is_none());
}
