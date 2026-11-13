//! Skills in the real headless app, with isolated project data and no agent process.
#[path = "skills/draft_only.rs"]
mod draft_only;
#[path = "skills/recovery.rs"]
mod recovery;
mod support;
#[path = "skills/visual.rs"]
mod visual;
use std::{fs, path::PathBuf};

use orchester_app::routes::{ProjectPage, Route, SkillsFilter};
use orchester_core::{
  id::ResourceId,
  resource::{InstructionScope, Skill},
  store::ResourceStore,
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
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/skills");
    fs::create_dir_all(&scratch).expect("private Skills fixture folder");
    let data = support::Data::at(
      tempfile::tempdir_in(scratch).expect("private Skills fixture"),
      tokio::runtime::Runtime::new().expect("tokio runtime"),
    );
    let launch = data.launch();
    data.add_kontur(launch.workspace().expect("the session started"));
    let window = Window::of(data, launch);
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
    if published {
      return editor::save_in(&self.repo, None, name, &body).expect("create as v1").id;
    }
    draft_only::create(&self.repo, name, &body)
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
fn skills_new_edit_autosave_and_publish_keep_versions_separate() {
  let mut s = Skills::new();
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.text("skills-empty").contains("No skills in Kontur yet"));
  s.click("skills-new");
  s.set("skill-name", "run-checks");
  s.set("skill-description", "Run checks after changing code.");
  s.set("skill-body", "# Checks\nRun the checks.");
  s.click("skill-create");
  s.window.until("created skill editor", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Skill(_)))
  });
  let Route::Project(_, ProjectPage::Skill(id)) = s.window.navigation().route_now() else {
    panic!("editor route")
  };
  // A new skill is created as v1 with no draft; editing it afterwards saves a draft over v1.
  assert_eq!(s.entry(id).state.published().expect("created as v1").get(), 1);
  assert!(!s.entry(id).state.has_draft());
  s.window.until("editor ready", |w| {
    w.tree.get_element_by_id("skill-description").is_some()
  });
  s.set("skill-body", "# Checks\nRun the checks, then read their output.");
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
  assert_eq!(s.entry(id).state.published().expect("v1 kept").get(), 1);
  s.click("skill-publish");
  s.window.until("person-only question armed", |w| {
    w.tree.tick_futures();
    w.tree.get_element_by_id("skill-confirm-confirm").is_some()
  });
  s.click("skill-confirm-confirm");
  let root = s.repo.clone();
  s.window.until("published v2", move |_| {
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| e.state.published().is_some_and(|v| v.get() == 2))
  });
  s.window.until("published editor", |w| {
    w.texts("skill-save-status")
      .is_some_and(|t| t.join(" ").contains("Published v2"))
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
fn skills_archive_is_asked_in_place_and_the_skill_leaves_the_library() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  s.go(ProjectPage::Skill(id));
  s.click("skill-more");
  s.click("skill-archive");
  s.window.until("asked in place", |w| {
    w.tree.tick_futures();
    w.tree.get_element_by_id("team-hub-archive-confirm").is_some()
  });
  assert!(
    s.text("team-hub-archive-body").contains("leaves every list"),
    "{}",
    s.text("team-hub-archive-body")
  );
  let root = s.repo.clone();
  // Archive takes the press once the question has read what uses the skill.
  s.window.until("archived", move |w| {
    if let Some(mut confirm) = w.tree.get_element_by_id_mut("team-hub-archive-confirm") {
      confirm.click();
    }
    ProjectSkills::load_in(&root).expect("skills").entry(id).is_none()
  });
}
#[test]
fn skills_discard_draft_returns_to_the_published_version() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  s.go(ProjectPage::Skill(id));
  s.set("skill-description", "A draft to throw away.");
  s.settle(id, "A draft to throw away.");
  s.click("skill-discard");
  s.window.until("asked in place", |w| {
    w.tree.tick_futures();
    w.tree.get_element_by_id("hub-discard-confirm").is_some()
  });
  let root = s.repo.clone();
  s.window.until("discarded", move |w| {
    if let Some(mut confirm) = w.tree.get_element_by_id_mut("hub-discard-confirm") {
      confirm.click();
    }
    ProjectSkills::load_in(&root)
      .expect("skills")
      .entry(id)
      .is_some_and(|e| !e.state.has_draft() && e.body.description() == "Run checks before handing over.")
  });
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
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  s.click("skill-leave-keep");
  assert!(s.window.tree.get_element_by_id("skill-editor").is_some());
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
  s.click("skill-leave-confirm");
  s.window
    .until("library", |w| w.tree.get_element_by_id("skills-list").is_some());
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
  // A pipeline is stored only as published versions, whose steps all run something (D-032).
  edit.update(&agent, |step| {
    if let StepKind::Agent { profile, .. } = &mut step.kind {
      *profile = Some(orchester_core::id::ResourceId::generate());
    }
  });
  edit.update(&tool, |step| {
    step.kind = StepKind::Tool {
      tool: Some(orchester_core::id::ResourceId::generate()),
    }
  });
  let mut db = ProjectDb::open(&s.repo).expect("db");
  let (pipeline, _) = db
    .create_published(
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
  s.go(ProjectPage::Skills(Default::default()));
  s.window.until("library reflects the saved scope", |w| {
    w.texts("skills-list").is_some_and(|t| t.join(" ").contains("Delivery"))
  });
  // One list, a group per scope (03): the skill is under Pipelines & steps, and no Project group is left empty.
  assert!(s.text("skills-group-pipelines").contains("Delivery"));
  assert!(s.window.tree.get_element_by_id("skills-group-project").is_none());
}

#[test]
fn skills_new_scope_without_a_name_asks_before_leaving_and_reverting_clears_the_guard() {
  use orchester_core::{name::Name, resource::Slug, time::Timestamp};
  let mut s = Skills::new();
  let mut db = ProjectDb::open(&s.repo).expect("db");
  db.create_published(
    Slug::new("delivery").expect("slug"),
    Name::new("Delivery").expect("name"),
    &one_step(),
    Timestamp::now(),
  )
  .expect("pipeline");
  s.go(ProjectPage::NewSkill);
  s.click("skill-scope");
  s.click("skill-scope-option-1");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  assert!(ProjectSkills::load_in(&s.repo).expect("skills").entries.is_empty());
  s.click("skill-leave-keep");
  assert!(s.text("skill-scope").contains("Delivery"));
  assert!(!s.text("skill-editor").contains("for the whole project"));
  s.click("skill-scope");
  s.click("skill-scope-option-0");
  s.go(ProjectPage::Skills(Default::default()));
  assert!(s.window.tree.get_element_by_id("skills-empty").is_some());
  assert!(s.window.tree.get_element_by_id("skill-leave-question").is_none());
  s.go(ProjectPage::NewSkill);
  s.click("skill-scope");
  s.click("skill-scope-option-1");
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
  s.click("skill-leave-keep");
  assert!(s.text("skill-scope").contains("Delivery"));
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
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
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
  s.window.pass();
  assert!(s.text("skill-leave-question").contains("unsaved"));
  assert_eq!(s.entry(id).name.as_ref(), "checks");
  s.click("skill-leave-keep");
  assert!(s.window.tree.get_element_by_id("skill-editor").is_some());
  s.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Skills(Default::default())));
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

/// The crumb back to the library returns to the filter the skill was opened from.
#[test]
fn skills_crumb_returns_to_the_library_filter_it_came_from() {
  let mut s = Skills::new();
  let id = s.seed("checks", true);
  let library = Route::Project(project_id(), ProjectPage::Skills(SkillsFilter::Project));
  s.window.navigation().go(library.clone());
  s.go(ProjectPage::Skill(id));
  s.window.until("the skill's crumb", |w| {
    w.tree.get_element_by_id("skill-crumb").is_some()
  });
  s.click("skill-crumb");
  s.window.pass();
  assert_eq!(s.window.navigation().route_now(), library);
}
/// A pipeline of one step that runs a profile, as every stored pipeline's steps run something (D-032).
pub(crate) fn one_step() -> orchester_core::resource::Pipeline {
  use orchester_core::resource::{Pipeline, PipelineEdit, StepKind};
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let key = edit.add_step(orchester_core::name::Name::new("Build").expect("name"), None);
  edit.update(&key, |step| {
    if let StepKind::Agent { profile, .. } = &mut step.kind {
      *profile = Some(orchester_core::id::ResourceId::generate());
    }
  });
  edit.validate().expect("one step that runs a profile")
}
