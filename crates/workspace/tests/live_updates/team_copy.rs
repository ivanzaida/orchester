//! An Admin shares one of their own local pipelines for the team (`docs/architecture/server.md` row 80, the app MCP
//! tool `share_pipeline`): its newest version becomes a new team pipeline's first version, sent through the shared
//! configuration's publishing like any first version, which creates it on the server. Once the server took it, the team
//! project offers it and a team task may name it; the local pipeline stays hidden under a slug of its own.

use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Pipeline, PromptText, RepositoryAccess, Slug, Step, StepKey, StepKind},
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing};
use orchester_workspace::{tasks::data::published_pipelines, team_reads::listed::TeamPipelines};

use super::{ALICE, TeamServer, publish::Computer};

/// One step, Implement, run by `profile`.
fn one_step(profile: ResourceId) -> Pipeline {
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

/// A pipeline of this computer's own, kept on it when the project joined, at v1.
fn own_pipeline(db: &mut ProjectDb) -> ResourceId {
  let body = one_step(ResourceId::generate());
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
  let local = own_pipeline(&mut alice.workspace.db());
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
  let local = own_pipeline(&mut alice.workspace.db());
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
