//! Profile transitions must leave a usable permission editor and no orphaned field errors.
use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepCommands},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKind},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};

use crate::pipelines_support::Editor;

fn profile(editor: &Editor, name: &str, commands: &[&str]) -> ResourceId {
  let mut db = editor.db();
  let connection = ConnectionId::generate();
  db.create_connection(
    &Connection::new(
      connection,
      Name::new(name).unwrap(),
      ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
      AgentEndpoint::Program(editor.window.data.dir.path().join("unused-claude")),
      Timestamp::now(),
    )
    .unwrap(),
  )
  .unwrap();
  let commands = AllowedCommands::new(commands.iter().map(|s| CommandPrefix::new(s).unwrap()).collect()).unwrap();
  let profile = AgentProfile::new(
    Name::new(name).unwrap(),
    "".into(),
    PromptText::new("").unwrap(),
    connection,
    ModelId::new("test-model").unwrap(),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(commands),
    NetworkAccess::On,
  ));
  let id = db
    .create_resource(
      Slug::from_name(name).unwrap(),
      Name::new(name).unwrap(),
      &profile,
      Timestamp::now(),
    )
    .unwrap()
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now()).unwrap();
  id
}

fn changed_profile() -> (Editor, ResourceId) {
  let mut editor = Editor::new();
  profile(&editor, "Profile A", &["cargo check", "cargo test"]);
  profile(&editor, "Profile B", &["cargo test"]);
  let id = editor.empty("Permission transitions");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Profile A");
  editor.click("step-command-1");
  editor.until("narrowed draft saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .first()
      .is_some_and(|step| {
        matches!(&step.kind, StepKind::Agent { permissions, .. }
        if matches!(&permissions.commands, StepCommands::Only(only)
          if only.entries().len() == 1 && only.entries()[0].as_str() == "cargo check"))
      })
  });
  editor.choose("step-executor", "Profile B");
  assert!(editor.exists("step-executor-error"));
  (editor, id)
}

fn permission_error(editor: &mut Editor) {
  // The only offered command belongs to B; A's hidden cargo check prevents this edit.
  editor.click("step-command-0");
  assert!(editor.exists("pipeline-field-error"));
}

#[test]
fn pipelines_permissions_reset_recovers_obsolete_commands_after_profile_change() {
  let (mut editor, id) = changed_profile();
  permission_error(&mut editor);
  assert!(
    editor.exists("step-permissions-reset"),
    "obsolete restrictions need a reset path"
  );
  editor.click("step-permissions-reset");
  assert!(!editor.exists("step-executor-error"));
  assert!(!editor.exists("pipeline-field-error"));
  editor.click("pipeline-save");
  editor.until("reset permissions saved", |e| {
    matches!(&e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0].kind,
      StepKind::Agent { permissions, .. } if permissions.commands == StepCommands::AsProfile)
  });
  editor.click("step-command-0");
  editor.until("new profile can be narrowed", |e| {
    matches!(&e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0].kind,
      StepKind::Agent { permissions, .. }
        if matches!(&permissions.commands, StepCommands::Only(only) if only.is_empty()))
  });
}

#[test]
fn pipelines_permissions_removing_step_clears_its_error_and_allows_autosave() {
  let (mut editor, id) = changed_profile();
  permission_error(&mut editor);
  editor.click("step-remove");
  editor.click("step-remove-confirm");
  assert!(
    !editor.exists("pipeline-field-error"),
    "removed step must not block saving"
  );
  editor.until("removal autosaved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps().is_empty()
  });
}

#[test]
fn pipelines_permissions_replacing_executor_clears_only_its_permission_error() {
  let (mut editor, id) = changed_profile();
  permission_error(&mut editor);
  editor.set("step-budget", "invalid");
  editor.choose("step-executor", "Profile A");
  assert!(!editor.exists("step-executor-error"));
  assert!(editor.exists("pipeline-field-error"), "the invalid budget must remain");
  editor.set("step-budget", "100");
  assert!(
    !editor.exists("pipeline-field-error"),
    "the old permission error is obsolete"
  );
  editor.click("pipeline-save");
  editor.until("replacement saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0]
      .budget
      .is_some()
  });
}

#[test]
fn pipelines_permissions_removing_another_step_does_not_clear_the_invalid_step() {
  let (mut editor, id) = changed_profile();
  permission_error(&mut editor);
  editor.click("pipeline-add-step");
  editor.click("step-remove");
  editor.click("step-remove-confirm");
  assert!(
    editor.exists("pipeline-field-error"),
    "the first step still has an error"
  );
  editor.click("pipeline-step-step");
  editor.click("step-permissions-reset");
  assert!(!editor.exists("pipeline-field-error"));
  editor.until("remaining step autosaved", |e| {
    let draft = e.db().draft::<Pipeline>(id).unwrap().unwrap();
    draft.body().steps().len() == 1
      && matches!(&draft.body().steps()[0].kind,
        StepKind::Agent { permissions, .. } if permissions.commands == StepCommands::AsProfile)
  });
}

#[test]
fn pipelines_permissions_reset_keeps_approvals_and_protects_weaker_publication() {
  let (mut editor, id) = changed_profile();
  editor.choose("step-executor", "Profile A");
  editor.click("step-ask-tools");
  editor.until("approval retained in draft", |e| {
    matches!(&e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0].kind,
      StepKind::Agent { permissions, .. } if permissions.interactive_tool_approvals)
  });
  editor.db().publish_draft::<Pipeline>(id, Timestamp::now()).unwrap();
  editor.go(orchester_app::routes::ProjectPage::Pipelines);
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("step-permissions-reset");
  editor.until("reset autosaved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_some_and(|draft| {
      matches!(&draft.body().steps()[0].kind, StepKind::Agent { permissions, .. }
        if permissions.commands == StepCommands::AsProfile && permissions.interactive_tool_approvals)
    })
  });
  editor.click("pipeline-publish");
  editor.until("weaker publication question", |e| {
    e.exists("pipeline-publish-question-confirm")
  });
  assert!(editor.gated());
  assert_eq!(
    editor
      .db()
      .latest_version::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .number()
      .get(),
    1
  );
  editor.click("pipeline-publish-question-confirm");
  editor.until("confirmed publication", |e| {
    e.db().latest_version::<Pipeline>(id).unwrap().unwrap().number().get() == 2
  });
}

#[test]
fn pipelines_permissions_tool_replacement_clears_error_but_keeps_save_approval() {
  let mut editor = Editor::new();
  editor.published(); // Supplies the published Checks tool.
  profile(&editor, "Profile A", &["cargo check", "cargo test"]);
  profile(&editor, "Profile B", &["cargo test"]);
  let id = editor.empty("Tool replacement");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Profile A");
  editor.click("step-command-1");
  editor.choose("step-executor", "Profile B");
  permission_error(&mut editor);
  editor.choose("step-executor", "Checks");
  assert!(!editor.exists("pipeline-field-error"));
  assert!(!editor.exists("step-executor-error"));
  editor.click("pipeline-save");
  editor.until("tool save question", |e| e.exists("pipeline-save-question-confirm"));
  assert!(editor.gated());
  editor.click("pipeline-save-question-confirm");
  editor.until("tool saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .first()
      .is_some_and(|step| matches!(step.kind, StepKind::Tool { tool: Some(_) }))
  });
}
