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
      Name::new(name).expect("valid fixture name"),
      ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
      AgentEndpoint::Program(editor.window.data.dir.path().join("unused-claude")),
      Timestamp::now(),
    )
    .expect("valid fixture connection"),
  )
  .expect("create fixture connection");
  let commands = AllowedCommands::new(
    commands
      .iter()
      .map(|s| CommandPrefix::new(s).expect("valid fixture command prefix"))
      .collect(),
  )
  .expect("valid fixture command allowlist");
  let profile = AgentProfile::new(
    Name::new(name).expect("valid fixture name"),
    "".into(),
    PromptText::new("").expect("empty fixture prompt is valid"),
    connection,
    ModelId::new("test-model").expect("valid fixture model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(commands),
    NetworkAccess::On,
  ));
  let id = db
    .create_resource(
      Slug::from_name(name).expect("fixture name yields a slug"),
      Name::new(name).expect("valid fixture name"),
      &profile,
      Timestamp::now(),
    )
    .expect("create fixture profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("publish fixture profile");
  id
}

/// The step's Permissions page, from its fields.
fn permissions(editor: &mut Editor) {
  if editor.exists("step-open-permissions") {
    editor.click("step-open-permissions");
  }
}

/// The step's fields, from one of its pages.
fn fields(editor: &mut Editor) {
  if editor.exists("step-page-back") {
    editor.click("step-page-back");
  }
}

fn command(editor: &mut Editor, index: usize) {
  permissions(editor);
  editor.click("step-command-picker");
  editor.click(&format!("step-command-{index}"));
  editor.click("step-command-picker");
}

fn executor(editor: &mut Editor, name: &str) {
  fields(editor);
  editor.choose("step-executor", name);
}

fn changed_profile() -> (Editor, ResourceId) {
  let mut editor = Editor::new();
  editor.window.tree.resize(1440, 1800);
  profile(&editor, "Profile A", &["cargo check", "cargo test"]);
  profile(&editor, "Profile B", &["cargo test"]);
  let id = editor.empty("Permission transitions");
  editor.open(id);
  editor.click("pipeline-add-step");
  executor(&mut editor, "Profile A");
  command(&mut editor, 1);
  editor.until("narrowed draft saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .expect("read narrowed pipeline draft")
      .expect("pipeline draft exists after creation")
      .body()
      .steps()
      .first()
      .is_some_and(|step| {
        matches!(&step.kind, StepKind::Agent { permissions, .. }
        if matches!(&permissions.commands, StepCommands::Only(only)
          if only.entries().len() == 1 && only.entries()[0].as_str() == "cargo check"))
      })
  });
  executor(&mut editor, "Profile B");
  editor.until("Profile B cannot keep A's narrowing", |e| {
    e.exists("step-executor-error")
  });
  (editor, id)
}

fn permission_error(editor: &mut Editor) {
  // The only offered command belongs to B; A's hidden cargo check prevents this edit.
  command(editor, 0);
  assert!(editor.exists("pipeline-field-error"));
}

#[test]
fn pipelines_permissions_choices_open_from_compact_summary_and_preserve_enforcement() {
  let mut editor = Editor::new();
  profile(&editor, "Reviewer", &["cargo check", "cargo test", "cargo clippy"]);
  let id = editor.empty("Compact permissions");
  editor.open(id);
  editor.click("pipeline-add-step");
  executor(&mut editor, "Reviewer");
  assert!(editor.text("step-open-permissions").contains("From Reviewer"));
  permissions(&mut editor);
  assert!(!editor.exists("step-command-0"));
  assert_eq!(editor.text("step-command-picker-label"), "From the profile");
  command(&mut editor, 1);
  assert_eq!(editor.text("step-command-picker-label"), "Narrowed · 2 allowed");
  assert!(editor.text("step-page-title").contains("Narrower than Reviewer"));
  assert!(!editor.exists("step-command-0"));
  assert!(editor.text("step-commands-lead").contains("cargo clippy"));
  editor.until("commands saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .first()
      .is_some_and(|step| {
        matches!(&step.kind, StepKind::Agent { permissions, .. }
        if matches!(&permissions.commands, StepCommands::Only(only) if only.entries().len() == 2))
      })
  });
  let before = editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().clone();
  editor.click("step-network-off");
  // The fixture uses Claude Code CLI; NetworkOff is not offered on this platform.
  let choices = orchester_workspace::pipelines::data::Assignables::load(&editor.db()).unwrap();
  if !orchester_workspace::pipelines::narrowing::can_block_network(&choices.profiles[0]) {
    editor.arm();
    assert_eq!(editor.db().draft::<Pipeline>(id).unwrap().unwrap().body(), &before);
  }
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
  command(&mut editor, 0);
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
  fields(&mut editor);
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
  fields(&mut editor);
  editor.set("step-budget", "invalid");
  executor(&mut editor, "Profile A");
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
  permissions(&mut editor);
  editor.click("step-permissions-reset");
  assert!(!editor.exists("pipeline-field-error"));
  editor.until("remaining step autosaved", |e| {
    let draft = e.db().draft::<Pipeline>(id).unwrap().unwrap();
    draft.body().steps().len() == 1
      && matches!(&draft.body().steps()[0].kind,
        StepKind::Agent { permissions, .. } if permissions.commands == StepCommands::AsProfile)
  });
}

/// A step that asks about its agent's tool calls, published as v1, and its Permissions page open again.
fn asking_published() -> (Editor, ResourceId) {
  let (mut editor, id) = changed_profile();
  executor(&mut editor, "Profile A");
  permissions(&mut editor);
  editor.click("step-ask-tools");
  editor.until("approval retained in draft", |e| {
    let draft = e
      .db()
      .draft::<Pipeline>(id)
      .expect("the draft reads")
      .expect("the draft exists");
    matches!(&draft.body().steps()[0].kind,
      StepKind::Agent { permissions, .. } if permissions.interactive_tool_approvals)
  });
  editor
    .db()
    .publish_draft::<Pipeline>(id, Timestamp::now())
    .expect("the asking draft publishes");
  editor.go(orchester_app::routes::ProjectPage::Pipelines);
  editor.open(id);
  editor.click("pipeline-step-step");
  permissions(&mut editor);
  (editor, id)
}

#[test]
fn pipelines_permissions_turning_ask_off_asks_before_saving_with_its_reason() {
  let (mut editor, id) = asking_published();
  editor.click("step-ask-tools");
  editor.arm();
  assert!(
    editor.db().draft::<Pipeline>(id).unwrap().is_none(),
    "a weaker draft is not saved on its own"
  );
  editor.click("pipeline-save");
  editor.until("save question", |e| e.exists("pipeline-save-question-confirm"));
  assert!(editor.gated(), "only a person answers it");
  assert!(
    editor
      .text("pipeline-save-question")
      .contains("Saving needs your approval")
  );
  assert!(
    editor.exists("step-ask-tools"),
    "the question sits beside the Permissions page"
  );
  editor.click("pipeline-save-question-keep");
  assert!(!editor.exists("pipeline-save-question"));
  editor.click("pipeline-save");
  editor.until("asked again", |e| e.exists("pipeline-save-question-confirm"));
  editor.click("pipeline-save-question-confirm");
  editor.until("weaker draft saved after approval", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_some_and(|draft| {
      matches!(&draft.body().steps()[0].kind, StepKind::Agent { permissions, .. }
        if !permissions.interactive_tool_approvals)
    })
  });
}

#[test]
fn pipelines_permissions_reset_keeps_approvals_and_protects_weaker_publication() {
  let (mut editor, id) = asking_published();
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
  executor(&mut editor, "Profile A");
  command(&mut editor, 1);
  executor(&mut editor, "Profile B");
  permission_error(&mut editor);
  executor(&mut editor, "Checks");
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
