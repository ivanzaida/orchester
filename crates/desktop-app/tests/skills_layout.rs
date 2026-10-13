//! The Skills editor keeps a visible body and a separate inspector at the task's 1440 size.
mod support;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::resource::{InstructionScope, Skill};
use orchester_workspace::skills::editor;
use support::{Window, project_id};
#[test]
fn skills_layout_keeps_body_and_inspector_visible_at_1440() {
  let mut w = Window::open();
  let repo = w.data.dir.path().join("repo-kontur");
  let body = Skill::new(
    "Run checks.".into(),
    "# Checks\nRun the checks.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("body");
  let skill = editor::save_in(&repo, None, "run-checks", &body).expect("skill");
  w.navigation()
    .go(Route::Project(project_id(), ProjectPage::Skill(skill.id)));
  w.until("editor", |w| w.tree.get_element_by_id("skill-body").is_some());
  w.until("body input fills the measured viewport", |w| {
    w.tree
      .get_element_by_id_mut("skill-body")
      .and_then(|e| e.bounds())
      .is_some_and(|b| b.height > 400.0)
  });
  let body = w
    .tree
    .get_element_by_id_mut("skill-body")
    .and_then(|e| e.bounds())
    .expect("body bounds");
  let inspector = w
    .tree
    .get_element_by_id_mut("skill-inspector")
    .and_then(|e| e.bounds())
    .expect("inspector bounds");
  assert!(body.width > 500.0 && body.height > 20.0, "body visible: {body:?}");
  let area = w
    .tree
    .get_element_by_id_mut("skill-body-box")
    .and_then(|e| e.bounds())
    .expect("body area");
  assert!(area.height > 380.0, "the editor keeps its empty body visible: {area:?}");
  assert!(
    body.height >= area.height - 40.0,
    "the input fills the clickable editor: body={body:?}, area={area:?}"
  );
  let first = w
    .tree
    .get_element_by_id_mut("skill-body-line-1")
    .and_then(|e| e.bounds())
    .expect("first line");
  let second = w
    .tree
    .get_element_by_id_mut("skill-body-line-2")
    .and_then(|e| e.bounds())
    .expect("second line");
  assert!(
    (second.y - first.y - 20.0).abs() < 0.1,
    "line gutter follows source rows"
  );
  assert!(
    inspector.width > 300.0 && inspector.x >= body.x + body.width,
    "inspector stays beside body"
  );
}
