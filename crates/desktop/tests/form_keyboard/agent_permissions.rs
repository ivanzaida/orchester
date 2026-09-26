//! Agent permissions in the profile editor's Permissions tab and in the step inspector: options a
//! connection's CLI cannot enforce are disabled with the reason and are no Tab stops, the tabs are
//! pointer-only, a profile whose permissions cannot run is not publishable, and a step narrows its
//! profile's allowed commands.

use std::path::Path;

use lurq::node::element::ElementRef;
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    StepPermissions,
  },
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug, Step, StepKey, StepKind},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::harness::Desktop;

/// More than the editor has stops.
const MAX_STOPS: usize = 40;

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn commands(values: &[&str]) -> AllowedCommands {
  AllowedCommands::new(
    values
      .iter()
      .map(|value| CommandPrefix::new(value).expect("command"))
      .collect(),
  )
  .expect("list")
}

fn connection(db: &mut ProjectDb, label: &str, provider: AgentProvider, repo: &Path) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    provider,
    repo.join("cli.exe"),
    Timestamp::now(),
  )
  .expect("absolute executable");
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: "1.0.0".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(
        CatalogSource::Provider,
        vec![CatalogModel::new(ModelId::new("model").expect("model"), Vec::new())],
      ),
    },
  });
  let id = connection.id();
  db.create_connection(&connection.into_connection()).expect("connection");
  id
}

/// A profile `role` on `connection`, published when `publish`, else a draft only.
fn profile(
  db: &mut ProjectDb,
  role: &str,
  connection: ConnectionId,
  permissions: AgentPermissions,
  publish: bool,
) -> ResourceId {
  let body = AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("Work.").expect("prompt"),
    connection,
    ModelId::new("model").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions);
  let slug = Slug::new(&role.to_lowercase()).expect("slug");
  let id = db
    .create_resource(slug, name(role), &body, Timestamp::now())
    .expect("profile")
    .id();
  if publish {
    db.publish_draft::<AgentProfile>(id, Timestamp::now()).expect("publish");
  }
  id
}

/// A project with a Codex and a Claude connection, a Builder on Codex, a published Reviewer on
/// Claude allowing three commands, a Scout draft on Claude with commands it cannot confine, and a
/// pipeline whose review step keeps two of the Reviewer's commands.
fn seeded() -> Desktop {
  let desktop = Desktop::with_project();
  let mut db = ProjectDb::open(desktop.repo()).expect("project database");
  let codex = connection(&mut db, "Codex", AgentProvider::CodexCli, desktop.repo());
  let claude = connection(&mut db, "Claude", AgentProvider::ClaudeCodeCli, desktop.repo());
  let any = CommandPermission::AnyInWorktree;
  profile(
    &mut db,
    "Builder",
    codex,
    AgentPermissions::new(RepositoryAccess::Write, any.clone(), NetworkAccess::On),
    true,
  );
  let listed = CommandPermission::Allowed(commands(&["git", "pnpm test", "pnpm lint"]));
  let reviewer = profile(
    &mut db,
    "Reviewer",
    claude,
    AgentPermissions::new(RepositoryAccess::ReadOnly, listed, NetworkAccess::On),
    true,
  );
  profile(
    &mut db,
    "Scout",
    claude,
    AgentPermissions::new(RepositoryAccess::ReadOnly, any, NetworkAccess::On),
    false,
  );
  let review = Step {
    key: StepKey::new("review").expect("key"),
    name: name("Review"),
    kind: StepKind::Agent {
      profile: Some(reviewer),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions {
        commands: StepCommands::Only(commands(&["git", "pnpm test"])),
        ..StepPermissions::default()
      },
    },
    instructions: PromptText::new("Review.").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![review]).expect("pipeline");
  db.create_resource(
    Slug::new("delivery").expect("slug"),
    name("Delivery"),
    &pipeline,
    Timestamp::now(),
  )
  .expect("pipeline");
  desktop
}

fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| contains_text(child, text))
}

fn shows(desktop: &mut Desktop, text: &str) -> bool {
  desktop.tree.root().is_some_and(|root| contains_text(root, text))
}

/// The agents screen with `role` in the editor and its Permissions tab open.
fn permissions_of(desktop: &mut Desktop, role: &str) {
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  desktop.wait_for("agent-role");
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  let id = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == role)
    .expect("profile")
    .id();
  let item = format!("agent-{id}");
  desktop.press(&item);
  desktop.settle("the profile in the editor", |tree| {
    tree
      .get_element_by_id_mut("agent-role")
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == role)
  });
  desktop.press("agent-tab-permissions");
  desktop.wait_for("agent-enforcement");
}

#[test]
fn agent_permissions_disable_what_the_connection_cannot_enforce_with_the_reason() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Builder");
  assert!(shows(&mut desktop, "Enforced by Codex CLI"));
  assert!(shows(
    &mut desktop,
    "Unavailable on this connection: Codex CLI can’t limit commands to a list."
  ));
  assert!(shows(&mut desktop, "Ask before running commands · later"));
  let network_off_enforced = cfg!(not(windows));
  assert_eq!(
    shows(&mut desktop, "Off is unavailable on this connection: Codex CLI"),
    !network_off_enforced
  );
  // Only options that can be chosen are form controls; the tabs are pointer-only.
  let stops = desktop.stops(MAX_STOPS, false);
  for option in [
    "agent-permission-read-only",
    "agent-permission-edit",
    "agent-permission-any",
  ] {
    assert!(stops.contains(&Some(option.to_owned())), "{option} is a Tab stop");
  }
  for unavailable in [
    "agent-permission-ask",
    "agent-permission-allowed",
    "agent-tab-profile",
    "agent-tab-permissions",
    "agent-tab-usage",
  ] {
    assert!(
      !stops.contains(&Some(unavailable.to_owned())),
      "{unavailable} is no Tab stop"
    );
  }
  assert_eq!(stops.contains(&Some("agent-network".to_owned())), network_off_enforced);
}

#[test]
fn agent_permissions_choose_an_option_and_add_a_command() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Reviewer");
  assert!(shows(
    &mut desktop,
    "Unavailable on this connection: Claude Code CLI can’t confine commands to the worktree."
  ));
  desktop.tab_to("agent-new-command", MAX_STOPS);
  desktop.type_text("rg");
  // Enter in the command field adds the command; it does not save the draft.
  desktop.enter();
  desktop.wait_for("agent-command-3");
  assert_eq!(desktop.value("agent-new-command"), "");
  assert!(shows(&mut desktop, "Unsaved changes"));
  desktop.type_text("git; rm");
  desktop.enter();
  assert!(shows(&mut desktop, "A command cannot contain ;"));
  desktop.press("agent-permission-edit");
  assert!(shows(
    &mut desktop,
    "Edits files in the task worktree · 4 allowed commands · Network on"
  ));
}

#[test]
fn agent_permissions_the_connection_cannot_enforce_block_publishing() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Scout");
  desktop.wait_for("agent-permissions-blocked");
  assert!(shows(
    &mut desktop,
    "Claude Code CLI cannot enforce Any command in the worktree. Choose another option."
  ));
  // Publish is disabled: pressing it publishes nothing.
  desktop.press("agent-publish");
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  let scout = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Scout")
    .expect("scout")
    .id();
  assert!(db.latest_version::<AgentProfile>(scout).expect("read").is_none());
  // Choosing what the connection enforces clears it.
  desktop.press("agent-permission-allowed");
  desktop.settle("no blocked notice", |tree| {
    tree.get_element_by_id("agent-permissions-blocked").is_none()
  });
}

#[test]
fn step_inspector_narrows_the_profile_commands_and_never_widens() {
  let mut desktop = seeded();
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.press("pipeline-row");
  desktop.wait_for("step-card-review");
  desktop.press("step-card-review");
  desktop.wait_for("step-command-2");
  assert!(shows(&mut desktop, "Commands · narrowed"));
  assert!(shows(&mut desktop, "Reviewer v1 also allows pnpm lint."));
  // A read-only profile offers no repository choice, and a step cannot add commands of its own.
  assert!(desktop.tree.get_element_by_id("step-repository").is_none());
  assert!(desktop.tree.get_element_by_id("step-command-3").is_none());
  // The entries sit below the inspector's scrolled view; a click reaches them as the pointer would
  // after scrolling.
  desktop.click("step-command-2");
  desktop.settle("the step back at the profile's commands", |tree| {
    tree
      .root()
      .is_some_and(|root| !contains_text(root, "Commands · narrowed"))
  });
  desktop.click("step-command-0");
  desktop.settle("git left out", |tree| {
    tree
      .root()
      .is_some_and(|root| contains_text(root, "Reviewer v1 also allows git."))
  });
}
