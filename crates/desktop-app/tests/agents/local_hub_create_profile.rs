//! A local project's new profile is made with the fields its panel edits (`Agents · v2` L3 `JEkCL`, L17 `b7Tbxb`): its
//! role instructions, and the project's instructions, MCP servers, tools and skills it gives its agents, picked in the
//! form as its panel picks them. `cargo test -p orchester-app --test agents local_hub_create_profile`.

use std::collections::BTreeMap;

use orchester_app::routes::HubFilter;
use orchester_core::{
  account::AccountCli,
  connection::AgentProvider,
  id::ResourceId,
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AgentTargets, ApprovalMode, DeniedBehavior, InjectionEvent, Instruction,
    InstructionScope, PromptText, ResourceBody, Skill, Slug, Tool,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tools::{data::create_tool, form::CommandText};

use super::{harness::Agents, local_hub::server, local_hub_create::show_form};

/// The field of what a new profile runs on, which opens the *Runs on* picker.
pub(super) const RUNS_ON: &str = "team-new-profile-runs-on";

/// Chooses `model` of Codex's own sign-in in the new profile's *Runs on* picker, as a person's presses would: the
/// account on the rail, then the model in its list, and closes the picker.
pub(super) fn run_on_codex(agents: &mut Agents, model: &str) {
  let menu = format!("{RUNS_ON}-menu");
  if !agents.exists(&menu) {
    agents.click(RUNS_ON);
    agents.until("the picker", |s| s.exists(&menu));
  }
  let account = format!("{menu}-connection-account-{}", AccountCli::Codex.default_account());
  agents.until("the account on the rail", |s| s.exists(&account));
  agents.click(&account);
  let mut row = String::new();
  agents.until("the model in the list", |s| {
    let found = (0..8)
      .map(|at| format!("{menu}-model-{at}"))
      .find(|id| s.exists(id) && s.text(id).contains(model));
    row = found.clone().unwrap_or_default();
    found.is_some()
  });
  agents.click(&row);
  agents.until("the field says it", |s| s.text(RUNS_ON).contains(model));
  // The picker stays open for the effort; the field closes it, as a person does before going on with the form.
  agents.click(RUNS_ON);
  agents.until("the picker closed", |s| !s.exists(&menu));
}

/// `body` made as `name` and published as v1.
fn published<B: ResourceBody>(agents: &Agents, name: &str, body: &B) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new(name).expect("slug"),
      Name::new(name).expect("name"),
      body,
      Timestamp::now(),
    )
    .expect("created")
    .id();
  db.publish_draft::<B>(id, Timestamp::now()).expect("v1");
  id
}

/// A command tool an agent can call, as a check: it has an access policy.
fn check(agents: &Agents) -> ResourceId {
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Checks").expect("name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy");
  let command = CommandText {
    program: std::env::current_exe()
      .expect("test program")
      .to_string_lossy()
      .into_owned(),
    description: "Runs checks".into(),
    ..CommandText::new_tool()
  }
  .tool(Some(policy))
  .expect("command");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = create_tool(&mut db, &Name::new("checks").expect("name"), &command, Timestamp::now()).expect("tool");
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("v1");
  id
}

/// Picks the entry `id` of the list `what` from its *Add* menu.
fn pick(agents: &mut Agents, what: &str, id: &str) {
  // The menu opens under a pointer's press, as a person's.
  agents.window.press(&format!("local-hub-profile-add-{what}-button"));
  let item = format!("local-hub-profile-{what}-{id}");
  agents.until("the menu", |s| s.exists(&item));
  agents.window.press(&item);
  agents.until("the tag", |s| s.exists(&format!("team-new-profile-{what}-value-0")));
}

#[test]
fn local_hub_create_profile_makes_it_with_its_role_instructions_and_what_it_gives() {
  let mut agents = Agents::new(true);
  let rules = published(
    &agents,
    "rules",
    &Instruction::new(
      PromptText::new("Keep it small.").expect("prompt"),
      AgentTargets::All,
      [InjectionEvent::Start].into(),
      InstructionScope::Project,
    )
    .expect("instruction"),
  );
  let review = published(
    &agents,
    "review-diff",
    &Skill::new(
      "Reviews a diff".into(),
      "# review-diff\n".into(),
      Vec::new(),
      InstructionScope::Project,
    )
    .expect("skill"),
  );
  let checks = check(&agents);
  let docs = server(&agents, "docs");
  // Tall enough that the lists under the role instructions are in the window, where a pointer presses them.
  agents.window.tree.resize(1440, 2400);
  show_form(&mut agents, HubFilter::Profiles);
  // The form has the panel's *Role*, with the role instructions.
  assert_eq!(agents.text("team-new-profile-role-label"), "Role");
  agents.set("team-new-profile-role-name-input", "Reviewer");
  agents.set("team-new-profile-role-instructions-input", "You are the Reviewer.");
  run_on_codex(&mut agents, "test-model");
  pick(&mut agents, "instructions", &rules.to_string());
  pick(&mut agents, "mcp", &docs.to_string());
  pick(&mut agents, "tools", &checks.to_string());
  pick(&mut agents, "skills", &review.to_string());
  agents.click("local-hub-new-create");
  agents.until("the profile", |s| {
    s.listed()
      .profiles
      .iter()
      .any(|entry| entry.profile.role().as_str() == "Reviewer")
  });
  let listed = agents.listed();
  let made = &listed
    .profiles
    .iter()
    .find(|entry| entry.profile.role().as_str() == "Reviewer")
    .expect("the profile")
    .profile;
  assert_eq!(made.instructions().as_str(), "You are the Reviewer.");
  assert_eq!(made.instruction_resources().ids(), [rules]);
  assert_eq!(made.skills().ids(), [review]);
  assert_eq!(made.tools().ids(), [checks]);
  assert_eq!(made.mcp_servers().ids(), [docs]);
}

/// The rail is this computer's accounts, as in the profile's panel, whether the project has a connection on one or
/// not: the picker opens on Claude Code's own sign-in, which this project has no connection on, so it gets one (the
/// program set for the CLI is a file that is not there, and its check says so), and nothing is chosen by that.
#[test]
fn local_hub_create_profile_an_account_looked_at_gets_the_projects_connection() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::Profiles);
  assert_eq!(agents.listed().connections.len(), 1, "only Codex's");

  agents.click(RUNS_ON);

  let menu = format!("{RUNS_ON}-menu");
  agents.until("the picker", |s| s.exists(&menu));
  for cli in AccountCli::ALL {
    let item = format!("{menu}-connection-account-{}", cli.default_account());
    assert!(agents.exists(&item), "{item} is on the rail");
  }
  assert!(
    agents.exists("team-new-profile-add-connection"),
    "Add opens the accounts"
  );
  agents.until("the connection on Claude Code's sign-in", |s| {
    s.listed().connections.len() == 2
  });
  let listed = agents.listed();
  let made = listed
    .connections
    .iter()
    .find(|connection| connection.provider() == AgentProvider::ClaudeCodeCli)
    .expect("made");
  assert_eq!(made.name().as_str(), "Claude Code");
  assert_eq!(
    agents.text(RUNS_ON),
    "Choose an account and a model",
    "looking at an account chooses nothing"
  );
}
