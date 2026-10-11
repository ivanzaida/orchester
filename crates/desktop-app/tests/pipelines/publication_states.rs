//! The three supported publication problems and the 340px Summary stay readable and actionable.
use std::collections::BTreeMap;

use orchester_core::{
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, DatabaseQuery, DeniedBehavior, ModelId, Pipeline,
    PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::pipelines::{data::Assignables, narrowing::check_step};

use crate::pipelines_support::Editor;

fn name(text: &str) -> Name {
  Name::new(text).expect("valid publication fixture")
}

fn problems(editor: &Editor) -> (ResourceId, ResourceId) {
  let mut db = editor.db();
  let now = Timestamp::now();
  let profile = AgentProfile::new(
    name("Unpublished reviewer"),
    "".into(),
    PromptText::new("").expect("valid publication fixture"),
    ConnectionId::generate(),
    ModelId::new("qa-model").expect("valid publication fixture"),
    ProviderSettings::default(),
  );
  let profile = db
    .create_resource(
      Slug::new("unpublished-reviewer").expect("valid publication fixture"),
      name("Unpublished reviewer"),
      &profile,
      now,
    )
    .expect("valid publication fixture")
    .id();
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
  let tool = db
    .create_resource(
      Slug::new("unsupported-database").expect("valid publication fixture"),
      name("Unsupported database"),
      &tool,
      now,
    )
    .expect("valid publication fixture")
    .id();
  db.publish_draft::<Tool>(tool, now).expect("valid publication fixture");
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("valid publication fixture"));
  let first = edit.add_step(name("Missing profile"), None);
  let second = edit.add_step(name("Unpublished profile"), Some(&first));
  edit.update(&second, |s| {
    s.kind = StepKind::Agent {
      profile: Some(profile),
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions::default(),
    }
  });
  let third = edit.add_step(name("Unsupported tool"), Some(&second));
  edit.update(&third, |s| s.kind = StepKind::Tool { tool: Some(tool) });
  let id = db
    .create_resource(
      Slug::new("publication-problems").expect("valid publication fixture"),
      name("Publication problems"),
      &edit.validate().expect("valid publication fixture"),
      now,
    )
    .expect("valid publication fixture")
    .id();
  (id, profile)
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
fn pipelines_publication_problems_fit_and_select_every_named_step() {
  let mut editor = Editor::new();
  let (id, profile) = problems(&editor);
  editor.open(id);
  let strip = editor.text("pipeline-problem-strip");
  assert!(
    !strip.contains(&profile.to_string()),
    "raw resource UUID in problem links"
  );
  let steps = editor
    .db()
    .draft::<Pipeline>(id)
    .unwrap()
    .unwrap()
    .body()
    .steps()
    .to_vec();
  for (index, step) in steps.iter().enumerate() {
    let link = if index == 0 {
      "pipeline-problem-0".to_owned()
    } else {
      format!("pipeline-problem-executor-{}", step.key)
    };
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
  editor.click("pipeline-publish");
  assert!(!editor.exists("pipeline-publish-confirm"));
  assert!(editor.db().latest_version::<Pipeline>(id).unwrap().is_none());
}

#[test]
fn pipelines_unpublished_profile_shows_draft_identity_model_and_stays_unassignable() {
  let mut editor = Editor::new();
  let (id, profile) = problems(&editor);
  let choices = Assignables::load(&editor.db()).unwrap();
  assert!(choices.profile(profile).is_none());
  assert_eq!(choices.draft_profiles[0].name.as_ref(), "Unpublished reviewer");
  let step = editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[1].clone();
  assert!(check_step(&step, &choices).is_err());
  editor.open(id);
  editor.click(&format!("pipeline-step-{}", step.key));
  assert!(editor.text("step-executor").contains("Unpublished reviewer · draft v1"));
  assert!(
    editor
      .text("step-profile-unpublished")
      .contains("Publish it before this pipeline can publish")
  );
  assert!(editor.exists("step-open-profile"));
  assert!(editor.text("step-model").contains("qa-model"));
  assert!(editor.text("step-model-source").contains("From the profile’s draft"));
  editor.click("pipeline-publish");
  assert!(!editor.exists("pipeline-publish-confirm"));
}

#[test]
fn pipelines_unsupported_database_is_distinct_from_runnable_commands() {
  let mut editor = Editor::new();
  let (id, _) = problems(&editor);
  let command = editor.published();
  editor.open(id);
  let step = editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[2].clone();
  let card = format!("pipeline-step-{}", step.key);
  assert!(editor.text(&card).contains("Unavailable"), "{}", editor.text(&card));
  assert!(!editor.text(&card).contains("exit code"));
  editor.click(&card);
  assert!(editor.text("step-tool-unavailable").contains("Database actions"));
  assert!(!editor.text("step-tool-unavailable").contains("pull request"));
  assert!(editor.text("step-model").contains("Not used by tools"));
  editor.open(command);
  let step = editor
    .db()
    .latest_version::<Pipeline>(command)
    .unwrap()
    .unwrap()
    .body()
    .steps()[0]
    .clone();
  let card = format!("pipeline-step-{}", step.key);
  assert!(editor.text(&card).contains("exit code"));
  editor.click(&card);
  assert!(!editor.exists("step-tool-unavailable"));
}

#[test]
fn pipelines_summary_long_slug_and_zero_gates_have_room_for_their_lines() {
  let mut editor = Editor::new();
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

#[test]
fn pipelines_publication_problem_links_flow_onto_another_line() {
  let mut editor = Editor::new();
  let (id, _) = problems(&editor);
  let mut db = editor.db();
  let saved = db.draft::<Pipeline>(id).unwrap().unwrap();
  let mut edit = PipelineEdit::new(saved.body().clone());
  for n in 1..=5 {
    edit.add_step(name(&format!("Additional reviewer {n}")), None);
  }
  db.save_draft(
    id,
    &orchester_core::resource::Draft::new(edit.validate().unwrap(), None, Timestamp::now()),
  )
  .unwrap();
  editor.open(id);
  let first = bounds(&mut editor, "pipeline-problem-0");
  let last = bounds(&mut editor, "pipeline-problem-5");
  assert!(last.y > first.y, "links should wrap onto another line");
  let strip = bounds(&mut editor, "pipeline-problem-strip");
  for index in 0..6 {
    let link = format!("pipeline-problem-{index}");
    let rect = bounds(&mut editor, &link);
    assert!(
      rect.x >= strip.x && rect.x + rect.width <= strip.x + strip.width,
      "{link}: {rect:?}"
    );
    assert!(rect.y + rect.height <= strip.y + strip.height, "{link}: {rect:?}");
    editor.click(&link);
    assert!(editor.exists("step-profile-required"));
  }
}
