//! An admin's change of a team resource, sent straight to the team server (`docs/design/team-configuration-live.md`,
//! rule 6), against the fake team server: a new version is published on the version it was made on and memory holds
//! the server's answer; one made on an older version is refused and memory takes the server's latest; a new resource
//! is created there. Nothing waits on this computer: a member, a server that refuses it and a server that cannot be
//! reached leave everything as it was, and nothing of it is written to the project's database.

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{ResourceKind, VersionNumber},
  sharing::{MemberRole, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  messages::Message,
  sharing::team_edit::{EditRefused, NewTeamResource, Publication, Refusal},
  team_config::{TeamConfig, TeamResource, TeamVersion},
};

use super::{
  ALICE, TeamServer,
  desktop::Desktop,
  publish::{json, number},
  support::Fixture,
};

/// The team's instruction Style, which the server has at v1.
fn style(server: &TeamServer) -> ResourceId {
  let id = ResourceId::generate();
  server.publish(
    (id, SharedResourceKind::Instruction, "Style"),
    json("Use tabs."),
    Vec::new(),
  );
  id
}

/// Alice's desktop, the project's owner where `admin`, holding `style` at version `held` as a read of the server would.
fn desktop(server: &TeamServer, (style, held): (ResourceId, u32), admin: bool) -> Desktop {
  let desktop = Desktop::signed_in(server, ALICE, Fixture::new(), &[]);
  let role = if admin { MemberRole::Owner } else { MemberRole::Member };
  ProjectDb::open(desktop.root())
    .expect("the project")
    .record_sharing_state(Some(role), None, None)
    .expect("the role");
  let resource = TeamResource {
    id: style,
    kind: ResourceKind::Instruction,
    name: Name::new("Style").expect("name"),
    archived: false,
    revision: TaskRevision::try_from(1).expect("a revision"),
    updated: orchester_workspace::team_config::Changed {
      by: "Alice Moreau".to_owned(),
      at: orchester_core::time::Timestamp::from_unix_millis(0),
    },
    latest: Some(TeamVersion {
      number: number(held),
      body: json("Use tabs."),
      published_by: "Alice Moreau".to_owned(),
      published_at: Timestamp::from_unix_millis(1),
    }),
  };
  desktop
    .workspace
    .keep_team_config_for_test(desktop.root(), TeamConfig::new([resource]));
  desktop
}

/// The latest version of `id` memory holds.
fn held(desktop: &Desktop, id: ResourceId) -> Option<TeamVersion> {
  desktop
    .workspace
    .team_read(desktop.root())
    .config()
    .and_then(|config| config.resource(id).cloned())
    .and_then(|resource| resource.latest)
}

fn publication(text: &str) -> Publication {
  Publication {
    body: json(text),
    references: Vec::new(),
  }
}

fn no_instruction_written(desktop: &Desktop) {
  let db = ProjectDb::open(desktop.root()).expect("the project");
  assert!(db.resources(ResourceKind::Instruction).expect("read").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_admins_save_is_published_on_the_server_at_once_and_memory_holds_it() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let style = style(&server);
  let desktop = desktop(&server, (style, 1), true);

  let published = desktop
    .servers
    .publish_team(desktop.root(), style, (Some(number(1)), publication("Use spaces.")))
    .await
    .expect("published");

  assert_eq!(published, number(2));
  let versions = server.versions_of(style);
  assert_eq!(versions.len(), 2);
  assert_eq!(versions[1].based_on, Some(number(1)));
  let kept = held(&desktop, style).expect("held");
  assert_eq!((kept.number, kept.body), (number(2), json("Use spaces.")));
  no_instruction_written(&desktop);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_save_on_an_older_version_is_refused_and_memory_takes_the_latest() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let style = style(&server);
  server.publish(
    (style, SharedResourceKind::Instruction, "Style"),
    json("Use spaces."),
    Vec::new(),
  );
  let desktop = desktop(&server, (style, 1), true);

  let refused = desktop
    .servers
    .publish_team(desktop.root(), style, (Some(number(1)), publication("Use both.")))
    .await
    .expect_err("someone published first");

  assert_eq!(refused, EditRefused::Changed { latest: number(2) });
  assert_eq!(server.versions_of(style).len(), 2, "nothing was published");
  let kept = held(&desktop, style).expect("held");
  assert_eq!(kept.number, number(2), "the panel reloads from the server's latest");
  assert_eq!(kept.body, json("Use spaces."));

  let stale = desktop
    .servers
    .publish_team(desktop.root(), style, (Some(number(1)), publication("Use both.")))
    .await
    .expect_err("made on an older version than memory holds");
  assert_eq!(stale, EditRefused::Changed { latest: number(2) });
  assert_eq!(server.versions_of(style).len(), 2, "this computer sent nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_new_team_resource_is_created_on_the_server_and_memory_holds_it() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let style = style(&server);
  let desktop = desktop(&server, (style, 1), true);
  let id = ResourceId::generate();

  let created = desktop
    .servers
    .create_team(
      desktop.root(),
      NewTeamResource {
        id,
        kind: ResourceKind::Instruction,
        name: Name::new("Tests first").expect("name"),
        publication: publication("Write the failing test first."),
      },
    )
    .await
    .expect("created");

  assert_eq!(created, id);
  assert_eq!(server.versions_of(id).len(), 1);
  assert_eq!(held(&desktop, id).expect("held").number, VersionNumber::FIRST);
  no_instruction_written(&desktop);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_nothing_is_saved_for_a_member_a_refusal_or_an_unreachable_server() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let style = style(&server);
  let save = |desktop: &Desktop| {
    let servers = desktop.servers.clone();
    let root = desktop.root().to_owned();
    async move {
      servers
        .publish_team(&root, style, (Some(number(1)), publication("Use spaces.")))
        .await
    }
  };

  let member = desktop(&server, (style, 1), false);
  assert_eq!(
    save(&member).await,
    Err(EditRefused::Refused(Refusal::NotAdminHere)),
    "this computer knows they are no admin"
  );

  server.make_admin(ALICE);
  server.refuse_versions(true);
  let admin = desktop(&server, (style, 1), true);
  assert_eq!(save(&admin).await, Err(EditRefused::Refused(Refusal::NotAdmin)));
  server.refuse_versions(false);

  let reason = Message::new("sharing.errors.offline");
  admin.workspace.team_unreached_for_test(admin.root(), reason.clone());
  assert_eq!(save(&admin).await, Err(EditRefused::Unreachable(reason)));

  assert_eq!(
    server.versions_of(style).len(),
    1,
    "nothing reached the server's versions"
  );
  assert_eq!(held(&admin, style).expect("held").number, number(1));
}
