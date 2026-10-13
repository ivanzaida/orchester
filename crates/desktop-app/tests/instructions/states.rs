use orchester_core::{
  resource::{Draft, Instruction, InstructionScope},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::instructions::editor;

use super::harness::{Instructions, body};

#[test]
fn instructions_empty_and_new_draft_publish() {
  let mut screen = Instructions::new();
  screen.go();
  assert!(
    screen
      .text("instructions-empty")
      .contains("No instructions in Kontur yet")
  );
  screen.click("instructions-new");
  screen.until("new editor", |s| s.exists("instruction-name"));
  screen.set("instruction-name", "Working agreement");
  screen.set("instruction-prompt", "Follow the approved plan.");
  screen.click("instruction-create");
  screen.until("draft created", |s| s.exists("instruction-publish"));
  let id = screen.listed().entries[0].id;
  assert!(screen.entry(id).state.has_draft());
  screen.click("instruction-publish");
  screen.until("published", |s| {
    s.entry(id).state.published().is_some() && !s.entry(id).state.has_draft()
  });
  assert_eq!(screen.entry(id).body.text().as_str(), "Follow the approved plan.");
}
#[test]
fn instructions_autosave_retains_published_version_then_publishes_next() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published text", InstructionScope::Project, true);
  screen.go();
  screen.set("instruction-prompt", "New draft text");
  assert!(screen.text("instruction-save-status").contains("Unsaved"));
  screen.until("draft saved", |s| {
    s.entry(id).body.text().as_str() == "New draft text"
      && s.text("instruction-save-status").contains("Draft saved · Published v1")
  });
  let db = ProjectDb::open(&screen.root).expect("valid instruction fixture");
  assert_eq!(
    db.latest_version::<Instruction>(id)
      .expect("valid instruction fixture")
      .expect("valid instruction fixture")
      .body()
      .text()
      .as_str(),
    "Published text"
  );
  screen.click("instruction-publish");
  screen.until("v2 published", |s| {
    s.text("instruction-save-status").contains("Published v2")
  });
  assert_eq!(
    db.latest_version::<Instruction>(id)
      .expect("valid instruction fixture")
      .expect("valid instruction fixture")
      .number()
      .get(),
    2
  );
}
#[test]
fn instructions_invalid_placeholder_keeps_typed_text_and_holds_navigation() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Saved text", InstructionScope::Project, true);
  screen.go();
  screen.set("instruction-prompt", "Unknown {{secret.value}}");
  assert!(screen.exists("instruction-text-error"));
  screen.pump();
  assert_eq!(screen.entry(id).body.text().as_str(), "Saved text");
  screen.click("instruction-tab-1");
  assert!(screen.exists("instruction-leave-question"));
  assert!(screen.text("instruction-prompt").contains("secret.value"));
  screen.click("instruction-keep");
  assert!(screen.exists("instruction-prompt"));
}
#[test]
fn instructions_checked_write_refuses_changes_made_elsewhere() {
  let screen = Instructions::new();
  let id = screen.rule("Standards", "Initial", InstructionScope::Project, true);
  let opened = screen.entry(id);
  let changed = editor::save_in(&screen.root, &opened, body("Outside draft", InstructionScope::Project))
    .expect("valid instruction fixture");
  assert!(editor::save_in(&screen.root, &opened, body("Delayed save", InstructionScope::Project)).is_err());
  assert!(editor::publish_in(&screen.root, &opened).is_err());
  assert_eq!(screen.entry(id), changed);
}
#[test]
fn instructions_blank_publish_fails_in_place_and_keeps_the_draft() {
  let mut screen = Instructions::new();
  let id = screen.rule("Empty", "", InstructionScope::Project, false);
  screen.go();
  screen.click("instruction-publish");
  screen.until("publish failed", |s| s.exists("instruction-write-error"));
  assert!(screen.entry(id).state.has_draft());
  assert!(screen.entry(id).state.published().is_none());
}
#[test]
fn instructions_stale_draft_is_visible_and_cannot_publish() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published", InstructionScope::Project, true);
  ProjectDb::open(&screen.root)
    .expect("valid instruction fixture")
    .save_draft(
      id,
      &Draft::new(body("Stale", InstructionScope::Project), None, Timestamp::now()),
    )
    .expect("valid instruction fixture");
  screen.go();
  assert!(screen.text("instruction-save-status").contains("Stale"));
  screen.click("instruction-publish");
  screen.pump();
  assert_eq!(
    screen
      .entry(id)
      .state
      .published()
      .expect("valid instruction fixture")
      .get(),
    1
  );
}
#[test]
fn instructions_import_reviews_repository_and_only_creates_drafts() {
  let mut screen = Instructions::new();
  std::fs::write(screen.root.join("AGENTS.md"), "Keep changes small.").expect("valid instruction fixture");
  std::fs::write(screen.root.join("CLAUDE.md"), "@AGENTS.md").expect("valid instruction fixture");
  screen.go();
  screen.click("instructions-import");
  screen.until("scan", |s| s.exists("instruction-import-create"));
  assert!(screen.text("instruction-import-review").contains("nothing to import"));
  screen.click("instruction-import-create");
  screen.until("imported", |s| s.exists("instruction-import-result"));
  let listed = screen.listed();
  assert_eq!(listed.entries.len(), 1);
  assert!(listed.entries[0].state.has_draft());
  assert!(listed.entries[0].state.published().is_none());
  assert_eq!(
    std::fs::read_to_string(screen.root.join("AGENTS.md")).expect("valid instruction fixture"),
    "Keep changes small."
  );
}

#[test]
fn instructions_all_scope_tabs_show_their_editor_and_save_timing() {
  let mut screen = Instructions::new();
  let pipeline = super::preview::seed_preview(&screen);
  let step = screen
    .listed()
    .pipeline(pipeline)
    .expect("valid instruction fixture")
    .steps[0]
    .key
    .clone();
  let role = screen.rule(
    "Builder agreement",
    "Role text",
    InstructionScope::AgentRole {
      role: orchester_core::name::Name::new("Builder").expect("valid instruction fixture"),
    },
    true,
  );
  let pipeline_rule = screen.rule(
    "Delivery agreement",
    "Pipeline text",
    InstructionScope::Pipeline { pipeline },
    true,
  );
  let step_rule = screen.rule(
    "Implement agreement",
    "Step text",
    InstructionScope::Step { pipeline, step },
    true,
  );
  screen.go();
  screen.click("instruction-tab-1");
  screen.until("role", |s| s.exists("instruction-role"));
  assert!(screen.text("instruction-library").contains("Builder agreement"));
  screen.choose("instruction-event", "Start & resume");
  screen.until("timing saved", |s| {
    s.entry(role).body.events().len() == 2 && s.text("instruction-save-status").contains("Draft saved · Published v1")
  });
  screen.click("instruction-tab-2");
  screen.until("pipeline", |s| s.exists("instruction-pipeline"));
  assert!(screen.text("instruction-library").contains("Delivery agreement"));
  assert_eq!(
    screen
      .entry(pipeline_rule)
      .state
      .published()
      .expect("valid instruction fixture")
      .get(),
    1
  );
  screen.click("instruction-tab-3");
  screen.until("step", |s| s.exists("instruction-step"));
  assert!(screen.text("instruction-library").contains("Implement agreement"));
  assert!(matches!(
    screen.entry(step_rule).body.scope(),
    InstructionScope::Step { .. }
  ));
}
#[test]
fn instructions_outside_conflict_keeps_typed_text_then_reloads_explicitly() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Original", InstructionScope::Project, true);
  screen.go();
  let opened = screen.entry(id);
  editor::save_in(&screen.root, &opened, body("Outside", InstructionScope::Project))
    .expect("valid instruction fixture");
  screen.set("instruction-prompt", "My unsaved text");
  screen.until("conflict", |s| s.exists("instruction-write-error"));
  assert_eq!(screen.entry(id).body.text().as_str(), "Outside");
  assert!(screen.text("instruction-prompt").contains("My unsaved text"));
  screen.click("instruction-reload");
  screen.until("reloaded", |s| {
    s.text("instruction-prompt") == "Outside" && !s.exists("instruction-write-error")
  });
}
#[test]
fn instructions_new_scope_starts_a_fresh_inline_editor() {
  let mut screen = Instructions::new();
  super::preview::seed_preview(&screen);
  screen.go();
  screen.click("instruction-tab-1");
  screen.click("instructions-new");
  screen.until("new role", |s| s.exists("instruction-role"));
  screen.set("instruction-name", "Role rule");
  screen.choose("instruction-role", "Builder");
  screen.set("instruction-prompt", "Do the work.");
  screen.click("instruction-create");
  screen.until("created role", |s| s.exists("instruction-publish"));
  screen.click("instruction-tab-2");
  screen.click("instructions-new");
  screen.until("new pipeline", |s| s.exists("instruction-pipeline"));
  assert_eq!(screen.text("instruction-name"), "");
}
#[test]
fn instructions_theme_roles_match_the_prompt_and_library_design() {
  let mut app = lurq::app::App::new();
  orchester_app::theme::install(&mut app, orchester_app::theme::Mode::Dark);
  let theme = app.theme();
  assert_eq!(
    theme.spacing_value(orchester_app::theme::instructions::head_top()),
    lurq::node::dimension::Dimension::Px(18.0)
  );
  assert_eq!(
    theme
      .typography_style(orchester_app::theme::instructions::prompt_text())
      .font_size,
    12.0
  );
  assert_eq!(
    theme
      .typography_style(orchester_app::theme::instructions::gutter_text())
      .font_size,
    11.0
  );
  assert_eq!(
    theme
      .typography_style(orchester_app::theme::instructions::rule_name())
      .font_size,
    12.5
  );
}

#[test]
fn instructions_failed_load_is_visible_and_can_retry() {
  let mut screen = Instructions::new();
  screen.rule("Standards", "Saved", InstructionScope::Project, false);
  screen.go();
  screen.window.navigation().go(orchester_app::routes::Route::AllProjects);
  screen.window.pass();
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("valid instruction fixture");
  let stored: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("valid instruction fixture");
  db.execute("UPDATE resource_drafts SET body='broken'", [])
    .expect("valid instruction fixture");
  screen.window.navigation().go(orchester_app::routes::Route::Project(
    super::support::project_id(),
    orchester_app::routes::ProjectPage::Instructions,
  ));
  screen.window.pass();
  assert!(screen.exists("instructions-loading"));
  screen.until("load failure", |s| s.exists("instructions-load-error"));
  assert!(!screen.exists("instructions-empty"));
  db.execute("UPDATE resource_drafts SET body=?1", [stored])
    .expect("valid instruction fixture");
  screen.click("instructions-load-error-retry");
  screen.until("reloaded", |s| s.exists("instruction-prompt"));
}
#[test]
fn instructions_selecting_agents_saves_the_normalized_targets() {
  let mut screen = Instructions::new();
  super::preview::seed_preview(&screen);
  let id = screen.rule("Standards", "For builders", InstructionScope::Project, true);
  screen.go();
  screen.choose("instruction-targets", "Builder");
  screen.until("targets saved", |s| {
    matches!(
      s.entry(id).body.targets(),
      orchester_core::resource::AgentTargets::Roles(_)
    ) && s.text("instruction-save-status").contains("Draft saved · Published v1")
  });
  assert_eq!(
    screen.entry(id).body.targets(),
    &orchester_core::resource::AgentTargets::Roles(std::collections::BTreeSet::from([
      orchester_core::name::Name::new("Builder").expect("valid instruction fixture")
    ]))
  );
}
