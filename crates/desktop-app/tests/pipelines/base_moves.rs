//! *Merge the base branch when it moves* (ORC-65) in a writing step's inspector: on by the default rule for a step
//! that implements, it keeps a step's own choice in the draft, and turning it back to what the rule says drops the
//! choice so the step follows the rule again.
use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKind},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};

use crate::pipelines_support::Editor;

/// A published profile `Builder` that may write.
fn builder(editor: &Editor) {
  let mut db = editor.db();
  let connection = ConnectionId::generate();
  db.create_connection(
    &Connection::new(
      connection,
      Name::new("Builder").expect("valid fixture name"),
      ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
      AgentEndpoint::Program(editor.window.data.dir.path().join("unused-claude")),
      Timestamp::now(),
    )
    .expect("valid fixture connection"),
  )
  .expect("create fixture connection");
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid fixture name"),
    "".into(),
    PromptText::new("").expect("empty fixture prompt is valid"),
    connection,
    ModelId::new("test-model").expect("valid fixture model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  let id = db
    .create_resource(
      Slug::new("builder").expect("fixture slug"),
      Name::new("Builder").expect("valid fixture name"),
      &profile,
      Timestamp::now(),
    )
    .expect("create fixture profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("publish fixture profile");
}

/// The first step's own choice in `id`'s draft, once its repository access is `write`.
fn choice(editor: &Editor, id: ResourceId) -> Option<Option<bool>> {
  let draft = editor.db().draft::<Pipeline>(id).ok()??;
  match &draft.body().steps().first()?.kind {
    StepKind::Agent {
      repository: RepositoryAccess::Write,
      permissions,
      ..
    } => Some(permissions.takes_base_moves),
    _ => None,
  }
}

#[test]
fn pipelines_base_moves_a_writing_steps_choice_is_kept_and_dropped_back_to_the_default() {
  let mut editor = Editor::new();
  builder(&editor);
  let id = editor.empty("Base moves");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Builder");
  assert!(!editor.exists("step-base-moves"), "a read-only step merges nothing");
  editor.click("step-write");
  editor.until("the writing step saved", |e| choice(e, id) == Some(None));
  assert!(editor.exists("step-base-moves-note"));

  // On by the rule: the step writes and checks no one's work. Turning it off is the step's own choice.
  editor.click("step-base-moves");
  editor.until("the choice saved", |e| choice(e, id) == Some(Some(false)));
  editor.click("step-base-moves");
  editor.until("back to the rule", |e| choice(e, id) == Some(None));
}
