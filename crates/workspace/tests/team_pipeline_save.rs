//! A team pipeline's save waits for the team server (PV2-34, D-042, D-043), against the stand-in team server: a new
//! pipeline, the next version and a rename are sent at once and kept here only once the server took them; a refusal, a
//! version someone published first, an old revision or a server that cannot be reached store nothing and say why; and
//! nothing of a pipeline waits in the outbox. The editor's save renames first and says when only the name was saved.
//!
//! `cargo test -p orchester-workspace --test team_pipeline_save team_pipeline_save`

#[path = "startup_join/support.rs"]
#[allow(dead_code, reason = "the joins' desktop, of which these tests use a part")]
mod support;

use std::path::PathBuf;

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TeamServer, USERNAME},
};
use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  name::Name,
  project::ProjectKey,
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing, SharingLink};
use orchester_workspace::{
  pipelines::{
    data::OpenedPipeline,
    write::{Allowed, Stored, WriteError, save_named_in},
  },
  sharing::{
    CodeRequest, NotTaken, Servers, Trust,
    team_edit::{Local, Refusal, Rejection},
    team_join::JoinOutcome,
  },
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
      .run(tokio::task::spawn_blocking(move || work(servers, root)))
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

  fn pipelines_here(&self) -> usize {
    self.db().resources(ResourceKind::Pipeline).expect("read").len()
  }

  /// Nothing of a pipeline waits in the outbox.
  fn assert_no_pipeline_waits(&self) {
    let waiting = self.db().waiting_publishes().expect("the outbox");
    assert!(
      waiting.iter().all(|waiting| waiting.kind != ResourceKind::Pipeline),
      "{waiting:?}"
    );
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

/// The team tool *Cargo*, new here and on its way to the team.
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
  db.create_published(slug("cargo"), name("Cargo"), &tool, Timestamp::now())
    .expect("the tool")
    .0
    .id()
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
  team.assert_no_pipeline_waits();
}

#[test]
fn team_pipeline_save_publish_keeps_the_version_the_server_numbered() {
  let team = Team::owner();
  let id = team.created();

  let published = team.publish(id, 1, "Changed.");

  assert_eq!(published, Ok(number(2)));
  assert_eq!(team.latest(id), (2, "Changed.".to_owned()));
  assert_eq!(team.on_server(id).map(|(versions, _)| versions), Some(2));
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
  team.assert_no_pipeline_waits();
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
