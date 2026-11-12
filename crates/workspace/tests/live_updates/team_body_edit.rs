//! An admin's Save of a team instruction, skill or tool in the hub's panel (`docs/design/team-configuration-live.md`,
//! rule 6; `Agents · v2` A27–A29, A30), against the fake team server: the body the fields make is published as the
//! next version on the version the panel was opened on, and memory holds it as what the panel shows next; a Save on an
//! older version is refused and what the panel reloads from is the server's latest.

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{CommandAction, CommandProgram, InstructionScope, ResourceKind, Skill, Tool},
  sharing::{MemberRole, TaskRevision},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::team_body_edit::{self, Latest},
  sharing::team_edit::EditRefused,
  team_config::{TeamConfig, TeamResource, TeamVersion},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{
  ALICE, TeamServer,
  desktop::Desktop,
  publish::{number, rule},
  support::Fixture,
};

fn value(body: &impl Serialize) -> Value {
  serde_json::to_value(body).expect("a body serializes")
}

fn skill(description: &str) -> Skill {
  Skill::new(
    description.to_owned(),
    "# review-diff\n".to_owned(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a skill")
}

fn tool(program: &str) -> Tool {
  Tool::command(
    "Checks the build.",
    CommandAction::new(CommandProgram::new(program).expect("a program"), Vec::new()),
  )
}

/// The team's resource `name` of `kind`, at v1 on the server with `body`, and Alice's desktop, an admin, holding it as
/// a read of the server would.
fn held(server: &TeamServer, (kind, name): (SharedResourceKind, &str), body: &impl Serialize) -> (Desktop, ResourceId) {
  let id = ResourceId::generate();
  server.publish((id, kind, name), value(body), Vec::new());
  let desktop = Desktop::signed_in(server, ALICE, Fixture::new(), &[]);
  ProjectDb::open(desktop.root())
    .expect("the project")
    .record_sharing_state(Some(MemberRole::Owner), None, None)
    .expect("the role");
  let resource = TeamResource {
    id,
    kind: kind.core(),
    name: Name::new(name).expect("name"),
    archived: false,
    revision: TaskRevision::try_from(1).expect("a revision"),
    updated: orchester_workspace::team_config::Changed {
      by: "Alice Moreau".to_owned(),
      at: orchester_core::time::Timestamp::from_unix_millis(0),
    },
    latest: Some(TeamVersion {
      number: number(1),
      body: value(body),
      published_by: "Alice Moreau".to_owned(),
      published_at: Timestamp::from_unix_millis(1),
    }),
  };
  desktop
    .workspace
    .keep_team_config_for_test(desktop.root(), TeamConfig::new([resource]));
  (desktop, id)
}

/// What the panel of `id` starts from now.
fn shown<B: DeserializeOwned>(desktop: &Desktop, id: ResourceId) -> Option<Latest<B>> {
  let read = desktop.workspace.team_read(desktop.root());
  team_body_edit::latest(read.config()?, id)
}

async fn saves_the_next_version<B>(kind: (SharedResourceKind, &str), (before, after): (B, B))
where
  B: Serialize + DeserializeOwned + Clone + PartialEq + std::fmt::Debug + Send + 'static,
{
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let (desktop, id) = held(&server, kind, &before);
  assert_eq!(
    shown::<B>(&desktop, id),
    Some(Latest {
      number: number(1),
      body: before
    })
  );

  let saved = team_body_edit::save(&desktop.servers, desktop.root(), (id, number(1)), &after)
    .await
    .expect("published");

  assert_eq!(saved, number(2));
  let versions = server.versions_of(id);
  assert_eq!(versions.len(), 2);
  assert_eq!(
    (versions[1].based_on, &versions[1].body),
    (Some(number(1)), &value(&after))
  );
  assert_eq!(
    shown::<B>(&desktop, id),
    Some(Latest {
      number: number(2),
      body: after
    }),
    "the panel shows what was published"
  );
  let db = ProjectDb::open(desktop.root()).expect("the project");
  for kind in [ResourceKind::Instruction, ResourceKind::Skill, ResourceKind::Tool] {
    assert!(
      orchester_core::store::ResourceStore::resources(&db, kind)
        .expect("read")
        .is_empty(),
      "nothing is kept on this computer"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_team_body_edit_an_instructions_save_publishes_its_next_version() {
  saves_the_next_version(
    (SharedResourceKind::Instruction, "Project rules"),
    (rule("Use tabs."), rule("Use spaces.")),
  )
  .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_team_body_edit_a_skills_save_publishes_its_next_version() {
  saves_the_next_version(
    (SharedResourceKind::Skill, "review-diff"),
    (skill("Reviews a diff."), skill("Reviews a diff against the brief.")),
  )
  .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_team_body_edit_a_tools_save_publishes_its_next_version() {
  saves_the_next_version(
    (SharedResourceKind::Tool, "cargo-check"),
    (tool("cargo"), tool("cargo-nextest")),
  )
  .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_team_body_edit_a_save_on_an_older_version_reloads_the_latest() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let (desktop, id) = held(&server, (SharedResourceKind::Instruction, "Style"), &rule("Use tabs."));
  server.publish(
    (id, SharedResourceKind::Instruction, "Style"),
    value(&rule("Use spaces.")),
    Vec::new(),
  );

  let refused = team_body_edit::save(&desktop.servers, desktop.root(), (id, number(1)), &rule("Use both."))
    .await
    .expect_err("someone published first");

  assert_eq!(refused, EditRefused::Changed { latest: number(2) });
  assert_eq!(server.versions_of(id).len(), 2, "nothing was published");
  assert_eq!(
    shown(&desktop, id),
    Some(Latest {
      number: number(2),
      body: rule("Use spaces.")
    }),
    "the panel reloads from the server's latest"
  );
}
