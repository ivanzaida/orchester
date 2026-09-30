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
pub(crate) const MAX_STOPS: usize = 40;

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
/// Claude allowing three commands, a Scout draft on Claude with commands it cannot confine, an
/// Explorer draft on Claude with network off, which Claude cannot keep, and a pipeline whose review
/// step keeps two of the Reviewer's commands.
pub(crate) fn seeded() -> Desktop {
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
  profile(
    &mut db,
    "Explorer",
    claude,
    AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(commands(&["git"])),
      NetworkAccess::Off,
    ),
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

pub(crate) fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| contains_text(child, text))
}

pub(crate) fn shows(desktop: &mut Desktop, text: &str) -> bool {
  desktop.tree.root().is_some_and(|root| contains_text(root, text))
}

/// The agents screen with `role` in the editor and its Permissions tab open.
pub(crate) fn permissions_of(desktop: &mut Desktop, role: &str) {
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
  assert!(shows(&mut desktop, "Ask before running commands"));
  let network_off_enforced = cfg!(not(windows));
  assert_eq!(
    shows(
      &mut desktop,
      concat!(
        "Off is unavailable on this connection: Codex CLI can’t block network use by commands, ",
        "so network stays on. The run summary says so."
      )
    ),
    !network_off_enforced
  );
  // The panel says what the switch shows: network on, and whether off could be enforced.
  assert_eq!(shows(&mut desktop, "Network on · enforced"), network_off_enforced);
  assert_eq!(
    shows(&mut desktop, "Network stays on · off is not enforceable"),
    !network_off_enforced
  );
  // Only options that can be chosen are form controls; the tabs are pointer-only.
  let stops = desktop.stops(MAX_STOPS, false);
  for option in [
    "agent-permission-read-only",
    "agent-permission-edit",
    "agent-permission-ask",
    "agent-permission-any",
  ] {
    assert!(stops.contains(&Some(option.to_owned())), "{option} is a Tab stop");
  }
  for unavailable in [
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

/// Whether the knob of the switch `id` sits at the right of its track, where it is on.
fn switched_on(desktop: &mut Desktop, id: &str) -> bool {
  let switch = desktop.bounds(id);
  let knob = desktop.bounds(&format!("{id}-knob"));
  knob.x + knob.width / 2.0 > switch.x + TRACK_WIDTH / 2.0
}

/// The switch's track width (`K8ItM`).
const TRACK_WIDTH: f32 = 28.0;

#[test]
fn agent_permissions_locked_network_switch_shows_the_network_a_run_gets() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Explorer");
  // Explorer chose network off; Claude cannot keep commands off the network, so runs have it on.
  assert!(shows(
    &mut desktop,
    concat!(
      "Off is unavailable on this connection: Claude Code CLI can’t block network use by commands, ",
      "so network stays on. The run summary says so."
    )
  ));
  // The panel agrees with the switch and says how the run gets network: without the web tools.
  assert!(shows(&mut desktop, "Network stays on · off is not enforceable"));
  assert!(shows(&mut desktop, "web tools removed · commands keep network"));
  assert!(!shows(&mut desktop, "Network off"));
  assert!(switched_on(&mut desktop, "agent-network"));
  let label = desktop
    .tree
    .get_element_by_id("agent-network")
    .is_some_and(|switch| contains_text(switch, "On") && !contains_text(switch, "Off"));
  assert!(label, "the switch's label says On");
  // Locked: no Tab stop, and a press changes nothing.
  assert!(
    !desktop
      .stops(MAX_STOPS, false)
      .contains(&Some("agent-network".to_owned()))
  );
  desktop.press("agent-network");
  assert!(switched_on(&mut desktop, "agent-network"));
  assert!(!shows(&mut desktop, "Unsaved changes"));
}

#[test]
fn agent_permissions_choose_an_option_and_add_a_command() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Reviewer");
  assert!(shows(
    &mut desktop,
    "Unavailable on this connection: Claude Code CLI can’t confine commands to the worktree."
  ));
  // Network on was chosen, but off could not be enforced either: the panel does not call it enforced.
  assert!(shows(&mut desktop, "Network stays on · off is not enforceable"));
  assert!(shows(&mut desktop, "web tools allowed"));
  assert!(!shows(&mut desktop, "Network on · enforced"));
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

#[test]
fn agent_permissions_claude_asking_read_only_runs_no_commands_and_says_why() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Reviewer");
  assert!(shows(&mut desktop, "edit tools removed · listed cmds run"));
  // Read-only and asking: an approved command would run unsandboxed, so the step gets none.
  desktop.press("agent-permission-ask");
  assert!(shows(&mut desktop, "read-only · no commands"));
  assert!(shows(
    &mut desktop,
    "Claude Code would run an approved command outside any sandbox, so a read-only step gets none."
  ));
  assert!(shows(&mut desktop, "edit tools removed · no commands"));
  assert!(!shows(&mut desktop, "listed cmds run"));
  desktop.press("agent-permission-edit");
  assert!(shows(&mut desktop, "--permission-prompt-tool · asks each command"));
  assert!(!shows(&mut desktop, "outside any sandbox"));
}
