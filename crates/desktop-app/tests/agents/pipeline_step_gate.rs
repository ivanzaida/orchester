//! *Gate and access* of a step in the hub's panel (`Pipelines · v2` P2b `EGEos`, P2f `Tv9hK`; PV2-18): *Merge the base
//! branch when it moves* for a step that can write, and the step's permissions in place — the commands its profile
//! allows, *Turn network off* locked where the profile cannot enforce it, *Ask when a tool needs approval* and *Use
//! profile commands and network* — each changing the edit until *Save*; turning *Ask* off asks before saving, and while
//! that question waits nothing in the step changes. `cargo test -p orchester-app --test agents pipeline_step_gate`.

use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepCommands, StepPermissions,
  },
  resource::{
    AgentProfile, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKey,
    StepKind, takes_base_moves,
  },
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_step::{SAVE, open_step, rect, route, save_until, stored},
};

/// A published profile `name` on a Claude Code connection, which enforces a list of commands but cannot turn the
/// network off on any platform; it may write when `repository` says so.
pub(super) fn claude(agents: &Agents, name: &str, repository: RepositoryAccess, commands: &[&str]) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let connection = ConnectionId::generate();
  db.create_connection(
    &Connection::new(
      connection,
      Name::new(name).expect("name"),
      ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
      AgentEndpoint::Program(agents.root.join("unused-claude")),
      Timestamp::now(),
    )
    .expect("connection"),
  )
  .expect("stored connection");
  let commands = AllowedCommands::new(
    commands
      .iter()
      .map(|command| CommandPrefix::new(command).expect("command"))
      .collect(),
  )
  .expect("commands");
  let profile = AgentProfile::new(
    Name::new(name).expect("name"),
    "".into(),
    PromptText::new("").expect("instructions"),
    connection,
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    repository,
    CommandPermission::Allowed(commands),
    NetworkAccess::On,
  ));
  let id = db
    .create_resource(
      Slug::from_name(name).expect("slug"),
      Name::new(name).expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  id
}

/// A published pipeline with one step *Build* that runs `profile` with `repository` and `permissions`.
pub(super) fn one_step(
  agents: &Agents,
  profile: ResourceId,
  repository: RepositoryAccess,
  permissions: StepPermissions,
) -> (ResourceId, StepKey) {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let key = edit.add_step(Name::new("Build").expect("name"), None);
  edit.update(&key, |step| {
    step.kind = StepKind::Agent {
      profile: Some(profile),
      repository,
      model: None,
      permissions,
    };
  });
  let body = edit.validate().expect("a pipeline");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_published(
      Slug::from_name(&format!("Gate {repository:?}")).expect("slug"),
      Name::new(&format!("Gate {repository:?}")).expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the pipeline")
    .0
    .id();
  (id, key)
}

/// The narrowing step `key` of `pipeline` has as stored.
fn narrowing(agents: &Agents, pipeline: ResourceId, key: &StepKey) -> StepPermissions {
  match &stored(agents, pipeline).step(key).expect("the step").kind {
    StepKind::Agent { permissions, .. } => permissions.clone(),
    StepKind::Tool { .. } => panic!("an agent step"),
  }
}

fn only(commands: &[&str]) -> StepCommands {
  StepCommands::Only(
    AllowedCommands::new(
      commands
        .iter()
        .map(|command| CommandPrefix::new(command).expect("command"))
        .collect(),
    )
    .expect("commands"),
  )
}

/// Where the bottom of `id` is laid out.
fn bottom(agents: &mut Agents, id: &str) -> f32 {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  bounds.y + bounds.height
}

/// How tall `id` is laid out.
fn tall(agents: &mut Agents, id: &str) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .height
}

/// The panel's question before a save that widens what the pipeline's agents may do.
const CONFIRM: &str = "pipeline-hub-confirm-confirm";
const HINT: &str = "team-hub-actions-hint";

#[test]
fn a_writing_step_keeps_its_own_base_branch_choice_and_drops_it_back_to_the_rule() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  // Both before the hub reads the project's pipelines.
  let (reading, read_key) = one_step(&agents, builder, RepositoryAccess::ReadOnly, StepPermissions::default());
  let (id, key) = one_step(&agents, builder, RepositoryAccess::Write, StepPermissions::default());
  open_step(&mut agents, reading, &read_key);
  assert!(
    !agents.exists("pipeline-hub-step-base-moves-row"),
    "a read-only step merges nothing"
  );

  open_step(&mut agents, id, &key);
  assert_eq!(
    agents.text("pipeline-hub-step-base-moves-row-label"),
    "Merge the base branch when it moves"
  );
  assert!(
    agents.exists("pipeline-hub-step-base-moves-info"),
    "the (i) says the rule"
  );
  let by_rule = takes_base_moves(stored(&agents, id).steps(), &key);

  agents.click("pipeline-hub-step-base-moves");
  save_until(&mut agents, "the step's own choice", |s| {
    narrowing(s, id, &key).takes_base_moves == Some(!by_rule)
  });
  // The panel lists what was stored before the next change: one made between the two would be put back with it.
  agents.until("the panel lists v2", |s| s.text(HINT).contains("v3"));
  agents.click("pipeline-hub-step-base-moves");
  save_until(&mut agents, "back to the rule", |s| {
    narrowing(s, id, &key).takes_base_moves.is_none()
  });
}

#[test]
fn a_step_narrows_the_profile_s_commands_and_takes_them_back_keeping_ask() {
  let mut agents = Agents::new(true);
  let builder = claude(
    &agents,
    "Builder",
    RepositoryAccess::ReadOnly,
    &["cargo check", "cargo test"],
  );
  let (id, key) = one_step(&agents, builder, RepositoryAccess::ReadOnly, StepPermissions::default());
  open_step(&mut agents, id, &key);
  assert_eq!(agents.text("pipeline-hub-step-commands-row-label"), "Commands");
  assert_eq!(
    agents.text("pipeline-hub-step-commands-row-sub"),
    "A step can only narrow what Builder allows"
  );
  assert!(agents.text("pipeline-hub-step-command-0").contains("cargo check"));
  assert!(agents.text("pipeline-hub-step-command-1").contains("cargo test"));
  assert_eq!(agents.text("pipeline-hub-step-network-row-label"), "Turn network off");
  assert_eq!(
    agents.text("pipeline-hub-step-ask-tools-row-label"),
    "Ask when a tool needs approval"
  );
  assert!(
    !agents.exists("pipeline-hub-step-permissions-reset"),
    "nothing to take back while the step narrows nothing"
  );

  let (network, ask) = (
    rect(&mut agents, "pipeline-hub-step-network-off"),
    rect(&mut agents, "pipeline-hub-step-ask-tools"),
  );
  assert!(
    (network.x - ask.x).abs() < 0.5,
    "the locked switch stands at the end as the others do: {network:?} {ask:?}"
  );
  let (label, row) = (
    rect(&mut agents, "pipeline-hub-step-ask-tools-row-label"),
    rect(&mut agents, "pipeline-hub-step-network-row-label"),
  );
  assert!(
    tall(&mut agents, "pipeline-hub-step-ask-tools-row") <= tall(&mut agents, "pipeline-hub-step-network-row") + 0.5,
    "Ask keeps one line: {label:?} {row:?}"
  );

  // Claude Code cannot turn the network off: the switch is locked and changes nothing.
  agents.click("pipeline-hub-step-network-off");
  assert!(
    !agents.text(HINT).starts_with("Unsaved"),
    "the locked switch made no change"
  );

  agents.click("pipeline-hub-step-command-1");
  agents.click("pipeline-hub-step-ask-tools");
  agents.until("the reset shows", |s| s.exists("pipeline-hub-step-permissions-reset"));
  save_until(&mut agents, "the narrowing stored", |s| {
    let stored = narrowing(s, id, &key);
    stored.commands == only(&["cargo check"]) && stored.interactive_tool_approvals
  });

  // The panel lists what was stored before the next change: one made between the two would be put back with it.
  agents.until("the panel lists v2", |s| s.text(HINT).contains("v3"));
  agents.click("pipeline-hub-step-permissions-reset");
  agents.until("nothing to take back", |s| {
    !s.exists("pipeline-hub-step-permissions-reset")
  });
  save_until(&mut agents, "the profile's commands again, still asking", |s| {
    let stored = narrowing(s, id, &key);
    stored.commands == StepCommands::AsProfile && stored.interactive_tool_approvals
  });
}

#[test]
fn turning_ask_off_asks_before_saving_and_the_step_waits_unchanged() {
  let mut agents = Agents::new(true);
  let builder = claude(
    &agents,
    "Builder",
    RepositoryAccess::ReadOnly,
    &["cargo check", "cargo test"],
  );
  let asking = StepPermissions {
    interactive_tool_approvals: true,
    ..StepPermissions::default()
  };
  let (id, key) = one_step(&agents, builder, RepositoryAccess::ReadOnly, asking);
  open_step(&mut agents, id, &key);
  agents.click("pipeline-hub-step-ask-tools");
  agents.press_until(SAVE, "the question", |s| s.exists(CONFIRM));
  assert!(
    agents
      .text("pipeline-hub-confirm")
      .contains("Step Build would no longer ask a person about its tool calls."),
    "{}",
    agents.text("pipeline-hub-confirm")
  );

  // The question stands over the end of the panel; the rows it asks about are brought into view with it.
  agents.until("the Ask row in view", |s| {
    let (row, view) = (
      bottom(s, "pipeline-hub-step-ask-tools-row"),
      bottom(s, "pipeline-hub-panel-scroll"),
    );
    row <= view + 0.5
  });

  // Open Builder does not leave while the question waits: it asks about leaving, as any way out of the panel does, and
  // *Keep editing* returns to the question.
  let at = route(&agents);
  agents.click("pipeline-hub-step-open-profile");
  agents.until("the question about leaving", |s| s.exists("pipeline-hub-leave-cancel"));
  assert_eq!(route(&agents), at, "nothing was left");
  agents.click("pipeline-hub-leave-cancel");
  agents.until("back to the question", |s| {
    !s.exists("pipeline-hub-leave-cancel") && s.exists(CONFIRM)
  });
  assert_eq!(route(&agents), at);

  // P2f: while the question waits the step is locked; its controls keep their place and take no press.
  for id in [
    "pipeline-hub-step-approval",
    "pipeline-hub-step-command-0",
    "pipeline-hub-step-ask-tools",
    "pipeline-hub-step-name-edit",
  ] {
    assert!(agents.exists(id), "{id} keeps its place");
    agents.click(id);
  }
  assert!(
    agents
      .window
      .tree
      .get_element_by_id_mut("pipeline-hub-step-name")
      .and_then(|element| element.as_text_input())
      .is_none(),
    "the name reads, it is not edited"
  );

  agents.press_until(CONFIRM, "saved", |s| !narrowing(s, id, &key).interactive_tool_approvals);
  let stored = stored(&agents, id);
  let step = stored.step(&key).expect("the step");
  assert!(!step.approval_gate, "the presses while locked changed nothing");
  assert_eq!(narrowing(&agents, id, &key).commands, StepCommands::AsProfile);
}

#[test]
fn a_command_the_profile_no_longer_allows_is_said_refused_and_taken_back() {
  let mut agents = Agents::new(true);
  let builder = claude(
    &agents,
    "Builder",
    RepositoryAccess::ReadOnly,
    &["cargo check", "cargo test"],
  );
  let narrowed = StepPermissions {
    commands: only(&["cargo build"]),
    ..StepPermissions::default()
  };
  let (id, key) = one_step(&agents, builder, RepositoryAccess::ReadOnly, narrowed);
  open_step(&mut agents, id, &key);
  assert!(
    agents.exists("pipeline-hub-step-permissions-error"),
    "the step says its narrowing is not the profile's"
  );
  assert!(
    agents
      .text("pipeline-hub-step-permissions-error")
      .contains("cargo build"),
    "{}",
    agents.text("pipeline-hub-step-permissions-error")
  );

  // A pipeline holds no draft (D-032): a narrowing the profile no longer allows cannot be saved, so Save is off and
  // says why, and pressing it stores nothing.
  agents.click("pipeline-hub-step-approval");
  agents.until("the bar says why", |s| s.text(HINT).contains("cargo build"));
  agents.click(SAVE);
  agents.window.pass();
  assert!(
    !agents.exists("pipeline-hub-refused"),
    "a Save that is off tries nothing"
  );
  let db = ProjectDb::open(&agents.root).expect("database");
  assert!(db.draft::<Pipeline>(id).expect("read").is_none(), "no draft is stored");
  assert_eq!(db.versions::<Pipeline>(id).expect("read").len(), 1, "nothing published");
  assert_eq!(
    narrowing(&agents, id, &key).commands,
    only(&["cargo build"]),
    "nothing published"
  );
  agents.until("the bar says why", |s| s.text(HINT).contains("cargo build"));

  agents.click("pipeline-hub-step-permissions-reset");
  agents.until("the error is gone", |s| {
    !s.exists("pipeline-hub-step-permissions-error")
  });
  save_until(&mut agents, "saved with the profile's commands", |s| {
    narrowing(s, id, &key).commands == StepCommands::AsProfile
  });
}
