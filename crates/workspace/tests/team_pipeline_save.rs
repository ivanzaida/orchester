//! A team pipeline's save waits for the team server (PV2-34, D-042, D-043), against the stand-in team server: a new
//! pipeline, the next version and a rename are sent at once and kept here only once the server took them; a refusal, a
//! version someone published first, an old revision or a server that cannot be reached store nothing and say why. The
//! editor's save renames first and says when only the name was saved.
//!
//! `cargo test -p orchester-workspace --test team_pipeline_save team_pipeline_save`

#[path = "startup_join/support.rs"]
#[allow(dead_code, reason = "the joins' desktop, of which these tests use a part")]
mod support;

use std::path::PathBuf;

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, Standing, TeamServer, USERNAME},
};
use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  name::Name,
  project::ProjectKey,
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  sharing::{MemberRole, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_store::{ProjectDb, ResourceSharing, SharingLink};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  pipelines::{
    data::OpenedPipeline,
    write::{Allowed, Stored, WriteError, save_named_in},
  },
  runner::Runners,
  sharing::{
    CodeRequest, NotTaken, Servers, Trust,
    team_edit::{Local, Refusal, Rejection},
    team_join::JoinOutcome,
  },
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  team_config::{TeamConfig, TeamResource},
};
use support::Desk;

const SLUG: &str = "kontur";

/// This computer joined to the stand-in team, with Kontur linked to the team's Kontur and a team tool *Cargo* the
/// pipelines run.
struct Team {
  server: TeamServer,
  desk: Desk,
  root: PathBuf,
  tool: ResourceId,
}

impl Team {
  /// Kontur linked as `role`; an owner changes the team's configuration, a member does not.
  fn as_role(role: MemberRole) -> Self {
    let server = TeamServer::named("Acme");
    // The server takes an admin's writes and refuses a member's (`403 not_admin`).
    server.stand_as(if role == MemberRole::Owner {
      Standing::Admin
    } else {
      Standing::Member
    });
    server.share(&[("Kontur", SLUG, None)]);
    let desk = Desk::new();
    let joined = join(&desk, &server);
    let root = desk.project("kontur", "Kontur").repo_root;
    let project = server.project(SLUG).expect("the team shares Kontur");
    link(&root, (joined, project), role);
    let tool = if role == MemberRole::Owner {
      cargo(&mut ProjectDb::open(&root).expect("the project"))
    } else {
      ResourceId::generate()
    };
    Self {
      server,
      desk,
      root,
      tool,
    }
  }

  fn owner() -> Self {
    Self::as_role(MemberRole::Owner)
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("the project")
  }

  fn servers(&self) -> Servers {
    self.desk.servers.clone()
  }

  /// Runs `work` on a worker of the runtime, as the app and the tools do.
  fn blocking<T: Send + 'static>(&self, work: impl FnOnce(Servers, PathBuf) -> T + Send + 'static) -> T {
    let (servers, root) = (self.servers(), self.root.clone());
    self
      .desk
      .run(async move { tokio::task::spawn_blocking(move || work(servers, root)).await })
      .expect("the work finished")
  }

  /// Creates the team pipeline *Checks* of one step running `tool`, described `text`.
  fn create(&self, tool: ResourceId, text: &str) -> Result<ResourceId, NotTaken> {
    let body = checks(tool, text);
    self.blocking(move |servers, root| servers.create_team_pipeline(&root, (slug("checks"), name("Checks")), &body))
  }

  /// *Checks* as the team server made it.
  fn created(&self) -> ResourceId {
    self.create(self.tool, "Ours.").expect("the server makes it")
  }

  /// Sends `text` as the description of `id`'s next version after `based_on`.
  fn publish(&self, id: ResourceId, based_on: u32, text: &str) -> Result<VersionNumber, NotTaken> {
    let body = checks(self.tool, text);
    self.blocking(move |servers, root| servers.publish_team_pipeline(&root, id, number(based_on), &body))
  }

  fn rename(&self, id: ResourceId, to: &str) -> Result<(), NotTaken> {
    let to = name(to);
    self.blocking(move |servers, root| servers.rename_team_pipeline(&root, id, &to))
  }

  /// The editor's save of `id` as it is stored now, with its steps described `text` and its name `to`.
  fn save(&self, id: ResourceId, text: &str, to: &str) -> Result<Stored, WriteError> {
    let before = OpenedPipeline::load(&self.db(), id).expect("the pipeline");
    let (body, to) = (checks(self.tool, text), name(to));
    self.blocking(move |servers, root| save_named_in(&root, &servers, &before, &body, &to, &Allowed::default()))
  }

  /// The number and description of `id`'s latest version here.
  fn latest(&self, id: ResourceId) -> (u32, String) {
    let version = self
      .db()
      .latest_version::<Pipeline>(id)
      .expect("read")
      .expect("a version");
    (version.number().get(), version.body().description().to_owned())
  }

  fn name_here(&self, id: ResourceId) -> String {
    self
      .db()
      .resource(id)
      .expect("read")
      .expect("the pipeline")
      .name()
      .as_str()
      .to_owned()
  }

  /// How many versions the team server holds of `id`, and its name there; `None` when it holds no such resource.
  fn on_server(&self, id: ResourceId) -> Option<(usize, String)> {
    self
      .server
      .resources(SLUG)
      .into_iter()
      .find(|(resource, _)| resource["id"] == id.to_string())
      .map(|(resource, versions)| {
        let name = resource["name"].as_str().unwrap_or_default().to_owned();
        (versions.len(), name)
      })
  }

  /// What the team's configuration in memory holds of `id`.
  fn held(&self, id: ResourceId) -> Option<TeamResource> {
    self
      .desk
      .workspace
      .team_read(&self.root)
      .config()
      .and_then(|config| config.resource(id).cloned())
  }

  /// The app MCP's answer to `tool` with `args`, as a person who lets automation change agent permissions calls it;
  /// a refusal fails the test.
  fn call(&self, tool: &str, args: serde_json::Value) -> serde_json::Value {
    self
      .desk
      .workspace
      .set_automation_changes_permissions(true)
      .expect("the switch");
    let services = app_mcp::Services::new(
      self.desk.workspace.clone(),
      Runners::new(
        self.root.join("no-runner"),
        self.desk.runtime.handle().clone(),
        Activity::new(),
        SessionEnd::Clean,
      ),
      self.servers(),
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    let translate = Translate::new(|message| message.key.to_owned());
    let serde_json::Value::Object(args) = args else {
      unreachable!("an object");
    };
    let answer = match self.desk.run(app_mcp::call(&services, &translate, tool, args.clone())) {
      Some(answer) => answer,
      None => self
        .desk
        .run(app_mcp::setup::call(&services, &translate, tool, args))
        .expect("a tool of the app"),
    };
    answer.unwrap_or_else(|error| panic!("{tool}: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn pipelines_here(&self) -> usize {
    self.db().resources(ResourceKind::Pipeline).expect("read").len()
  }
}

/// Joins `server` with its code; the server's id here.
fn join(desk: &Desk, server: &TeamServer) -> ServerId {
  let address = ServerAddress::parse(&server.url()).expect("the stand-in's address");
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  match desk.run(
    desk
      .join
      .join_by_code(&address, Trust::System(PlainHttp::NotAllowed), request),
  ) {
    Ok(JoinOutcome::Joined(joined)) => joined.id,
    other => panic!("not joined: {other:?}"),
  }
}

/// Links the project at `root` to the team's `project` on `server`, the person's role there `role`.
fn link(root: &std::path::Path, (server, project): (ServerId, ProjectId), role: MemberRole) {
  let link = SharingLink {
    server,
    project,
    key: ProjectKey::new("KON").expect("key"),
    role: Some(role),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  ProjectDb::open(root)
    .expect("the project")
    .start_sharing(&link, &[], (&[], &Default::default()))
    .expect("linked");
}

/// The team tool *Cargo*, as the stream keeps one the team server has.
fn cargo(db: &mut ProjectDb) -> ResourceId {
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("a program"),
      vec![],
    )),
    None,
  )
  .expect("a tool");
  let id = db
    .create_published(slug("cargo"), name("Cargo"), &tool, Timestamp::now())
    .expect("the tool")
    .0
    .id();
  // The team's, as the stream keeps what the team server has: nothing waits to be sent any more.
  db.set_resource_sharing(id, Some(ResourceSharing::Shared(TaskRevision::FIRST)))
    .expect("the team's tool");
  id
}

/// A pipeline of one step running `tool`, described `text`.
fn checks(tool: ResourceId, text: &str) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty body"));
  let key = edit.add_step(name("Check"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  edit.set_description(text);
  edit.validate().expect("a valid pipeline")
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn slug(text: &str) -> Slug {
  Slug::new(text).expect("a slug")
}

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(std::num::NonZeroU32::new(value).expect("a version number"))
}

#[test]
fn team_pipeline_save_create_keeps_v1_only_once_the_server_made_it() {
  let team = Team::owner();

  let id = team.create(team.tool, "Ours.").expect("the server makes it");

  assert!(matches!(
    team.db().resource_sharing(id).expect("read"),
    Some(ResourceSharing::Shared(_))
  ));
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()));
  assert_eq!(team.on_server(id), Some((1, "Checks".to_owned())));
}

#[test]
fn team_pipeline_save_publish_keeps_the_version_the_server_numbered() {
  let team = Team::owner();
  let id = team.created();

  let published = team.publish(id, 1, "Changed.");

  assert_eq!(published, Ok(number(2)));
  assert_eq!(team.latest(id), (2, "Changed.".to_owned()));
  assert_eq!(team.on_server(id).map(|(versions, _)| versions), Some(2));
}

#[test]
fn team_pipeline_save_a_refusal_stores_nothing_and_says_why() {
  let team = Team::owner();
  let id = team.created();
  team.server.reject_content(SLUG, "body");

  let refused = team.publish(id, 1, "Changed.").expect_err("refused");

  assert_eq!(refused, NotTaken::Refused(Refusal::Rejected(Rejection::Body)));
  assert_eq!(refused.message().key, "sharing.edit.refused.body");
  assert_eq!(refused.code(), "team_server_refused");
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()), "nothing is stored");
  assert_eq!(team.on_server(id).map(|(versions, _)| versions), Some(1));
  let new = team.create(team.tool, "Another.");
  assert!(matches!(new, Err(NotTaken::Refused(_))), "{new:?}");
  assert_eq!(team.pipelines_here(), 1, "a refused new pipeline is not kept");
}

#[test]
fn team_pipeline_save_an_unreachable_server_stores_nothing() {
  let team = Team::owner();
  let id = team.created();
  team.server.set_down(true);

  let refused = team.publish(id, 1, "Changed.").expect_err("not sent");
  let new = team.create(team.tool, "Another.").expect_err("not sent");

  for not_sent in [&refused, &new] {
    assert!(matches!(not_sent, NotTaken::Unreached(_)), "{not_sent:?}");
    assert_eq!(not_sent.message().key, "pipelines.problems.teamUnreached");
    assert_eq!(not_sent.code(), "team_server_not_reached");
  }
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()));
  assert_eq!(team.pipelines_here(), 1);
}

#[test]
fn team_pipeline_save_a_version_someone_published_first_is_refused_naming_it() {
  let team = Team::owner();
  let id = team.created();
  let theirs = serde_json::to_value(checks(team.tool, "Theirs.")).expect("a body");
  team.server.publish_as(SLUG, id, theirs, "olha");

  let refused = team.publish(id, 1, "Mine.").expect_err("refused");

  assert_eq!(
    refused,
    NotTaken::Changed {
      latest: Some(number(2))
    }
  );
  let message = refused.message();
  assert_eq!(message.key, "pipelines.problems.teamChanged");
  assert_eq!(refused.code(), "version_changed");
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()), "mine is not stored");
}

#[test]
fn team_pipeline_save_a_rename_goes_to_the_server() {
  let team = Team::owner();
  let id = team.created();

  team.rename(id, "Gates").expect("renamed");

  assert_eq!(team.name_here(id), "Gates");
  assert_eq!(team.on_server(id), Some((1, "Gates".to_owned())));
  assert_eq!(team.latest(id).0, 1, "a rename publishes no version");
  team.rename(id, "Gates again").expect("renamed on the revision it kept");
  assert_eq!(team.on_server(id).map(|(_, name)| name).as_deref(), Some("Gates again"));
}

/// Memory, which the hub and readiness read, holds what the server took at once, before the stream brings it.
#[test]
fn team_pipeline_save_memory_holds_what_the_server_took_at_once() {
  let team = Team::owner();
  // The team's configuration as a read at the stream's open left it; no stream runs, so only the saves change it.
  team
    .desk
    .workspace
    .keep_team_config_for_test(&team.root, TeamConfig::new([]));
  let id = team.created();
  let made = team.held(id).expect("memory holds the new pipeline");
  assert_eq!((made.kind, made.number()), (ResourceKind::Pipeline, Some(number(1))));

  team.publish(id, 1, "Changed.").expect("published");
  team.rename(id, "Gates").expect("renamed");

  let changed = team.held(id).expect("memory holds it");
  assert_eq!(changed.number(), Some(number(2)), "the published version");
  assert_eq!(changed.name.as_str(), "Gates", "the new name");
}

#[test]
fn team_pipeline_save_a_rename_on_an_old_revision_is_refused_with_why() {
  let team = Team::owner();
  let id = team.created();
  team.server.archive(SLUG, id);

  let refused = team.rename(id, "Gates").expect_err("refused");

  assert_eq!(refused, NotTaken::Changed { latest: None });
  assert_eq!(refused.message().key, "pipelines.problems.teamRenameChanged");
  assert_eq!(team.name_here(id), "Checks", "the name stays");
}

#[test]
fn team_pipeline_save_the_editor_saves_the_name_and_the_steps() {
  let team = Team::owner();
  let id = team.created();

  let stored = team.save(id, "Changed.", "Gates").expect("saved");

  assert_eq!(stored.steps_not_saved, None);
  assert_eq!((&*stored.opened.name, stored.opened.version), ("Gates", number(2)));
  assert_eq!(team.on_server(id), Some((2, "Gates".to_owned())));
}

#[test]
fn team_pipeline_save_the_editor_says_when_only_the_name_was_saved() {
  let team = Team::owner();
  let id = team.created();
  // The content check refuses versions only: the rename, sent first, is taken.
  team.server.reject_content(SLUG, "body");

  let stored = team.save(id, "Changed.", "Gates").expect("the name is saved");

  assert_eq!(
    stored.steps_not_saved.map(|why| why.key),
    Some("sharing.edit.refused.body")
  );
  assert_eq!((&*stored.opened.name, stored.opened.version), ("Gates", number(1)));
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()), "the steps are not stored");
}

#[test]
fn team_pipeline_save_the_editors_refusal_keeps_everything_as_it_was() {
  let team = Team::owner();
  let id = team.created();
  team.server.set_down(true);

  let refused = team.save(id, "Changed.", "Checks").expect_err("not sent");

  assert_eq!(refused.message().key, "pipelines.problems.teamUnreached");
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()));
  assert_eq!(team.name_here(id), "Checks");
}

#[test]
fn team_pipeline_save_a_member_sends_nothing() {
  let team = Team::as_role(MemberRole::Member);

  let refused = team.create(team.tool, "Ours.").expect_err("refused here");

  assert_eq!(refused, NotTaken::Refused(Refusal::NotAdminHere));
  assert_eq!(refused.code(), "not_admin");
  assert!(team.server.resources(SLUG).is_empty(), "nothing reached the server");
  assert_eq!(team.pipelines_here(), 0);
}

#[test]
fn team_pipeline_save_a_step_running_what_the_team_lacks_is_not_sent() {
  let team = Team::owner();

  let refused = team.create(ResourceId::generate(), "Ours.").expect_err("refused here");

  assert_eq!(refused, NotTaken::Refused(Refusal::Local(Local::NamesNonTeam)));
  assert_eq!(refused.code(), "pipeline_names_non_team");
  assert!(team.server.resources(SLUG).is_empty(), "nothing reached the server");
  assert_eq!(team.pipelines_here(), 0);
}

/// `share_pipeline` makes a pipeline of this computer's own, hidden in the team project, a team pipeline on the team
/// server at once: the copy takes the hidden one's slug, which stays this computer's, and nothing waits.
#[test]
fn team_pipeline_save_share_pipeline_makes_the_team_copy_beside_the_hidden_local_one() {
  let team = Team::owner();
  let (local, _) = team
    .db()
    .kept_pipeline_for_test("delivery", "Delivery", &checks(team.tool, "Ours."), Timestamp::now())
    .expect("a pipeline of this computer's own");
  let shared = team.call(
    "share_pipeline",
    serde_json::json!({"project": "Kontur", "pipeline": "Delivery"}),
  );

  assert_eq!(shared["pipeline"]["state"], "team");
  assert_eq!(shared["pipeline"]["slug"], "delivery");
  let copy: ResourceId = shared["pipeline"]["id"]
    .as_str()
    .and_then(|id| id.parse().ok())
    .expect("the copy's id");
  assert_ne!(copy, local, "a new team pipeline");
  let db = team.db();
  assert!(matches!(
    db.resource_sharing(copy).expect("read"),
    Some(ResourceSharing::Shared(_))
  ));
  assert_eq!(
    db.resource_sharing(local).expect("read"),
    Some(ResourceSharing::NotShared)
  );
  let kept = db.resource(local).expect("read").expect("the hidden one stays");
  assert_eq!(kept.slug().as_str(), "delivery", "it keeps its slug");
  assert_eq!(team.latest(copy), (1, "Ours.".to_owned()));
  assert_eq!(team.on_server(copy), Some((1, "Delivery".to_owned())));
}

/// `duplicate_pipeline` in a team project is not refused as a draft tool: the copy is made on the team server at once
/// and kept here once the server made it (D-043).
#[test]
fn team_pipeline_save_duplicate_pipeline_makes_the_copy_on_the_team_server() {
  let team = Team::owner();
  let source = team.created();

  let copied = team.call(
    "duplicate_pipeline",
    serde_json::json!({"project": "Kontur", "pipeline": "Checks", "name": "Checks copy"}),
  );

  let copy: ResourceId = copied["pipeline"]["id"]
    .as_str()
    .and_then(|id| id.parse().ok())
    .expect("the copy's id");
  assert_ne!(copy, source);
  assert_eq!(team.on_server(copy), Some((1, "Checks copy".to_owned())));
  assert!(matches!(
    team.db().resource_sharing(copy).expect("read"),
    Some(ResourceSharing::Shared(_))
  ));
  assert_eq!(team.latest(copy), (1, "Ours.".to_owned()));
}

/// A save sent again after its answer was lost meets its own version on the server: it says the changes are there
/// already, not that someone published first, and stores nothing more.
#[test]
fn team_pipeline_save_a_retry_that_meets_its_own_earlier_send_says_it_reached_the_server() {
  let team = Team::owner();
  let id = team.created();
  // The earlier send reached the server, which numbered it v2; its answer never came back here.
  let mine = serde_json::to_value(checks(team.tool, "Changed.")).expect("a body");
  team.server.publish_as(SLUG, id, mine, "bohdan");

  let again = team.publish(id, 1, "Changed.").expect_err("not sent twice");

  assert_eq!(again, NotTaken::Reached { version: number(2) });
  assert_eq!(again.message().key, "pipelines.problems.teamAlreadySaved");
  assert_eq!(again.code(), "already_on_server");
  assert_eq!(
    team.on_server(id).map(|(versions, _)| versions),
    Some(2),
    "no third version"
  );
  assert_eq!(team.latest(id), (1, "Ours.".to_owned()), "the stream brings v2");
}

/// A new pipeline whose name the team has already says an earlier *Create* that got no answer may have made it: the
/// server cannot tell this computer whose it is.
#[test]
fn team_pipeline_save_a_create_whose_name_the_team_has_says_an_earlier_create_may_have_made_it() {
  let team = Team::owner();
  team.created();

  let again = team.create(team.tool, "Ours.").expect_err("the name is taken");

  assert_eq!(again, NotTaken::Refused(Refusal::NameTaken));
  assert_eq!(again.message().key, "pipelines.problems.teamNameTaken");
  assert_eq!(team.pipelines_here(), 1, "nothing more is kept");
}

#[test]
fn team_pipeline_save_a_pipeline_the_server_no_longer_has_is_said_in_words() {
  let gone = NotTaken::Refused(Refusal::of_code("not_found"));

  assert_eq!(gone.message().key, "pipelines.problems.teamGone");
  assert_eq!(gone.code(), "team_server_refused");
}
