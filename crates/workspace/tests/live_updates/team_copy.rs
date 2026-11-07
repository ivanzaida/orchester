//! An Admin shares one of their own local pipelines for the team (`docs/architecture/server.md` row 80, the app MCP
//! tool `share_pipeline`): its newest version becomes a new team pipeline's first version, sent through the shared
//! configuration's publishing like any first version, which creates it on the server. Once the server took it, the team
//! project offers it and a team task may name it; the local pipeline stays hidden under a slug of its own.

use std::collections::BTreeSet;

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey,
    StepKind,
  },
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing};
use orchester_workspace::{
  sharing::{
    SharingError,
    project::convert::{shared_pipeline_id, team_pipeline_id},
  },
  tasks::data::published_pipelines,
  team_reads::listed::TeamPipelines,
};

use super::{ALICE, LIVE, Member, TeamServer, config::builder, publish::Computer, seen_within};

/// One step, Implement, run by `profile`.
pub(super) fn one_step(profile: ResourceId) -> Pipeline {
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: Name::new("Implement").expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Say hello.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step]).expect("pipeline")
}

/// An agent profile the team server has, cached on `alice`'s computer: a team pipeline may name only these.
pub(super) async fn team_profile(server: &TeamServer, alice: &Computer) -> ResourceId {
  let profile = ResourceId::generate();
  let cli = ReferenceTarget::CliKind {
    cli: AgentProvider::CodexCli,
  };
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(ConnectionId::generate(), &[]),
    vec![cli],
  );
  let stream = alice.workspace.follow(Member::of(server, ALICE));
  seen_within(LIVE, "the profile cached here", || {
    let profiles = alice.workspace.db().resources(ResourceKind::AgentProfile);
    profiles.is_ok_and(|found| found.iter().any(|kept| kept.id() == profile))
  })
  .await;
  stream.abort();
  profile
}

/// An agent profile of this computer's own, hidden in the team project: not one the team server has.
pub(super) fn own_profile(db: &mut ProjectDb) -> ResourceId {
  let body = AgentProfile::new(
    Name::new("Scratch").expect("name"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("haiku").expect("model"),
    ProviderSettings::default(),
  );
  let id = db
    .create_resource(
      Slug::new("scratch").expect("slug"),
      Name::new("Scratch").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("a profile")
    .id();
  db.set_resource_sharing(id, Some(ResourceSharing::NotShared))
    .expect("kept on this computer");
  id
}

/// A pipeline of this computer's own, kept on it when the project joined, at v1, run by `profile`.
pub(super) fn own_pipeline(db: &mut ProjectDb, profile: ResourceId) -> ResourceId {
  let body = one_step(profile);
  let id = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("a pipeline")
    .id();
  db.set_resource_sharing(id, Some(ResourceSharing::NotShared))
    .expect("kept on this computer");
  db.publish_draft::<Pipeline>(id, Timestamp::now()).expect("v1 here");
  id
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_admins_own_pipeline_shared_for_the_team_becomes_a_team_pipeline() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let profile = team_profile(&server, &alice).await;
  let local = own_pipeline(&mut alice.workspace.db(), profile);
  let offered = |db: &ProjectDb| -> Vec<ResourceId> {
    published_pipelines(db)
      .expect("the pipelines")
      .iter()
      .map(|choice| choice.id)
      .collect()
  };
  assert!(
    offered(&alice.workspace.db()).is_empty(),
    "a team project hides its own"
  );

  let copy = alice
    .workspace
    .db()
    .copy_for_the_team::<Pipeline>(local, Timestamp::now())
    .expect("copied for the team")
    .id();
  assert_eq!(
    offered(&alice.workspace.db()),
    [copy],
    "the team's, waiting for the server"
  );
  alice.store(&server, |store| store.send_waiting()).await.expect("sent");

  let db = alice.workspace.db();
  assert!(matches!(
    db.resource_sharing(copy),
    Ok(Some(ResourceSharing::Shared(_)))
  ));
  assert_eq!(
    server.versions_of(copy).len(),
    1,
    "created on the server with its first version"
  );
  assert!(
    TeamPipelines::of(&db).expect("read").contains(copy),
    "a team task may name it"
  );
  assert_eq!(offered(&db), [copy]);
  let kept = db.resource(local).expect("read").expect("kept");
  assert_eq!(kept.slug().as_str(), "delivery-local");
  assert_eq!(
    db.resource(copy).expect("read").expect("the copy").slug().as_str(),
    "delivery"
  );
  assert!(matches!(
    db.resource_sharing(local),
    Ok(Some(ResourceSharing::NotShared))
  ));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_member_cannot_copy_a_pipeline_for_the_team() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  // Made while the person was its owner; a member now.
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let profile = team_profile(&server, &alice).await;
  let local = own_pipeline(&mut alice.workspace.db(), profile);
  rusqlite::Connection::open(alice.workspace.root.join(".orchester/orchester.db"))
    .expect("open")
    .execute("UPDATE sharing SET role = 'member'", [])
    .expect("a member now");
  let refused = alice
    .workspace
    .db()
    .copy_for_the_team::<Pipeline>(local, Timestamp::now());
  assert!(
    matches!(refused, Err(orchester_store::StoreError::NotAdmin)),
    "{refused:?}"
  );
  assert!(alice.workspace.db().waiting_publishes().expect("read").is_empty());
}

/// A task goes up naming its pipeline by its id (row 80): when the project is shared every pipeline with a version goes
/// up with its id first, so it is that id; a task of a team project names only a team pipeline, and any other is none.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_task_goes_up_naming_its_team_pipeline_by_id() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let profile = team_profile(&server, &alice).await;
  let local = own_pipeline(&mut alice.workspace.db(), profile);
  let team = alice
    .workspace
    .db()
    .copy_for_the_team::<Pipeline>(local, Timestamp::now())
    .expect("copied for the team")
    .id();
  alice.store(&server, |store| store.send_waiting()).await.expect("sent");

  let db = alice.workspace.db();
  assert_eq!(team_pipeline_id(&db, Some(team)).expect("read"), Some(team.to_string()));
  assert_eq!(
    team_pipeline_id(&db, Some(local)).expect("read"),
    None,
    "never a local id"
  );
  assert_eq!(team_pipeline_id(&db, None).expect("read"), None);
  assert_eq!(
    shared_pipeline_id(&db, Some(local)).expect("read"),
    Some(local.to_string()),
    "Share uploads it with its id"
  );
  let gone = ResourceId::generate();
  assert_eq!(
    shared_pipeline_id(&db, Some(gone)).expect("read"),
    None,
    "none it does not upload"
  );
}

/// In a team project only what the team server has live is offered: an archived team pipeline leaves every list and
/// comes back once restored; a pipeline the server refused to take, and an agent profile of this computer's own, are
/// offered nowhere, kept as they are.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_team_project_offers_only_what_its_server_has_live() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let profile = team_profile(&server, &alice).await;
  let local = own_pipeline(&mut alice.workspace.db(), profile);
  let team = alice
    .workspace
    .db()
    .copy_for_the_team::<Pipeline>(local, Timestamp::now())
    .expect("copied for the team")
    .id();
  alice.store(&server, |store| store.send_waiting()).await.expect("sent");
  let offered = |db: &ProjectDb| -> Vec<ResourceId> {
    published_pipelines(db)
      .expect("the pipelines")
      .iter()
      .map(|choice| choice.id)
      .collect()
  };

  let mut db = alice.workspace.db();
  db.mark_shared_archived(team, true, Timestamp::now()).expect("archived");
  assert!(offered(&db).is_empty(), "an archived team pipeline is offered nowhere");
  assert!(!TeamPipelines::of(&db).expect("read").contains(team));
  db.mark_shared_archived(team, false, Timestamp::now())
    .expect("restored");
  assert_eq!(offered(&db), [team], "restored, it is back");

  // An Admin's pipeline the server refused (its slug taken there, say) never became the team's.
  let refused = db
    .create_resource(
      Slug::new("hotfix").expect("slug"),
      Name::new("Hotfix").expect("name"),
      &one_step(ResourceId::generate()),
      Timestamp::now(),
    )
    .expect("a pipeline")
    .id();
  db.publish_draft::<Pipeline>(refused, Timestamp::now())
    .expect("published here");
  assert_eq!(offered(&db).len(), 2, "waiting for the server, it is the team's");
  let waiting = db.waiting_publishes().expect("read");
  let version = waiting
    .iter()
    .find(|waiting| waiting.resource == refused)
    .expect("waits")
    .version;
  db.record_publish_refused(version, "slug_taken").expect("refused");
  assert_eq!(offered(&db), [team]);
  assert!(
    db.kept_resources(orchester_core::resource::ResourceKind::Pipeline)
      .expect("read")
      .iter()
      .any(|kept| kept.id() == refused),
    "kept as it is"
  );
}

/// A team pipeline names its profiles by their ids on the server: one that runs a profile only this computer keeps is
/// refused, naming it, and nothing is sent (`docs/design/team-configuration.md`, "Per resource kind").
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_team_pipeline_running_a_profile_the_team_does_not_have_is_not_sent() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let own = own_profile(&mut alice.workspace.db());
  let pipeline = alice
    .workspace
    .db()
    .create_resource(
      Slug::new("hotfix").expect("slug"),
      Name::new("Hotfix").expect("name"),
      &one_step(own),
      Timestamp::now(),
    )
    .expect("a pipeline")
    .id();
  alice
    .workspace
    .db()
    .publish_draft::<Pipeline>(pipeline, Timestamp::now())
    .expect("published here");

  let refused = alice.store(&server, move |store| store.send_created(pipeline)).await;

  assert!(
    matches!(&refused, Err(SharingError::PipelineNamesNonTeam { resource, name }) if *resource == pipeline && name == "Scratch"),
    "{refused:?}"
  );
  assert!(server.versions_of(pipeline).is_empty(), "nothing reached the server");
}
