//! The three supported publication problems of a published pipeline (a profile that is gone, one never published and a
//! tool that cannot run) and the 340px Summary stay readable and actionable. The pipeline is the team's: its page reads
//! it from the team's configuration in memory, so the fixture keeps it there.
use std::collections::BTreeMap;

use orchester_core::{
  id::{ResourceId, SecretId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DatabaseQuery, DeniedBehavior, Pipeline, PipelineEdit, RepositoryAccess,
    ResourceKind, StepKind, Tool, ToolAction,
  },
};

use crate::pipelines_support::Editor;

fn name(text: &str) -> Name {
  Name::new(text).expect("valid publication fixture")
}

/// The steps with the three problems, and the team profile that has no version: the first step runs a profile that is
/// gone, the second that profile, the third a tool that cannot run.
fn problem_steps(editor: &Editor) -> (Pipeline, ResourceId) {
  let profile = editor.the_teams(
    (
      ResourceKind::AgentProfile,
      "unpublished-reviewer",
      "Unpublished reviewer",
    ),
    None,
  );
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    name("QA"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("valid publication fixture");
  let tool = Tool::new(
    "",
    ToolAction::DatabaseQuery(
      DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("valid publication fixture"),
    ),
    Some(policy),
  )
  .expect("valid publication fixture");
  let tool = editor.the_teams(
    (ResourceKind::Tool, "unsupported-database", "Unsupported database"),
    Some(serde_json::to_value(&tool).expect("valid publication fixture")),
  );
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("valid publication fixture"));
  let first = edit.add_step(name("Missing profile"), None);
  // A stored pipeline's steps all run something (D-032): the first runs a profile that is gone.
  edit.update(&first, |s| s.kind = agent(ResourceId::generate()));
  let second = edit.add_step(name("Unpublished profile"), Some(&first));
  edit.update(&second, |s| s.kind = agent(profile));
  let third = edit.add_step(name("Unsupported tool"), Some(&second));
  edit.update(&third, |s| s.kind = StepKind::Tool { tool: Some(tool) });
  (edit.validate().expect("valid publication fixture"), profile)
}

fn agent(profile: ResourceId) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    model: None,
    repository: RepositoryAccess::ReadOnly,
    permissions: StepPermissions::default(),
  }
}

/// Keeps `body` as the team pipeline *Publication problems*, as a read of the team server holds it; its id.
fn kept(editor: &Editor, body: &Pipeline) -> ResourceId {
  editor.the_teams(
    (ResourceKind::Pipeline, "publication-problems", "Publication problems"),
    Some(serde_json::to_value(body).expect("valid publication fixture")),
  )
}

fn problems(editor: &Editor) -> (ResourceId, ResourceId) {
  let (body, profile) = problem_steps(editor);
  (kept(editor, &body), profile)
}

fn bounds(editor: &mut Editor, id: &str) -> lurq::core::ElementRect {
  editor
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .unwrap_or_else(|| panic!("laid out: {id}"))
}

#[test]
fn pipelines_legal_long_names_keep_problem_actions_selectable_and_graph_cards_separate() {
  let mut editor = Editor::team();
  let (saved, _) = problem_steps(&editor);
  let mut edit = PipelineEdit::new(saved);
  let keys: Vec<_> = edit.steps().iter().map(|step| step.key.clone()).collect();
  for (index, key) in keys.iter().take(2).enumerate() {
    let prefix = if index == 0 { "Long name " } else { "Long step " };
    let long = format!("{prefix}{}", "W".repeat(190));
    assert_eq!(long.chars().count(), 200);
    edit.update(key, |step| step.name = name(&long));
  }
  let body = edit.validate().unwrap();
  let id = kept(&editor, &body);
  editor.open(id);
  let mut previous_bottom = 0.0;
  for step in body.steps() {
    let link = format!("pipeline-problem-executor-{}", step.key);
    let action = bounds(&mut editor, &link);
    let label = bounds(&mut editor, &format!("{link}-label"));
    let strip = bounds(&mut editor, "pipeline-problem-strip");
    assert!(
      action.x >= strip.x && action.x + action.width <= strip.x + strip.width,
      "{action:?}"
    );
    assert!(
      label.x >= action.x && label.x + label.width <= action.x + action.width,
      "{label:?}"
    );
    click_center(&mut editor, &link);
    assert_eq!(
      editor.value("step-name"),
      step.name.as_str(),
      "full name remains editable"
    );
    let card = bounds(&mut editor, &format!("pipeline-step-{}", step.key));
    let title = bounds(&mut editor, &format!("pipeline-step-title-{}", step.key));
    let executor = bounds(&mut editor, &format!("pipeline-step-executor-{}", step.key));
    assert!(
      card.y >= previous_bottom + 10.0,
      "cards and the edges between them must stay separate: {card:?}"
    );
    assert!(title.y + title.height <= executor.y, "title covers metadata");
    assert!(
      executor.y + executor.height < card.y + card.height,
      "metadata covers access badge"
    );
    assert!(title.x + title.width <= card.x + card.width, "title exceeds card");
    previous_bottom = card.y + card.height;
  }
}

fn click_center(editor: &mut Editor, id: &str) {
  let rect = bounds(editor, id);
  let (x, y) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
  editor.window.tree.mouse_move(x, y);
  editor
    .window
    .tree
    .mouse_down(x, y, lurq::app::events::MouseButton::Left);
  editor.window.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
  editor.window.pass();
  editor.window.pass();
}

#[test]
fn pipelines_publication_problems_fit_and_select_every_named_step() {
  let mut editor = Editor::team();
  let (id, profile) = problems(&editor);
  editor.open(id);
  let strip = editor.text("pipeline-problem-strip");
  assert!(
    !strip.contains(&profile.to_string()),
    "raw resource UUID in problem links"
  );
  let steps = editor.stored(id).steps().to_vec();
  for step in &steps {
    let link = format!("pipeline-problem-executor-{}", step.key);
    let rect = bounds(&mut editor, &link);
    assert!(rect.x >= 240.0 && rect.x + rect.width <= 1440.0, "{link}: {rect:?}");
    let strip = bounds(&mut editor, "pipeline-problem-strip");
    assert!(
      rect.y >= strip.y && rect.y + rect.height <= strip.y + strip.height,
      "{link}: {rect:?}"
    );
    editor.click(&link);
    assert_eq!(editor.value("step-name"), step.name.as_str());
  }
  assert_eq!(editor.version(id), 1, "choosing a problem's step publishes nothing");
}

#[test]
fn pipelines_summary_long_slug_and_zero_gates_have_room_for_their_lines() {
  let mut editor = Editor::team();
  let (id, _) = problems(&editor);
  editor.open(id);
  assert_eq!(editor.text("pipeline-slug-value"), "publication-problems");
  assert_eq!(editor.text("pipeline-step-count-value"), "3 · 0 approval gates");
  for id in ["pipeline-slug", "pipeline-step-count"] {
    let row = bounds(&mut editor, id);
    let value = bounds(&mut editor, &format!("{id}-value"));
    assert!(value.height >= 12.0, "{id} collapsed: {value:?}");
    assert!(
      value.y + value.height <= row.y + row.height,
      "{id} clips vertically: {row:?}, {value:?}"
    );
    assert!(value.x + value.width <= 1432.0, "{id} clips horizontally: {value:?}");
  }
}
