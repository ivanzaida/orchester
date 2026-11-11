//! What the Agents hub of a team project gives a member who is not an admin (`Agents · … · member` M1–M6 `oPF2o`,
//! `dijUt`, `H9nUK6`, `D8WX9`, `ITxzr`, `yTVeE`, ORC-839; `docs/design/team-configuration.md`, decision 7): the team's
//! part read as values under one quiet line naming the admins, no control of an admin's anywhere, the computer's own
//! part still theirs, and one empty state for a kind nothing is published of.
//! `cargo test -p orchester-app --test agents team_hub_member`.

use std::collections::HashMap;

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
  team_hub::{keep_team, open, team, team_as, team_with_profile_and_tool},
  team_hub_mcp::{Switch, program_body, program_file, remote_body, server, show, switch_of, team_server},
  team_hub_profile::{builder, claude, show as show_profile},
};

const LOCK: &str = "Only team admins change this: dana-k (Admin) and olha (Server admin).";

/// The members of the project as the server last listed them: `owners`, the other `members`, and the person, ivan; and
/// the server's admins, `server_admins`, who need not be among the members.
fn name_members(agents: &Agents, (owners, members): (&[&str], &[&str]), server_admins: &[&str]) {
  let mut users: HashMap<&str, UserId> = HashMap::new();
  let ivan = UserId::generate();
  let listed: Vec<SharedMember> = owners
    .iter()
    .map(|name| (*name, MemberRole::Owner))
    .chain(members.iter().map(|name| (*name, MemberRole::Member)))
    .map(|(name, role)| {
      let user = *users.entry(name).or_insert_with(UserId::generate);
      SharedMember {
        user,
        display_name: name.to_owned(),
        username: name.to_owned(),
        role,
        added_at: Timestamp::from_unix_millis(1),
      }
    })
    .chain([SharedMember {
      user: ivan,
      display_name: "ivan".to_owned(),
      username: "ivan".to_owned(),
      role: MemberRole::Member,
      added_at: Timestamp::from_unix_millis(1),
    }])
    .collect();
  let admins: Vec<ServerAdmin> = server_admins
    .iter()
    .map(|name| ServerAdmin {
      user: *users.entry(name).or_insert_with(UserId::generate),
      display_name: (*name).to_owned(),
      username: (*name).to_owned(),
    })
    .collect();
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.record_shared_members(&listed, Timestamp::from_unix_millis(2), Some(ivan))
    .expect("members");
  db.record_server_admins(&admins).expect("admins");
}

/// dana-k the owner, olha a member and the server's admin, and the person, ivan.
fn name_the_admins(agents: &Agents) {
  name_members(agents, (&["dana-k"], &["olha"]), &["olha"]);
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
      !agents.exists("team-hub-new") && !agents.exists("team-hub-edit"),
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
  // The role is the team's too: read-only, as the rest of *From the team*.
  assert_eq!(
    agents.text("team-hub-role-name-value"),
    agents.text("team-hub-panel-title"),
    "the member reads its role"
  );
  assert!(
    !agents.exists("team-hub-role-name-input"),
    "its role is values, not locked fields"
  );
  assert_eq!(agents.text("team-hub-from-team-skills-value"), "None");
  open(&mut agents, HubView::new(HubFilter::Tools, Some(tool)));
  assert_eq!(lock(&mut agents), LOCK);
  assert!(!agents.exists("team-hub-edit") && !agents.exists("team-hub-actions"));
}

#[test]
fn team_hub_member_names_the_projects_admins_and_the_servers_before_the_members_are_read() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(
    lock(&mut agents),
    "Only team admins change this: the project's Admins and the Server admins."
  );
}

#[test]
fn team_hub_member_a_server_that_names_no_admins_leaves_them_by_their_role() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  name_members(&agents, (&["dana-k"], &["olha"]), &[]);
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(
    lock(&mut agents),
    "Only team admins change this: dana-k (Admin) and the Server admins."
  );
}

#[test]
fn team_hub_member_a_server_admin_who_is_not_a_member_of_the_project_is_named_too() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  name_members(&agents, (&["dana-k"], &[]), &["olha"]);
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(lock(&mut agents), LOCK);
}

#[test]
fn team_hub_member_an_admin_has_no_line_and_the_members_head_says_what_they_are() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team(&agents);
  name_the_admins(&agents);
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert!(!agents.exists("team-hub-member-lock") && !agents.exists("team-hub-member-role"));
  // The head says *Team*, and no server's address after it.
  assert_eq!(agents.text("team-hub-team-label"), "Team");
  assert!(!agents.exists("team-hub-team-host"));
  let mut agents = Agents::new(true);
  let [skill, ..] = team_as(&agents, Some(MemberRole::Member));
  open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
  assert_eq!(agents.text("team-hub-team-label"), "Team");
  assert_eq!(agents.text("team-hub-member-role"), "· member");
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
  agents.until("the empty state", |s| s.exists("team-hub-empty-body"));
  assert_eq!(agents.text("team-hub-empty-title"), "No tools yet");
  assert!(agents.exists("team-hub-empty-admins"));
  assert!(!agents.exists("team-hub-list") && !agents.exists("team-hub-new"));
  assert!(
    !agents.exists("team-hub-empty-new"),
    "a member's empty state has no action"
  );
  assert_eq!(agents.text("team-hub-filter-tools-count"), "0");
  // An admin's state says what the kind is, with its New, and names no admins.
  let mut agents = Agents::new(true);
  team(&agents);
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::new(HubFilter::Tools, None)),
  ));
  agents.until("empty", |s| s.exists("team-hub-empty-body"));
  assert_eq!(agents.text("team-hub-empty-new"), "New tool");
  assert!(!agents.exists("team-hub-empty-admins"));
}

/// Takes every resource of [`team_as`] off the team, so no kind holds anything.
fn publish_nothing(agents: &Agents, resources: [ResourceId; 4]) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  for resource in resources {
    db.set_resource_sharing(resource, None).expect("local");
  }
  keep_team(agents);
}

#[test]
fn team_hub_member_every_kind_with_nothing_published_says_an_admin_adds_it_inside_the_outline() {
  let mut agents = Agents::new(true);
  let resources = team_as(&agents, Some(MemberRole::Member));
  publish_nothing(&agents, resources);
  name_the_admins(&agents);
  for (filter, title) in [
    (HubFilter::All, "Nothing published yet"),
    (HubFilter::Profiles, "No profiles yet"),
    (HubFilter::McpServers, "No MCP servers yet"),
    (HubFilter::Instructions, "No instructions yet"),
    (HubFilter::Skills, "No skills yet"),
    (HubFilter::Tools, "No tools yet"),
  ] {
    agents.window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::hub(HubView::new(filter, None)),
    ));
    agents.until(title, |s| {
      s.exists("team-hub-empty-body") && s.text("team-hub-empty-title") == title
    });
    assert!(
      agents
        .text("team-hub-empty-detail")
        .starts_with("An admin of the team adds ")
    );
    assert_eq!(
      agents.text("team-hub-empty-admins"),
      "Admins: dana-k (Admin) and olha (Server admin)",
      "{filter:?}"
    );
    assert!(
      !agents.exists("team-hub-list") && !agents.exists("team-hub-new") && !agents.exists("team-hub-empty-new"),
      "{filter:?}"
    );
  }
}

/// An admin's empty MCP servers tab says what a team server is (A24 `OZ056`); the other kinds say what the kind is, in
/// the same words as a local project's.
#[test]
fn team_hub_member_an_admins_empty_mcp_servers_say_the_teams_runs_use_them() {
  let mut agents = Agents::new(true);
  let resources = team(&agents);
  publish_nothing(&agents, resources);
  for (filter, detail) in [
    (
      HubFilter::McpServers,
      "Add a server the team's runs can use. Once published it is here for everyone; each computer sets its program path.",
    ),
    (
      HubFilter::Tools,
      "Commands and requests agents may run, with their approvals.",
    ),
  ] {
    agents.window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::hub(HubView::new(filter, None)),
    ));
    agents.until("the empty state", |s| {
      s.exists("team-hub-empty-body") && s.text("team-hub-empty-detail") == detail
    });
    assert!(agents.exists("team-hub-empty-new"), "{filter:?}");
  }
  assert_eq!(agents.text("team-hub-empty-title"), "No tools yet");
}

#[test]
fn team_hub_member_the_palette_offers_no_new_pipeline_or_agent_but_an_admins_does() {
  let offered = |role| {
    let mut agents = Agents::new(true);
    let [skill, ..] = team_as(&agents, Some(role));
    open(&mut agents, HubView::new(HubFilter::Skills, Some(skill)));
    agents.window.press("sidebar-jump");
    agents
      .window
      .until("Jump to", |window| window.texts("jump-to").is_some());
    agents.window.set("jump-to-field", "new");
    agents
      .window
      .until("the actions", |window| window.texts("jump-to-group-actions").is_some());
    ["new-task", "new-pipeline", "new-agent"]
      .map(|action| agents.window.texts(&format!("jump-to-actions-{action}")).is_some())
  };
  assert_eq!(offered(MemberRole::Member), [true, false, false]);
  assert_eq!(offered(MemberRole::Owner), [true, true, true]);
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
