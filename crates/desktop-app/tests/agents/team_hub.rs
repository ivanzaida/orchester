//! The Agents hub of a team project (`Agents · v2` `acFkz`, A1–A6, ORC-831): a filter by kind over a list grouped by
//! what this computer needs, and the chosen resource's panel. `cargo test -p orchester-app --test agents team_hub`.

use std::collections::HashMap;

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceBody, Skill, Slug,
    TeamMcpServer, VersionNumber,
  },
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, ResourceSharing, SharingLink};
use orchester_workspace::tools::{data::create_tool, form::CommandText};
use serde_json::{Value, json};

use super::{harness::Agents, support::project_id};

/// Links the project as its owner and caches the team's resources: the skill `review-diff` and the instruction
/// `Project rules` (always ready), the MCP server `github` nobody decided on here (needs the person) and `playwright`
/// switched off here. Returns their ids in that order.
pub(super) fn team(agents: &Agents) -> [ResourceId; 4] {
  team_as(agents, Some(MemberRole::Owner))
}

/// [`team`] for a person with `role`, which they have once the team's resources are cached: a member cannot make them.
pub(super) fn team_as(agents: &Agents, role: Option<MemberRole>) -> [ResourceId; 4] {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: db.project().key().clone(),
    role: Some(MemberRole::Owner),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
  let skill = Skill::new(
    "Review a diff.".into(),
    "# Review".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill");
  let rules = Instruction::new(
    PromptText::new("Be careful.").expect("prompt"),
    AgentTargets::All,
    std::collections::BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let remote = |name: &str| -> (Value, TeamMcpServer) {
    let body = json!({"name": name, "launch": {
      "kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": []
    }});
    (body.clone(), serde_json::from_value(body).expect("a team MCP server"))
  };
  let ids = [
    cached(&mut db, ("review-diff", "review-diff"), json!({}), &skill),
    cached(&mut db, ("project-rules", "Project rules"), json!({}), &rules),
    cached(&mut db, ("github", "github"), remote("github").0, &remote("github").1),
    cached(
      &mut db,
      ("playwright", "playwright"),
      remote("playwright").0,
      &remote("playwright").1,
    ),
  ];
  db.set_team_mcp_on_here(ids[3], false).expect("switched off here");
  if role != Some(MemberRole::Owner) {
    db.record_sharing_state(role, None, None).expect("the role");
  }
  ids
}

/// The team's `body` at v1, cached the way the stream keeps it.
pub(super) fn cached<B: ResourceBody + Clone>(
  db: &mut ProjectDb,
  (slug, name): (&str, &str),
  server_body: Value,
  body: &B,
) -> ResourceId {
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(name).expect("name"),
      body,
      Timestamp::from_unix_millis(10),
    )
    .expect("resource")
    .id();
  share(db, id, server_body, body);
  id
}

/// Makes the resource `id`, which has `body` here, the team's at v1.
fn share<B: ResourceBody + Clone>(db: &mut ProjectDb, id: ResourceId, server_body: Value, body: &B) {
  db.set_resource_sharing(
    id,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("shared");
  db.cache_shared_version(
    id,
    &CachedVersion {
      number: VersionNumber::FIRST,
      published_at: Timestamp::from_unix_millis(20),
      publisher: SharedPerson {
        id: UserId::generate(),
        name: "Olena".to_owned(),
      },
      server_body,
      hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
      references: Vec::new(),
      local_body: body.clone(),
      waits_here: false,
    },
  )
  .expect("cached");
}

/// A profile and a command tool the person made here before the project joined the team, which the team then took as
/// its own; the project is linked as `role`. Returns the profile's and the tool's ids.
pub(super) fn team_with_profile_and_tool(agents: &Agents, role: Option<MemberRole>) -> (ResourceId, ResourceId) {
  let profile = agents.profile(true);
  let command = CommandText {
    program: std::env::current_exe()
      .expect("test program")
      .to_string_lossy()
      .into_owned(),
    description: "Runs checks".into(),
    arguments: "test".into(),
    ..CommandText::new_tool()
  }
  .tool(None)
  .expect("command");
  let tool = {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    create_tool(&mut db, &Name::new("Checks").expect("name"), &command, Timestamp::now()).expect("tool")
  };
  team_as(agents, Some(MemberRole::Owner));
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let body = db.latest_version::<AgentProfile>(profile).expect("read").expect("v1");
  share(
    &mut db,
    profile,
    serde_json::to_value(body.body()).expect("body"),
    body.body(),
  );
  share(&mut db, tool, serde_json::to_value(&command).expect("body"), &command);
  if role != Some(MemberRole::Owner) {
    db.record_sharing_state(role, None, None).expect("the role");
  }
  (profile, tool)
}

pub(super) fn open(agents: &mut Agents, view: HubView) {
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::hub(view)));
  agents.until("the hub", |s| {
    s.exists("team-hub-list") && !s.exists("team-hub-loading")
  });
}

#[test]
fn team_hub_groups_the_teams_resources_by_what_this_computer_needs() {
  let mut agents = Agents::new(true);
  let [skill, rules, github, playwright] = team(&agents);
  open(&mut agents, HubView::default());
  assert_eq!(agents.text("team-hub-title"), "Agents");
  assert_eq!(agents.text("team-hub-section-needs-title"), "Needs you here");
  assert_eq!(agents.text("team-hub-section-needs-count"), "1");
  assert_eq!(agents.text("team-hub-section-ready-count"), "2");
  assert_eq!(agents.text("team-hub-section-off-count"), "1");
  // Under All every row names its kind, and the first row of the list is open in the panel.
  assert_eq!(agents.text(&format!("team-hub-row-{github}-tag")), "mcp");
  assert_eq!(agents.text(&format!("team-hub-row-{rules}-tag")), "instruction");
  assert_eq!(agents.text("team-hub-panel-title"), "github");
  assert_eq!(agents.text("team-hub-panel-kind"), "MCP server");
  assert!(agents.exists(&format!("team-hub-row-{skill}")) && agents.exists(&format!("team-hub-row-{playwright}")));
  assert_eq!(agents.text("team-hub-filter-all-count"), "4");
  assert_eq!(agents.text("team-hub-filter-mcp_servers-count"), "2");
  assert_eq!(agents.text("team-hub-filter-profiles-count"), "0");
}

#[test]
fn team_hub_keeps_the_filter_and_the_chosen_resource_in_the_route() {
  let mut agents = Agents::new(true);
  let [skill, ..] = team(&agents);
  open(&mut agents, HubView::default());
  agents.click("team-hub-filter-skills");
  let filtered = Route::Project(project_id(), ProjectPage::hub(HubView::new(HubFilter::Skills, None)));
  agents.until("the Skills filter", |s| {
    s.window.navigation().route_now() == filtered && s.exists("team-hub-panel-title")
  });
  assert_eq!(agents.text("team-hub-panel-title"), "review-diff");
  // Under a kind's filter its rows carry no tag.
  assert!(!agents.exists(&format!("team-hub-row-{skill}-tag")));
  assert_eq!(agents.text("team-hub-panel-kind"), "Skill");
  // A route that names a resource opens its panel, whatever the list's first row is.
  open(&mut agents, HubView::new(HubFilter::All, Some(skill)));
  assert_eq!(agents.text("team-hub-panel-title"), "review-diff");
  assert_eq!(agents.text("team-hub-panel-version"), "v1");
}

#[test]
fn team_hub_pins_edit_for_an_admin_where_the_kind_has_an_editor() {
  let mut agents = Agents::new(true);
  let [skill, _, github, _] = team(&agents);
  open(&mut agents, HubView::new(HubFilter::All, Some(skill)));
  assert!(agents.exists("team-hub-actions") && agents.exists("team-hub-edit"));
  // An MCP server has no editor yet: no bar, and nothing disabled in its place.
  open(&mut agents, HubView::new(HubFilter::All, Some(github)));
  assert!(!agents.exists("team-hub-actions") && !agents.exists("team-hub-edit"));
}

#[test]
fn team_hub_says_so_when_the_filter_holds_nothing() {
  let mut agents = Agents::new(true);
  team(&agents);
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::new(HubFilter::Tools, None)),
  ));
  agents.until("empty", |s| s.exists("team-hub-empty"));
  assert_eq!(agents.text("team-hub-filter-tools-count"), "0");
}

#[test]
fn team_hub_rows_show_what_a_server_is_and_its_version_under_the_kind_s_filter() {
  let mut agents = Agents::new(true);
  let [skill, rules, github, _] = team(&agents);
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  assert_eq!(agents.text(&format!("team-hub-row-{github}-tag")), "remote");
  assert_eq!(agents.text(&format!("team-hub-row-{github}-version")), "v1");
  // Under All a row names its kind and carries no version of its own.
  open(&mut agents, HubView::default());
  assert_eq!(agents.text(&format!("team-hub-row-{github}-tag")), "mcp");
  assert!(!agents.exists(&format!("team-hub-row-{github}-version")));
  // Instructions and skills need nothing of a computer: no tag, no version, and the list says why.
  open(&mut agents, HubView::new(HubFilter::Instructions, None));
  assert!(!agents.exists(&format!("team-hub-row-{rules}-tag")));
  assert!(!agents.exists(&format!("team-hub-row-{rules}-version")));
  assert_eq!(
    agents.text("team-hub-note-text"),
    "Instructions have nothing to fill in on a computer, so they are always ready here."
  );
  open(&mut agents, HubView::new(HubFilter::Skills, None));
  assert!(!agents.exists(&format!("team-hub-row-{skill}-version")));
  assert_eq!(
    agents.text("team-hub-note-text"),
    "Skills have nothing to fill in on a computer, so they are always ready here."
  );
}
