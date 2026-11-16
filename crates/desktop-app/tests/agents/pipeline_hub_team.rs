//! The Pipelines hub of a team project: the same page a local project has (`Pipelines · v2` P1), with the team's
//! pipelines read from the team's configuration in memory and every change sent to the team server at once (D-043),
//! against the stand-in team server. A save waits for the server's answer and keeps the edit when it is refused or the
//! server cannot be reached; a rename, *Create v1*, *Archive* and *Restore* go to the server too; the versions are the
//! server's; a member who is not an admin reads the pipelines. Nothing of them is kept in the project's database.
//! `cargo test -p orchester-app --test agents pipeline_hub_team`.

use std::{path::PathBuf, time::Duration};

use orchester_api::test_support::team_server::{Standing, TeamServer};
use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  sharing::{MemberRole, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  sharing::Servers,
  team_config::{Changed, TeamResource, TeamVersion},
};

use super::{
  harness::Agents,
  pipeline_hub::open,
  pipeline_hub_menu::{key, open_menu, start_rename},
  support::team_link::{STAND_IN_SLUG, share_on_stand_in},
};

const SAVE: &str = "pipeline-hub-save";
const DESCRIPTION: &str = "pipeline-hub-description";

/// Kontur shared with the stand-in team server, the person its admin, with the team's tool *Checks* and the team's
/// pipeline *Delivery* of one step that runs it.
struct Team {
  agents: Agents,
  server: TeamServer,
  pipeline: ResourceId,
}

impl Team {
  fn new() -> Self {
    let agents = Agents::new(true);
    let server = share_on_stand_in(&agents.window, &agents.root);
    let tool = team_tool(&agents);
    let body = one_step(tool);
    let pipeline = on_worker(&agents, move |servers, root| {
      let named = (
        Slug::new("delivery").expect("a slug"),
        Name::new("Delivery").expect("a name"),
      );
      servers.create_team_pipeline(&root, named, &body)
    })
    .expect("the team server makes it");
    Self {
      agents,
      server,
      pipeline,
    }
  }

  /// The hub with *Delivery* open.
  fn opened() -> Self {
    let mut team = Self::new();
    open(&mut team.agents, PipelinesFilter::All, Some(team.pipeline));
    team.agents.until("the panel", |s| s.exists(SAVE));
    team
  }

  /// What the team's configuration in memory holds of `id`.
  fn held(&self, id: ResourceId) -> Option<TeamResource> {
    let workspace = self.agents.window.launch.workspace().expect("the session");
    let read = workspace.team_read(&self.agents.root);
    read.config().and_then(|config| config.resource(id).cloned())
  }

  /// The number and description of the latest version of `id` in memory.
  fn latest(&self, id: ResourceId) -> (u32, String) {
    let held = self.held(id).expect("memory holds the pipeline");
    let body = held.body::<Pipeline>().expect("a version").expect("a pipeline");
    (held.number().expect("a version").get(), body.description().to_owned())
  }

  /// How many versions the team server holds of `id`, its name there and whether it is archived there.
  fn on_server(&self, id: ResourceId) -> (usize, String, bool) {
    let held = self.server.resources(STAND_IN_SLUG);
    let found = held.into_iter().find(|(resource, _)| resource["id"] == id.to_string());
    let (resource, versions) = found.expect("the team server holds the pipeline");
    let name = resource["name"].as_str().unwrap_or_default().to_owned();
    (versions.len(), name, resource["archived"] == true)
  }

  /// Whether the project's database holds `id`: it keeps no copy of what the team has.
  fn in_database(&self, id: ResourceId) -> bool {
    let db = ProjectDb::open(&self.agents.root).expect("database");
    db.resource(id).expect("read").is_some()
  }
}

/// Runs `work` with the session's servers on a worker of the window's runtime, as the app does.
fn on_worker<T: Send + 'static>(agents: &Agents, work: impl FnOnce(Servers, PathBuf) -> T + Send + 'static) -> T {
  let servers = agents.window.launch.services().expect("the session").servers.clone();
  let root = agents.root.clone();
  agents
    .window
    .data
    .runtime
    .block_on(async move { tokio::task::spawn_blocking(move || work(servers, root)).await })
    .expect("the work finished")
}

/// Keeps the tool *Checks*, which runs `cargo`, in the team's configuration in memory, as a read of the team server
/// holds it: a team pipeline's steps may run only the team's profiles and tools.
fn team_tool(agents: &Agents) -> ResourceId {
  let program = CommandProgram::new("cargo").expect("a program");
  let tool = Tool::new("", ToolAction::Command(CommandAction::new(program, vec![])), None).expect("a tool");
  let id = ResourceId::generate();
  let resource = TeamResource {
    id,
    kind: ResourceKind::Tool,
    name: Name::new("Checks").expect("a name"),
    slug: "checks".to_owned(),
    archived: false,
    revision: TaskRevision::FIRST,
    latest: Some(TeamVersion {
      number: VersionNumber::FIRST,
      body: serde_json::to_value(&tool).expect("a body"),
      published_by: "Alice Moreau".to_owned(),
      published_at: Timestamp::now(),
    }),
    updated: Changed {
      by: "Alice Moreau".to_owned(),
      at: Timestamp::now(),
    },
  };
  let workspace = agents.window.launch.workspace().expect("the session");
  assert!(
    workspace.keep_team_resource_for_test(&agents.root, resource),
    "the team's configuration is read"
  );
  id
}

/// Lets what a press started run for a few passes, without waiting for it to end.
fn settle(agents: &mut Agents) {
  for _ in 0..3 {
    std::thread::sleep(Duration::from_millis(5));
    agents.window.tree.tick_futures();
    agents.window.pass();
  }
}

/// A pipeline of one step, *Check*, that runs `tool`.
fn one_step(tool: ResourceId) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty body"));
  let step = edit.add_step(Name::new("Check").expect("a name"), None);
  edit.update(&step, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  edit.validate().expect("a valid pipeline")
}

#[test]
fn pipeline_hub_team_is_the_hub_a_local_project_has_with_the_teams_line() {
  let mut team = Team::opened();
  let id = team.pipeline;
  assert!(team.agents.exists("screen-pipeline-hub"));
  assert!(team.agents.exists(&format!("pipeline-hub-row-{id}")));
  assert_eq!(team.agents.text("team-hub-panel-title"), "Delivery");
  assert_eq!(team.agents.text("team-hub-team-label"), "Team");
  assert!(!team.agents.exists("local-hub-local"), "not a local project's head");
  let line = team.agents.text("pipeline-hub-team-line");
  assert!(line.contains("goes to the team server at once"), "{line}");
  assert!(!team.in_database(id), "the project's database keeps no copy");
}

#[test]
fn pipeline_hub_team_save_goes_to_the_team_server_and_waits_for_its_answer() {
  let mut team = Team::opened();
  let id = team.pipeline;
  team.server.slow_resource_writes(Duration::from_millis(1200));
  team.agents.set(DESCRIPTION, "Changed.");
  team.agents.click(SAVE);
  settle(&mut team.agents);
  assert_eq!(
    team.latest(id),
    (1, String::new()),
    "nothing is kept while the server answers"
  );
  assert_eq!(team.agents.value(DESCRIPTION), "Changed.", "the edit stays in view");
  assert!(!team.agents.exists("pipeline-hub-saved"), "not said saved before it is");

  team.agents.until("saved", |s| s.exists("pipeline-hub-saved-title"));
  assert!(team.agents.text("pipeline-hub-saved-title").starts_with("Saved as v2"));
  assert_eq!(team.latest(id), (2, "Changed.".to_owned()));
  assert_eq!(team.on_server(id).0, 2);
  assert!(!team.in_database(id), "the project's database keeps no copy");
}

#[test]
fn pipeline_hub_team_a_refused_save_keeps_the_edit_with_why() {
  let mut team = Team::opened();
  let id = team.pipeline;
  team.server.reject_content(STAND_IN_SLUG, "body");
  team.agents.set(DESCRIPTION, "Changed.");
  team.agents.click(SAVE);
  team.agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));

  let said = team.agents.text("pipeline-hub-refused");
  assert!(said.contains("does not accept its content"), "{said}");
  assert_eq!(team.agents.value(DESCRIPTION), "Changed.", "the edit stays as typed");
  assert_eq!(team.latest(id), (1, String::new()), "nothing is kept");
  assert_eq!(team.on_server(id).0, 1);
}

#[test]
fn pipeline_hub_team_an_unreachable_server_keeps_the_edit_with_why() {
  let mut team = Team::opened();
  let id = team.pipeline;
  team.server.set_down(true);
  team.agents.set(DESCRIPTION, "Changed.");
  team.agents.click(SAVE);
  team.agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));

  let said = team.agents.text("pipeline-hub-refused");
  assert!(
    said.contains("The team server can't be reached, so nothing was saved"),
    "{said}"
  );
  assert_eq!(team.agents.value(DESCRIPTION), "Changed.");
  assert_eq!(team.latest(id), (1, String::new()));
}

#[test]
fn pipeline_hub_team_rename_goes_to_the_team_server() {
  let mut team = Team::opened();
  let id = team.pipeline;
  start_rename(&mut team.agents, id);
  team.agents.set("pipeline-hub-rename", "Gates");
  key(&mut team.agents, "Enter");
  team.agents.until("renamed", |s| {
    s.exists("team-hub-panel-title") && s.text("team-hub-panel-title") == "Gates"
  });

  assert_eq!(
    team.on_server(id),
    (1, "Gates".to_owned(), false),
    "a rename saves no version"
  );
  let held = team.held(id).expect("memory holds it");
  assert_eq!((held.name.as_str(), held.slug.as_str()), ("Gates", "delivery"));
}

#[test]
fn pipeline_hub_team_menu_has_no_default_which_this_computer_would_keep() {
  let mut team = Team::opened();
  open_menu(&mut team.agents, team.pipeline);
  assert!(team.agents.exists("pipeline-hub-menu-rename"));
  assert!(team.agents.exists("pipeline-hub-menu-duplicate"));
  assert!(
    !team.agents.exists("pipeline-hub-menu-default"),
    "a team project keeps no default pipeline here"
  );
}

#[test]
fn pipeline_hub_team_archive_and_restore_go_to_the_team_server() {
  let mut team = Team::opened();
  let id = team.pipeline;
  open_menu(&mut team.agents, id);
  team.agents.click("pipeline-hub-menu-archive");
  team
    .agents
    .until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  team.agents.press_until("team-hub-archive-confirm", "archived", |s| {
    s.exists("pipeline-hub-filter-archived-count") && s.text("pipeline-hub-filter-archived-count") == "1"
  });
  assert!(team.on_server(id).2, "the team server archived it");
  assert!(team.held(id).is_some_and(|held| held.archived), "memory follows");

  team.agents.click("pipeline-hub-filter-archived");
  team.agents.until("Archived", |s| s.exists("pipeline-hub-restore"));
  team.agents.press_until("pipeline-hub-restore", "restored", |s| {
    s.text("pipeline-hub-filter-archived-count") == "0"
  });
  assert!(!team.on_server(id).2, "the team server restored it");
}

#[test]
fn pipeline_hub_team_versions_are_the_team_servers() {
  let mut team = Team::new();
  let id = team.pipeline;
  let next = team.latest(id);
  assert_eq!(next.0, 1);
  let body = team
    .held(id)
    .and_then(|held| held.body::<Pipeline>())
    .expect("a version")
    .expect("a pipeline")
    .with_description("Second.".into());
  on_worker(&team.agents, move |servers, root| {
    servers.publish_team_pipeline(&root, id, VersionNumber::FIRST, &body)
  })
  .expect("the team server numbers it");
  open(&mut team.agents, PipelinesFilter::All, Some(id));
  team
    .agents
    .until("the panel", |s| s.exists("pipeline-hub-versions-button"));
  team.agents.click("pipeline-hub-versions-button");
  team.agents.until("the versions", |s| s.exists("pipeline-hub-versions"));
  team.agents.until("both versions", |s| {
    let listed = s.text("pipeline-hub-versions");
    listed.contains("v2") && listed.contains("v1")
  });
  assert!(!team.in_database(id), "read from the team server, not from a copy");
}

#[test]
fn pipeline_hub_team_a_member_reads_the_pipelines() {
  let mut team = Team::new();
  let id = team.pipeline;
  // The person is a member from here on, here and on the server: an admin made the pipeline first.
  team.server.stand_as(Standing::Member);
  let mut db = ProjectDb::open(&team.agents.root).expect("database");
  db.record_sharing_state(Some(MemberRole::Member), None, None)
    .expect("the role");
  db.set_server_admin(false).expect("no server admin");
  drop(db);

  open(&mut team.agents, PipelinesFilter::All, Some(id));
  team.agents.until("the member's hub", |s| {
    s.exists("pipeline-hub-panel") && !s.exists("pipeline-hub-new")
  });
  assert_eq!(team.agents.text("team-hub-panel-title"), "Delivery");
  assert!(
    team.agents.exists("pipeline-hub-versions-button"),
    "a member reads the versions"
  );
  for gone in ["pipeline-hub-more", SAVE, "pipeline-hub-cancel"] {
    assert!(!team.agents.exists(gone), "{gone}: a member changes nothing");
  }
}
