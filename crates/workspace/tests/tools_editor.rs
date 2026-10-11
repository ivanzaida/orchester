//! Editor operations use the command resolver and widening rules, and reject an obsolete read.
mod support;
use orchester_core::{
  name::Name,
  resource::{Tool, ToolAction},
  store::ResourceStore,
};
use orchester_workspace::{
  connections::mcp::Allowed,
  tools::{
    editor::{self, Edit, EditorData},
    form::CommandText,
  },
};
use support::Fixture;
fn fields() -> CommandText {
  CommandText {
    program: std::env::current_exe()
      .expect("test binary")
      .to_string_lossy()
      .into_owned(),
    ..CommandText::new_tool()
  }
}
fn create(f: &Fixture) -> orchester_core::id::ResourceId {
  editor::apply_in(
    f.repo.path(),
    Edit::Save {
      name: Name::new("Checks").expect("name"),
      tool: Box::new(fields().tool(None).expect("command")),
      before: None,
    },
    &Allowed::Anything,
  )
  .expect("saved")
}
#[test]
fn tools_editor_requires_review_for_creation_and_first_publication() {
  let f = Fixture::new();
  let edit = Edit::Save {
    name: Name::new("Checks").expect("name"),
    tool: Box::new(fields().tool(None).expect("tool")),
    before: None,
  };
  assert_eq!(edit.reasons(None).len(), 1);
  assert!(editor::apply_in(f.repo.path(), edit, &Allowed::These(vec![])).is_err());
  let id = create(&f);
  let data = EditorData::load_in(f.repo.path(), Some(id)).expect("read");
  let entry = data.listed.tool(id).expect("entry").clone();
  let publish = Edit::Publish(entry);
  assert!(editor::apply_in(f.repo.path(), publish.clone(), &Allowed::These(vec![])).is_err());
  editor::apply_in(f.repo.path(), publish, &Allowed::Anything).expect("published");
  assert!(f.db.latest_version::<Tool>(id).expect("version").is_some());
}
#[test]
fn tools_editor_rejects_stale_reads_and_duplicate_names() {
  let f = Fixture::new();
  let id = create(&f);
  let data = EditorData::load_in(f.repo.path(), Some(id)).expect("read");
  let entry = data.listed.tool(id).expect("entry").clone();
  let mut changed = fields();
  changed.timeout = "60".into();
  let edit = Edit::Save {
    name: Name::new("Checks").expect("name"),
    tool: Box::new(changed.tool(None).expect("tool")),
    before: Some(entry.clone()),
  };
  editor::apply_in(f.repo.path(), edit, &Allowed::Anything).expect("saved change");
  assert!(editor::apply_in(f.repo.path(), Edit::Publish(entry), &Allowed::Anything).is_err());
  let data = EditorData::load_in(f.repo.path(), Some(id)).expect("read");
  assert!(editor::new_name(&data.listed, "checks").is_err());
}
#[test]
fn tools_editor_program_is_resolved_outside_repository() {
  let f = Fixture::new();
  let mut text = fields();
  text.program = "cargo".into();
  let fixed = editor::prepare_in(f.repo.path(), &text.tool(None).expect("tool")).expect("search path program");
  let ToolAction::Command(command) = fixed.action() else {
    panic!("command")
  };
  assert!(std::path::Path::new(command.program().as_str()).is_absolute());
  let path = f.repo.path().join("program");
  std::fs::write(&path, "fixture").expect("file");
  text.program = path.to_string_lossy().into_owned();
  assert!(editor::prepare_in(f.repo.path(), &text.tool(None).expect("tool")).is_err());
  text.program = "scripts/program".into();
  assert!(editor::prepare_in(f.repo.path(), &text.tool(None).expect("tool")).is_err());
}
#[test]
fn tools_editor_reviews_arguments_codes_and_variables_but_allows_harmless_changes() {
  let f = Fixture::new();
  let id = create(&f);
  let entry = EditorData::load_in(f.repo.path(), Some(id))
    .expect("data")
    .listed
    .tool(id)
    .expect("entry")
    .clone();
  let save = |text: CommandText| Edit::Save {
    name: Name::new("Checks").expect("name"),
    tool: Box::new(text.tool(None).expect("tool")),
    before: Some(entry.clone()),
  };
  let mut harmless = fields();
  harmless.timeout = "60".into();
  harmless.environment = "CI=true".into();
  assert!(save(harmless).reasons(None).is_empty());
  let mut changed = fields();
  changed.arguments = "--other".into();
  assert!(!save(changed).reasons(None).is_empty());
  let mut changed = fields();
  changed.exit_codes = "0, 1".into();
  assert!(!save(changed).reasons(None).is_empty());
  let mut changed = fields();
  changed.environment = "PATH=/external".into();
  assert!(!save(changed).reasons(None).is_empty());
}
