//! A local profile's panel edits in place (`Agents · Profile` L1, ORC-842; `Agents on accounts` C1 `RwzH2`, C2
//! `t7s3I`): what it runs on is one field, *Runs on*, which opens the compact *Runs on* picker; a model, an effort and
//! the MCP servers chosen there change only what the panel holds, *Save* publishes it as the next version at once
//! (asking first, of a person only, when it widens what its agents may do) and *Cancel* puts the published version
//! back. Another connection the profile runs on as it is applies at once, with no version. The panel has no drafts: a
//! draft stored of the profile is neither shown nor touched until *Save* replaces it.
//! `cargo test -p orchester-app --test agents local_hub_profile`.

use std::{thread, time::Duration};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, RepositoryAccess, VersionNumber},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::{
  data::{self, VersionState},
  form::stored_effort,
  local_edit::{self, Edit},
};

use super::{
  harness::{Agents, changed},
  local_hub::{builder_without_connection, open, server},
};

pub(super) const SAVE: &str = "local-hub-profile-save";
const CANCEL: &str = "local-hub-profile-cancel";
/// The field of what the profile runs on, and the picker it opens.
pub(super) const FIELD: &str = "local-hub-profile-runs-on";
pub(super) const MENU: &str = "local-hub-profile-runs-on-menu";

/// Opens the *Runs on* picker, where it is not open.
pub(super) fn open_picker(agents: &mut Agents) {
  if !agents.exists(MENU) {
    agents.click(FIELD);
    agents.until("the picker", |s| s.exists(MENU));
  }
}

/// Chooses `part` of the picker, opening it first: `effort-high`, `model-1` (the second model listed), or a
/// connection on the rail ([`on_rail`]).
pub(super) fn pick(agents: &mut Agents, part: &str) {
  open_picker(agents);
  let id = format!("{MENU}-{part}");
  agents.until("the part", |s| s.exists(&id));
  agents.click(&id);
  agents.until("a pass", |_| true);
}

/// The rail's item of `connection`.
pub(super) fn on_rail(connection: ConnectionId) -> String {
  format!("connection-{connection}")
}

/// The connection `profile` runs on on this computer.
pub(super) fn runs_here(agents: &Agents, profile: ResourceId) -> ConnectionId {
  let named = stored(agents, profile).0.connection();
  ProjectDb::open(&agents.root)
    .expect("database")
    .connection_here(profile, named)
    .expect("read")
}
/// The question a save that widens asks, and its answers.
pub(super) const ALLOW: &str = "local-hub-profile-confirm-confirm";
const KEEP: &str = "local-hub-profile-confirm-keep";

/// Opens the hub on Profiles with `profile` in the panel.
pub(super) fn show(agents: &mut Agents, profile: ResourceId) {
  open(agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists(FIELD));
}

/// Whether the hub's row of `profile` says the panel holds a change.
pub(super) fn holds_a_change(agents: &mut Agents, profile: ResourceId) -> bool {
  agents
    .text(&format!("local-hub-row-{profile}-line"))
    .contains("Unsaved")
}

pub(super) fn stored(agents: &Agents, profile: ResourceId) -> (AgentProfile, VersionState) {
  let entry = agents.listed().profile(profile).expect("profile").clone();
  (entry.profile, entry.state)
}

fn effort(agents: &Agents, profile: ResourceId) -> Option<String> {
  stored_effort(&stored(agents, profile).0)
}

pub(super) fn v2() -> VersionState {
  VersionState::Published(VersionNumber::FIRST.next().expect("v2"))
}

/// Presses *Save* once, as a person does, and waits until `done`: another press while a question is asked would ask
/// it again.
pub(super) fn save(agents: &mut Agents, what: &str, done: impl FnMut(&mut Agents) -> bool) {
  agents.click(SAVE);
  agents.until(what, done);
}

/// Lets the window run a while, as it does between a person's presses.
pub(super) fn passes(agents: &mut Agents) {
  for _ in 0..20 {
    agents.until("a pass", |_| true);
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn local_hub_profile_a_changed_field_is_held_until_save_publishes_it_as_the_next_version() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  assert!(
    agents.text(FIELD).ends_with("· test-model"),
    "one field says what it runs on: {}",
    agents.text(FIELD)
  );
  for select in [
    "local-hub-profile-cli",
    "local-hub-profile-connection",
    "local-hub-profile-model",
    "local-hub-profile-effort",
  ] {
    assert!(!agents.exists(select), "{select}: the one field stands for it");
  }
  for link in ["local-hub-edit", "local-hub-profile-more-settings"] {
    assert!(
      !agents.exists(link),
      "{link}: no link to the editor's page, the fields are in place"
    );
  }

  pick(&mut agents, "effort-high");

  agents.until("the panel holds it", |s| holds_a_change(s, profile));
  assert_eq!(effort(&agents, profile), None, "nothing is stored before Save");
  assert!(!stored(&agents, profile).1.has_draft(), "and no draft is made");
  assert!(agents.text("team-hub-actions-hint").contains("Save makes v2"));
  save(&mut agents, "published", |s| stored(s, profile).1 == v2());
  assert_eq!(effort(&agents, profile).as_deref(), Some("high"));
  agents.until("nothing is held", |s| !holds_a_change(s, profile));
}

#[test]
fn local_hub_profile_cancel_puts_the_published_version_back_and_stores_nothing() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  pick(&mut agents, "effort-low");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));

  agents.click(CANCEL);

  agents.until("the version again", |s| !holds_a_change(s, profile));
  assert_eq!(effort(&agents, profile), None);
  assert_eq!(
    stored(&agents, profile).1,
    VersionState::Published(VersionNumber::FIRST)
  );
  assert!(!stored(&agents, profile).1.has_draft());
}

/// Holds `docs` among the MCP servers `profile` chooses, as a person's choice in *Add* would.
fn add_server(agents: &mut Agents, profile: ResourceId, docs: ConnectionId) {
  // The menu opens under a pointer's press, as a person's.
  // The row is under *Role*: the window is tall enough to show it, so the pointer's press lands on its *Add*.
  agents.window.tree.resize(1440, 2400);
  agents.until("a pass", |_| true);
  agents.window.press("local-hub-profile-add-server-button");
  let item = format!("local-hub-profile-server-{docs}");
  agents.until("the menu", |s| s.exists(&item));
  agents.window.press(&item);
  agents.until("the panel holds it", |s| {
    holds_a_change(s, profile) && s.exists("local-hub-profile-mcp-value-0")
  });
}

fn chooses(agents: &Agents, profile: ResourceId, docs: ConnectionId) -> bool {
  stored(agents, profile).0.mcp_servers().contains(docs)
}

#[test]
fn local_hub_profile_saving_a_chosen_server_asks_first_and_only_a_press_allows_it() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let docs = server(&agents, "docs");
  show(&mut agents, profile);
  add_server(&mut agents, profile, docs);
  assert!(agents.text("local-hub-profile-mcp-value-0").contains("docs"));
  assert!(!agents.exists(ALLOW), "a choice asks nothing: Save does");

  save(&mut agents, "the question", |s| s.exists(ALLOW));

  assert!(!chooses(&agents, profile, docs), "nothing saved before the answer");
  // Its confirm takes no press before the question has been drawn once.
  agents.press_until(ALLOW, "published", |s| stored(s, profile).1 == v2());
  assert!(chooses(&agents, profile, docs), "v2 chooses it");
}

#[test]
fn local_hub_profile_keep_editing_leaves_the_change_held_and_stores_nothing() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let docs = server(&agents, "docs");
  show(&mut agents, profile);
  add_server(&mut agents, profile, docs);
  save(&mut agents, "the question", |s| s.exists(KEEP));

  agents.press_until(KEEP, "the question is gone", |s| !s.exists(KEEP));

  assert!(!chooses(&agents, profile, docs), "nothing stored");
  assert!(holds_a_change(&mut agents, profile), "the change is still the panel's");
  assert!(agents.text("local-hub-profile-mcp-value-0").contains("docs"));
  save(&mut agents, "Save asks again", |s| s.exists(ALLOW));
}

#[test]
fn local_hub_profile_a_held_change_replaces_the_pinned_fix_with_save_and_cancel() {
  let mut agents = Agents::new(true);
  let (profile, _) = builder_without_connection(&agents);
  let docs = server(&agents, "docs");
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the fix", |s| s.exists("local-hub-fix-use"));
  assert!(!agents.exists(SAVE), "while nothing is changed the bar is the fix's");

  add_server(&mut agents, profile, docs);

  agents.until("the bar is Save's and Cancel's", |s| {
    !s.exists("local-hub-fix-use") && s.exists(SAVE) && s.exists(CANCEL)
  });
  agents.click(CANCEL);
  agents.until("the fix again", |s| s.exists("local-hub-fix-use"));
}

/// Changes the stored draft as another window or the profile's editor would, after the panel showed it.
fn change_elsewhere(agents: &Agents, profile: ResourceId, effort: &str) {
  // The draft's time moves on by at least a millisecond.
  thread::sleep(Duration::from_millis(5));
  local_edit::edit(&agents.root, profile, &Edit::Effort(Some(effort.into())), &[]).expect("saved elsewhere");
}

#[test]
fn local_hub_profile_cancel_leaves_a_draft_stored_meanwhile_alone() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  pick(&mut agents, "effort-low");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));
  change_elsewhere(&agents, profile, "high");

  agents.click(CANCEL);

  agents.until("the version again", |s| !holds_a_change(s, profile));
  passes(&mut agents);
  assert_eq!(
    effort(&agents, profile).as_deref(),
    Some("high"),
    "the draft stored elsewhere is kept"
  );
  assert!(stored(&agents, profile).1.has_draft());
}

#[test]
fn local_hub_profile_save_publishes_what_the_panel_holds_over_a_draft_stored_meanwhile() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  pick(&mut agents, "effort-low");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));
  change_elsewhere(&agents, profile, "high");

  save(&mut agents, "published", |s| stored(s, profile).1 == v2());

  assert_eq!(effort(&agents, profile).as_deref(), Some("low"), "what the panel held");
  assert!(!stored(&agents, profile).1.has_draft(), "Save replaces the draft");
}

#[test]
fn local_hub_profile_save_never_publishes_the_prompts_of_a_draft_it_did_not_show() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let published = stored(&agents, profile).0;
  let body = changed(&published);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  data::save_draft(&mut db, profile, body, Timestamp::now()).expect("a draft that tells its agents more");
  drop(db);
  show(&mut agents, profile);
  pick(&mut agents, "effort-high");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));

  save(&mut agents, "published", |s| stored(s, profile).1 == v2());

  let (now, _) = stored(&agents, profile);
  assert_eq!(
    now.instructions(),
    published.instructions(),
    "the draft's prompts are not published"
  );
  assert_eq!(stored_effort(&now).as_deref(), Some("high"));
}

/// A Claude Code connection whose last check found `claude-a` (with two efforts) and `claude-b`.
pub(super) fn claude(agents: &Agents) -> ConnectionId {
  claude_signed(agents, SignIn::SignedIn)
}

/// [`claude`] whose last check found the CLI as `sign_in` says.
fn claude_signed(agents: &Agents, sign_in: SignIn) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude 2").expect("name"),
    AgentProvider::ClaudeCodeCli,
    agents.root.join("claude.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  let model = |id: &str, efforts: &[&str]| {
    CatalogModel::new(
      ModelId::new(id).expect("model"),
      efforts.iter().map(|effort| (*effort).to_owned()).collect(),
    )
  };
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![model("claude-a", &["low", "high"]), model("claude-b", &[])],
        ),
      },
    })
    .expect("check");
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_connection(&connection)
    .expect("stored");
  connection.id()
}

/// Builder on `connection`'s `claude-a`, published as v1, with what every CLI enforces so that nothing widens.
pub(super) fn builder_on_claude(agents: &Agents, connection: ConnectionId) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new("Builder").expect("name"),
    "Builds the change".into(),
    PromptText::new("Implement the approved plan.").expect("instructions"),
    connection,
    ModelId::new("claude-a").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ));
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = data::create_profile(&mut db, &profile, Timestamp::now()).expect("profile");
  data::publish(&mut db, id, Timestamp::now()).expect("publish");
  id
}

#[test]
fn local_hub_profile_another_model_starts_at_the_default_effort() {
  let mut agents = Agents::new(true);
  let profile = builder_on_claude(&agents, claude(&agents));
  show(&mut agents, profile);
  pick(&mut agents, "effort-high");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));

  pick(&mut agents, "model-1");
  // A choice is made off the UI thread: the field says it once the panel holds it.
  agents.until("the panel holds the model", |s| s.text(FIELD).contains("claude-b"));
  save(&mut agents, "published", |s| stored(s, profile).1 == v2());

  assert_eq!(stored(&agents, profile).0.model().as_str(), "claude-b");
  assert_eq!(
    effort(&agents, profile),
    None,
    "a new model starts at the CLI's default"
  );
}

#[test]
fn local_hub_profile_automation_cannot_answer_allow_for_a_profile() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let docs = server(&agents, "docs");
  show(&mut agents, profile);
  add_server(&mut agents, profile, docs);
  save(&mut agents, "the question", |s| s.exists(ALLOW));
  // The confirm takes no press before the question lived through a pass: press it as a client would, until it answers.
  agents.until("the refusal", |s| {
    with_synthetic_input(|| s.click(ALLOW));
    s.refused().is_some()
  });
  passes(&mut agents);

  assert!(!chooses(&agents, profile, docs), "automation allowed nothing");
  assert!(agents.exists(ALLOW), "it still asks");
  assert!(
    !agents.exists("local-hub-profile-confirm-person-only"),
    "nothing in the window says it"
  );
  agents.press_until(ALLOW, "a person allows it", |s| chooses(s, profile, docs));
}

#[test]
fn local_hub_profile_a_list_row_ends_in_its_version_under_a_kinds_filter_only() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  show(&mut agents, profile);
  let version = format!("local-hub-row-{profile}-version");
  assert_eq!(agents.text(&version), "v1");

  open(&mut agents, HubView::new(HubFilter::All, Some(profile)));
  agents.until("the row", |s| s.exists(&format!("local-hub-row-{profile}")));
  assert!(
    !agents.exists(&version),
    "under All the row ends in its kind's tag instead"
  );
}

/// A profile on a connection whose CLI is signed out is not ready (ORC-848 D11): *Ready?* says so and the pinned fix is
/// to sign in.
#[test]
fn local_hub_profile_a_signed_out_connection_is_not_ready_and_the_fix_is_to_sign_in() {
  let mut agents = Agents::new(true);
  let connection = claude_signed(&agents, SignIn::SignedOut);
  let profile = builder_on_claude(&agents, connection);
  show(&mut agents, profile);

  agents.until("the sign-in fix", |s| s.exists("local-hub-fix-sign-in"));
  assert!(agents.text("local-hub-fix-sign-in").contains("Sign in to Claude Code"));
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "Claude Code is signed out here"
  );
}
