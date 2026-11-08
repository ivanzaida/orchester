//! What the Agents hub of a team project gives a member who is not an admin (`Agents · … · member` M1–M6 `oPF2o`,
//! `dijUt`, `H9nUK6`, `D8WX9`, `ITxzr`, `yTVeE`, ORC-839; `docs/design/team-configuration.md`, decision 7): the team's
//! part read as values under one quiet line naming the admins, no control of an admin's anywhere, the computer's own
//! part still theirs, and one empty state for a kind nothing is published of.
//! `cargo test -p orchester-app --test agents team_hub_member`.

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{
  id::{ResourceId, UserId},
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, ServerAdmin, SharedMember};

use super::{
  harness::Agents,
  support::project_id,
  team_hub::{open, team, team_as, team_with_profile_and_tool},
  team_hub_mcp::{Switch, program_body, program_file, remote_body, server, show, switch_of, team_server},
  team_hub_profile::{builder, claude, show as show_profile},
};

const LOCK: &str = "Only team admins change this: dana-k (owner) and olha (server admin).";

/// The members of the project as the server last listed them: dana-k its owner, olha a member and the server's admin,
/// and the person, ivan.
fn name_the_admins(agents: &Agents) {
  let person = |name: &str| (UserId::generate(), name.to_owned());
  let ((dana, dana_name), (olha, olha_name), (ivan, ivan_name)) = (person("dana-k"), person("olha"), person("ivan"));
  let member = |user, name: String, role| SharedMember {
    user,
    display_name: name.clone(),
    username: name,
    role,
    added_at: Timestamp::from_unix_millis(1),
  };
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.record_shared_members(
    &[
      member(dana, dana_name, MemberRole::Owner),
      member(olha, olha_name.clone(), MemberRole::Member),
      member(ivan, ivan_name, MemberRole::Member),
    ],
    Timestamp::from_unix_millis(2),
    Some(ivan),
  )
  .expect("members");
  db.record_server_admins(&[ServerAdmin {
    user: olha,
    display_name: olha_name.clone(),
    username: olha_name,
  }])
  .expect("admins");
}

/// The person is a member from here on: a member cannot make a team resource, so a test makes them first.
fn become_member(agents: &Agents) {
  ProjectDb::open(&agents.root)
    .expect("database")
    .record_sharing_state(Some(MemberRole::Member), None, None)
    .expect("the role");
}

fn lock(agents: &mut Agents) -> String {
  agents.text("team-hub-member-lock-text")
}

#[test]
fn team_hub_member_every_panel_names_the_admins_in_one_line_and_has_no_control_of_an_admin() {
  let mut agents = Agents::new(true);
  let [skill, rules, github, _] = team_as(&agents, Some(MemberRole::Member));
  name_the_admins(&agents);
  for (filter, resource) in [
    (HubFilter::Skills, skill),
    (HubFilter::Instructions, rules),
    (HubFilter::McpServers, github),
  ] {
    open(&mut agents, HubView::new(filter, Some(resource)));
    assert_eq!(lock(&mut agents), LOCK, "{filter:?}");
    assert!(
      !agents.exists("team-hub-new") && !agents.exists("team-hub-edit") && !agents.exists("team-hub-action-edit"),
      "{filter:?}: no New and no Edit"
    );
  }
  // Instructions and skills have nothing to do on a computer: no bar at all, rather than one that does nothing.
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert!(!agents.exists("team-hub-actions"));
  open(&mut agents, HubView::new(HubFilter::Instructions, Some(rules)));
  assert!(!agents.exists("team-hub-actions"));
}

#[test]
fn team_hub_member_a_profile_and_a_tool_name_the_admins_too() {
  let mut agents = Agents::new(true);
  let (profile, tool) = team_with_profile_and_tool(&agents, Some(MemberRole::Member));
  name_the_admins(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  assert_eq!(lock(&mut agents), LOCK);
  assert!(!agents.exists("team-hub-edit") && !agents.exists("team-hub-new"));
  open(&mut agents, HubView::new(HubFilter::Tools, Some(tool)));
  assert_eq!(lock(&mut agents), LOCK);
  assert!(!agents.exists("team-hub-edit") && !agents.exists("team-hub-actions"));
}

#[test]
fn team_hub_member_says_only_that_admins_change_it_before_the_members_are_read() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(lock(&mut agents), "Only team admins change this.");
}

#[test]
fn team_hub_member_an_admin_has_no_line_and_the_members_head_says_what_they_are() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team(&agents);
  name_the_admins(&agents);
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert!(!agents.exists("team-hub-member-lock") && !agents.exists("team-hub-member-role"));
  assert_eq!(
    agents.text("team-hub-filter-hint"),
    "Skills are the team's; each computer fills in only what is local"
  );
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(agents.text("team-hub-member-role"), "· member");
  assert_eq!(
    agents.text("team-hub-filter-hint"),
    "Skills are the team's; you fill in only what is local"
  );
}

#[test]
fn team_hub_member_a_kind_with_nothing_published_is_one_empty_state_and_no_new() {
  let mut agents = Agents::new(true);
  team_as(&agents, Some(MemberRole::Member));
  name_the_admins(&agents);
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::new(HubFilter::Tools, None)),
  ));
  agents.until("the empty state", |s| s.exists("team-hub-member-empty"));
  assert_eq!(agents.text("team-hub-empty-title"), "No tools yet");
  assert!(!agents.exists("team-hub-list") && !agents.exists("team-hub-new"));
  assert_eq!(agents.text("team-hub-filter-tools-count"), "0");
  // An admin keeps the generic state, which ORC-835 gives the kind's New.
  let mut agents = Agents::new(true);
  team(&agents);
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::new(HubFilter::Tools, None)),
  ));
  agents.until("empty", |s| s.exists("team-hub-empty"));
  assert!(!agents.exists("team-hub-member-empty"));
}

#[test]
fn team_hub_member_the_connection_pick_is_theirs_and_is_kept() {
  let mut agents = Agents::new(true);
  team(&agents);
  let first = claude(&agents.root, "Claude 2", "2.1.173");
  claude(&agents.root, "Claude Home", "2.1.170");
  let (profile, _) = builder(&agents);
  become_member(&agents);
  show_profile(&mut agents, profile);
  assert!(agents.exists("team-hub-pick") && !agents.exists("team-hub-edit"));
  agents.click("team-hub-fix-use");
  agents.until("the pick is kept", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .connection_pick(profile)
      .expect("pick")
      .is_some_and(|pick| pick.connection == first)
  });
}

#[test]
fn team_hub_member_the_program_path_and_on_here_are_theirs() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::Undecided);
  become_member(&agents);
  show(&mut agents, pencil);
  assert!(!agents.exists("team-hub-edit") && !agents.exists("team-hub-new"));
  let program = program_file(&agents);
  agents.stand_ins.choose(Some(program.clone()));
  agents.click("team-hub-choose-path");
  agents.until("the path is kept", |s| {
    AppDb::open(&s.window.data.database())
      .expect("application database")
      .mcp_server_program(team_server(s), pencil)
      .expect("read")
      .is_some_and(|set| set.program == program)
  });
  // The hub is read again after the path: the press waits for it, as a person's does.
  agents.press_until("team-hub-fix-switch-on", "on", |s| switch_of(s, pencil) == Some(true));
  agents.press_until("team-hub-here-switch", "off", |s| switch_of(s, pencil) == Some(false));
  agents.until("said off", |s| {
    s.text("team-hub-here-switch") == "Off: runs here don't use it"
  });
}

#[test]
fn team_hub_member_a_secret_value_is_still_asked_of_a_person_in_a_dialog() {
  let mut agents = Agents::new(true);
  team(&agents);
  let github = server(&agents, ("copilot", remote_body()), Switch::On);
  become_member(&agents);
  show(&mut agents, github);
  agents.until("its panel", |s| s.text("team-hub-panel-title") == "copilot");
  agents.press_until("team-hub-fix-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
}

#[test]
fn team_hub_member_an_item_with_nothing_to_fill_in_has_no_bar_and_a_ready_one_neither() {
  let mut agents = Agents::new(true);
  let [_, _, _, playwright] = team(&agents);
  let figma = serde_json::json!({
    "name": "figma", "launch": {"kind": "remote", "url": "https://mcp.figma.test/", "secret_headers": []}
  });
  let ready: ResourceId = server(&agents, ("figma", figma), Switch::On);
  become_member(&agents);
  open(&mut agents, HubView::new(HubFilter::McpServers, Some(ready)));
  agents.until("the panel", |s| s.exists("team-hub-from-team-url"));
  assert!(!agents.exists("team-hub-actions") && !agents.exists("team-hub-edit"));
  // A server switched off here is the member's to turn on: that bar stays.
  open(&mut agents, HubView::new(HubFilter::McpServers, Some(playwright)));
  agents.until("its bar", |s| s.exists("team-hub-actions"));
  assert!(agents.exists("team-hub-fix-switch-on") && !agents.exists("team-hub-edit"));
}
