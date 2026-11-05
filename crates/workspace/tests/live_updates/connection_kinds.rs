//! Connection references choose only by CLI kind and stale choices are unbound.

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{AgentProfile, VersionNumber},
  run::Answerer,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{Bound, BoundBy, BoundReference};
use orchester_workspace::sharing::{
  SharingError,
  sync::{Here, bind},
};

use super::{
  ALICE, LIVE, Member, TeamServer, Workspace,
  config::{builder, codex, reference, use_first},
  seen_within,
};

fn claude(workspace: &Workspace, id: ConnectionId, name: &str) {
  let connection = AgentConnection::new(
    id,
    Name::new(name).expect("name"),
    AgentProvider::ClaudeCodeCli,
    workspace.root.join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute path")
  .into_connection();
  workspace.db().create_connection(&connection).expect("created");
}

fn publish(server: &TeamServer, profile: ResourceId, theirs: ConnectionId) {
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(theirs, &[]),
    vec![reference(
      ReferenceTarget::Connection { id: theirs },
      "Work",
      Some("codex_cli"),
    )],
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_connection_kind_auto_binding_ignores_names() {
  for (codex_count, other_same_name) in [(0, false), (1, false), (2, false), (0, true), (1, true)] {
    let server = TeamServer::start(&[(ALICE, "Alice")], &[]);
    let workspace = Workspace::linked(&server);
    let mut locals = Vec::new();
    for index in 0..codex_count {
      let name = if index == 0 && !other_same_name {
        "Work"
      } else {
        "Laptop"
      };
      locals.push(codex(&mut workspace.db(), &workspace.root, name));
    }
    if other_same_name {
      claude(&workspace, ConnectionId::generate(), "Work");
    }
    let (profile, theirs) = (ResourceId::generate(), ConnectionId::generate());
    publish(&server, profile, theirs);
    let stream = workspace.follow(Member::of(&server, ALICE));
    use_first(&workspace, profile).await;
    let bindings = workspace.db().reference_bindings().expect("bindings");
    let expected = if locals.len() == 1 {
      Some(Bound {
        local: locals[0],
        by: BoundBy::Role,
      })
    } else {
      None
    };
    assert_eq!(
      bindings[0].reference,
      BoundReference::Connection {
        reference: theirs,
        bound: expected
      }
    );
    stream.abort();
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_connection_kind_bind_refuses_mismatch_and_changed_provider_is_unbound() {
  let server = TeamServer::start(&[(ALICE, "Alice")], &[]);
  let workspace = Workspace::linked(&server);
  let local = codex(&mut workspace.db(), &workspace.root, "Laptop");
  let other = ConnectionId::generate();
  claude(&workspace, other, "Work");
  let (profile, theirs) = (ResourceId::generate(), ConnectionId::generate());
  publish(&server, profile, theirs);
  let stream = workspace.follow(Member::of(&server, ALICE));
  use_first(&workspace, profile).await;
  let choice = |local| BoundReference::Connection {
    reference: theirs,
    bound: Some(Bound {
      local,
      by: BoundBy::Person,
    }),
  };
  assert!(matches!(
    bind(&workspace.root, choice(other), Answerer::Person),
    Err(SharingError::ConnectionKindMismatch { .. })
  ));
  assert_eq!(
    workspace.db().reference_bindings().expect("bindings")[0].reference,
    BoundReference::Connection {
      reference: theirs,
      bound: Some(Bound {
        local,
        by: BoundBy::Role
      })
    }
  );
  bind(&workspace.root, choice(local), Answerer::Person).expect("same kind binds");
  // The store keeps a connection's provider immutable; simulate a changed row from an older writer.
  rusqlite::Connection::open(workspace.root.join(".orchester/orchester.db"))
    .expect("database")
    .execute(
      "UPDATE connections SET provider = 'claude_code_cli' WHERE id = ?1",
      [local.to_string()],
    )
    .expect("changed provider");
  let here = Here::read(&workspace.root).expect("here");
  assert!(here.references[0].is_missing());
  assert_eq!(
    here.references[0].binding.reference,
    BoundReference::Connection {
      reference: theirs,
      bound: None
    }
  );
  publish(&server, profile, theirs);
  seen_within(LIVE, "v2 cached", || {
    workspace
      .db()
      .cached_version::<AgentProfile>(
        profile,
        VersionNumber::new(std::num::NonZeroU32::new(2).expect("positive")),
      )
      .expect("cached")
      .is_some()
  })
  .await;
  let cached = workspace
    .db()
    .cached_version::<AgentProfile>(
      profile,
      VersionNumber::new(std::num::NonZeroU32::new(2).expect("positive")),
    )
    .expect("read")
    .expect("v2");
  assert_eq!(cached.local_body.connection(), theirs, "stale binding is not resolved");
  stream.abort();
}
