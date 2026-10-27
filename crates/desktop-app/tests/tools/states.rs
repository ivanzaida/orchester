use orchester_app::routes::{ProjectPage, Route, ToolTab};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::agents::data::VersionState;

use super::harness::{Tools, command, command_text};

#[test]
fn tools_library_reports_kind_state_version_and_published_callers() {
  let mut t = Tools::new();
  let id = t.seed("Run checks", true);
  let step = Step {
    key: StepKey::new("checks").expect("key"),
    name: Name::new("Run checks").expect("name"),
    kind: StepKind::Tool { tool: Some(id) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let mut db = t.db();
  let pipeline_id = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline created")
    .id();
  db.publish_draft::<Pipeline>(pipeline_id, Timestamp::now())
    .expect("published");
  let draft = t.seed("Unpublished", false);
  t.go(ProjectPage::Tools);
  let row = t.text(&format!("tool-row-{id}"));
  for word in ["Command", "Published v1", "1 pipeline step", "Ready"] {
    assert!(row.contains(word), "{row} lacks {word}");
  }
  assert!(!row.contains("1 pipeline steps"), "{row}");
  assert!(t.text(&format!("tool-row-{draft}")).contains("not published"));
  t.click(&format!("tool-row-{id}"));
  t.window
    .until("editor", |w| w.tree.get_element_by_id("tool-properties").is_some());
  t.click("tool-tab-access");
  assert!(t.text("tool-access-tab").contains("Exit code decides the step"));
  let access = t.text("tool-access-step-0");
  assert!(access.contains("Delivery v1 / 01 Run checks"), "{access}");
  assert!(t.window.tree.get_element_by_id("tool-access-open-0").is_some());
  t.click("tool-tab-inputs_outputs");
  t.wait_for("tool-inputs-outputs");
  assert!(t.text("tool-inputs-outputs").contains("Exit code"));
  t.click("tool-tab-usage_log");
  t.wait_for("tool-usage-unavailable");
  assert!(t.text("tool-usage-unavailable").contains("not available yet"));
}

#[test]
fn tools_invalid_values_and_repository_programs_cannot_be_saved() {
  let mut t = Tools::new();
  t.go(ProjectPage::NewTool);
  t.set("tool-name", "Checks");
  t.set("tool-program", "scripts/check");
  t.wait_note("search path");
  t.set("tool-timeout", "soon");
  t.set("tool-exitCodes", "zero");
  t.set("tool-outputLimit", "0");
  t.set("tool-environment", "1ST=bad");
  for id in [
    "tool-timeout-note",
    "tool-exitCodes-note",
    "tool-outputLimit-note",
    "tool-environment-note",
  ] {
    assert!(t.exists(id));
  }
  t.click("tool-create");
  assert!(!t.exists("tool-confirm"));
  assert!(
    orchester_workspace::tools::data::ProjectTools::load(&t.db())
      .expect("tools")
      .tools
      .is_empty()
  );
  let program = t.repo.join("program.exe");
  std::fs::write(&program, "fixture").expect("program file");
  t.set("tool-timeout", "60");
  t.set("tool-exitCodes", "0");
  t.set("tool-outputLimit", "64");
  t.set("tool-environment", "");
  t.set("tool-program", &program.to_string_lossy());
  t.wait_note("repository");
}

#[test]
fn tools_stale_draft_can_be_kept_or_replaced_with_latest() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  let original = t.entry(id).tool;
  let mut old = command_text(&original);
  old.description = "Changes in the stale draft".into();
  let old = old.tool(None).expect("draft body");
  let mut latest = command_text(&original);
  latest.description = "Latest version".into();
  let mut db = t.db();
  db.save_draft(
    id,
    &Draft::new(
      latest.tool(None).expect("tool"),
      Some(VersionNumber::FIRST),
      Timestamp::now(),
    ),
  )
  .expect("draft");
  let v2 = db.publish_draft::<Tool>(id, Timestamp::now()).expect("v2");
  db.save_draft(
    id,
    &Draft::new(old.clone(), Some(VersionNumber::FIRST), Timestamp::now()),
  )
  .expect("stale draft");
  t.edit(id);
  assert!(t.exists("tool-stale"));
  let notice = t.text("tool-stale");
  assert!(notice.contains("v1") && notice.contains("v2"), "{notice}");
  t.click("tool-publish");
  assert!(!t.exists("tool-confirm"));
  t.click("tool-keep-draft");
  t.window.until("rebased draft", |w| {
    w.tree.get_element_by_id("tool-editor").is_some() && w.tree.get_element_by_id("tool-stale").is_none()
  });
  assert_eq!(
    t.db().draft::<Tool>(id).expect("draft").expect("present").based_on(),
    Some(v2.number())
  );
  assert_eq!(t.entry(id).tool, old);
  db.save_draft(id, &Draft::new(old, Some(VersionNumber::FIRST), Timestamp::now()))
    .expect("stale again");
  t.go(ProjectPage::Tools);
  t.edit(id);
  t.click("tool-start-latest");
  t.window.until("latest draft", |w| {
    w.tree.get_element_by_id("tool-editor").is_some() && w.tree.get_element_by_id("tool-stale").is_none()
  });
  assert_eq!(t.entry(id).tool, v2.body().clone());
  assert!(matches!(t.entry(id).state, VersionState::Published(_)));
}

#[test]
fn tools_routes_preserve_tool_and_tab_and_missing_tool_is_explicit() {
  let mut t = Tools::new();
  let id = ResourceId::generate();
  for tab in ToolTab::ALL {
    let route = Route::Project(super::support::project_id(), ProjectPage::Tool(id, tab));
    assert_eq!(Route::parse(&route.path()), Ok(route));
  }
  let new = Route::Project(super::support::project_id(), ProjectPage::NewTool);
  assert_eq!(Route::parse(&new.path()), Ok(new));
  t.go(ProjectPage::Tool(id, ToolTab::Configuration));
  assert!(t.text("tool-missing").contains("no longer available"));
  // Fixture command construction remains independent of any installed CLI.
  assert!(command_text(&command()).program.ends_with(std::env::consts::EXE_SUFFIX));
}
