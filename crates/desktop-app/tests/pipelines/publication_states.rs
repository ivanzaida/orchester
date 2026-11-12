//! The three supported publication problems of a published pipeline (a profile that is gone, one never published and a
//! tool that cannot run) and the 340px Summary stay readable and actionable.
use std::collections::BTreeMap;

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, DatabaseQuery, DeniedBehavior, ModelId, Pipeline,
    PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ResourceSharing};

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
  // A stored pipeline's steps all run something (D-032): the first runs a profile that is gone.
  edit.update(&first, |s| {
    s.kind = StepKind::Agent {
      profile: Some(ResourceId::generate()),
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions::default(),
    }
  });
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
    .create_published(
      Slug::new("publication-problems").expect("valid publication fixture"),
      name("Publication problems"),
      &edit.validate().expect("valid publication fixture"),
      now,
    )
    .expect("valid publication fixture")
    .0
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
fn pipelines_legal_long_names_keep_problem_actions_selectable_and_graph_cards_separate() {
  let mut editor = Editor::team();
  let (id, _) = problems(&editor);
  let saved = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let mut edit = PipelineEdit::new(saved.body().clone());
  let keys: Vec<_> = edit.steps().iter().map(|step| step.key.clone()).collect();
  for (index, key) in keys.iter().take(2).enumerate() {
    let prefix = if index == 0 { "Long name " } else { "Long step " };
    let long = format!("{prefix}{}", "W".repeat(190));
    assert_eq!(long.chars().count(), 200);
    edit.update(key, |step| step.name = name(&long));
  }
  let body = edit.validate().unwrap();
  // A team pipeline's next version is kept here only as the team server numbered it (D-043), as the stream keeps it.
  let mut db = editor.db();
  db.set_resource_sharing(id, Some(ResourceSharing::Shared(TaskRevision::FIRST)))
    .expect("the team's");
  db.cache_shared_version(
    id,
    &CachedVersion {
      number: saved.number().next().expect("v2"),
      published_at: Timestamp::now(),
      publisher: SharedPerson {
        id: UserId::generate(),
        name: "olha".to_owned(),
      },
      server_body: serde_json::to_value(&body).expect("the body"),
      hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
      references: Vec::new(),
      local_body: body.clone(),
    },
  )
  .expect("v2 as the server numbered it");
  drop(db);
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
