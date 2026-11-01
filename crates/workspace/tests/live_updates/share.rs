//! Share uploads the project's configuration with its history, and Make local keeps it (`docs/architecture/server.md`,
//! "Migration", S5), against the fake team server: every pipeline, profile, instruction and skill goes up with its id
//! and every published version with its number and time, a skill's parts first by their hash, while a resource that
//! was never published stays here; the sharer's versions become the server's with their ids and bodies, and a second
//! workspace reads them from its stream. Make local keeps every version as a local one, drops the references and the
//! bindings, and the runs that pinned them read them as before.

use std::time::Duration;

use orchester_api::{Reference, ReferenceTarget, SharedResourceKind};
use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, RunId},
  name::Name,
  resource::{
    Draft, Instruction, InstructionScope, Pipeline, ResourceKind, Skill, SkillFile, SkillFilePath, Slug, Version,
  },
  run::Run,
  sharing::MemberRole,
  store::{BlobStore, ResourceStore, RunStore},
};
use orchester_store::{ProjectDb, ReferenceTarget as KeptTarget, ResourceSharing};
use orchester_workspace::sharing::sync::{keep_uploaded_configuration, upload_configuration};

use super::{
  ALICE, BOB, Member, TeamServer, Workspace, link,
  publish::{Publisher, json, number, rule},
  seen_within,
  support::{Fixture, at},
};

/// How long the second workspace may take to cache the uploaded resources from its stream.
const CACHED: Duration = Duration::from_secs(5);

/// Alice's local project: the fixture's pipeline Delivery and profile Builder at v1, the instruction Style at v1 and
/// v2, the skill Release notes at v1 with one part, an instruction Notes never published, and a run that pinned
/// Delivery's v1.
struct Sharer {
  fixture: Fixture,
  style: ResourceId,
  skill: ResourceId,
  unpublished: ResourceId,
  part: (ContentHash, Vec<u8>),
  run_id: RunId,
}

impl Sharer {
  fn new() -> Self {
    let mut fixture = Fixture::new();
    let db = &mut fixture.db;
    let style = db
      .create_resource(slug("style"), name("Style"), &rule("v1"), at(10))
      .expect("Style")
      .id();
    db.publish_draft::<Instruction>(style, at(11)).expect("v1");
    db.save_draft(style, &Draft::new(rule("v2"), Some(number(1)), at(12)))
      .expect("a draft of v2");
    db.publish_draft::<Instruction>(style, at(13)).expect("v2");
    let content = b"One line per change.".to_vec();
    let hash = db.put_blob("text/markdown", &content).expect("a part");
    let file = SkillFile {
      path: SkillFilePath::new("references/format.md").expect("path"),
      content: hash.clone(),
      size: content.len() as u64,
    };
    let body = Skill::new(
      "Writing release notes.".to_owned(),
      "# Release notes".to_owned(),
      vec![file],
      InstructionScope::Project,
    )
    .expect("a skill");
    let skill = db
      .create_resource(slug("release-notes"), name("Release notes"), &body, at(14))
      .expect("Release notes")
      .id();
    db.publish_draft::<Skill>(skill, at(15)).expect("v1");
    let unpublished = db
      .create_resource(slug("notes"), name("Notes"), &rule("draft"), at(16))
      .expect("Notes")
      .id();
    let task = fixture.task("Sign in", at(20));
    let run = fixture.pending_run(task, at(21)).id();
    Self {
      fixture,
      style,
      skill,
      unpublished,
      part: (hash, content),
      run_id: run,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.fixture.repo.path()).expect("the project database")
  }

  /// The fixture's pipeline Delivery and its one profile, Builder.
  fn delivery(&self) -> (ResourceId, ResourceId) {
    let profiles = self.db().resources(ResourceKind::AgentProfile).expect("profiles");
    (self.run().snapshot().pipeline().resource, profiles[0].id())
  }

  fn run(&self) -> Run {
    self.db().run(self.run_id).expect("read").expect("the run")
  }

  /// Shares the project with `server` as its owner: the configuration goes up, the project is linked, and the uploaded
  /// resources are kept as the server's.
  async fn share(&self, server: &TeamServer) {
    let publisher = Publisher::online(server, ALICE);
    let root = self.fixture.repo.path().to_owned();
    let uploaded = upload_configuration(&publisher, &root).await.expect("uploaded");
    link(&mut self.db(), server, MemberRole::Owner);
    keep_uploaded_configuration(&publisher, &root, uploaded)
      .await
      .expect("kept as the server's");
  }
}

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("slug")
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn style_versions(db: &ProjectDb, style: ResourceId) -> Vec<Version<Instruction>> {
  db.versions::<Instruction>(style).expect("Style's versions")
}

/// Each version of `resource` on the server: its number, when it was published, its publisher.
fn on_server(server: &TeamServer, resource: ResourceId) -> Vec<(u32, i64, String)> {
  server
    .versions_of(resource)
    .into_iter()
    .map(|version| {
      (
        version.number.get(),
        version.published.at,
        version.published.by.display_name,
      )
    })
    .collect()
}

/// A second workspace, Bob's, following the project until each of `resources` is cached there.
async fn bob_caches(server: &TeamServer, resources: &[ResourceId]) -> Workspace {
  let bob = Workspace::linked(server);
  let stream = bob.follow(Member::of(server, BOB));
  seen_within(CACHED, "the resources on the second workspace", || {
    let db = bob.db();
    resources
      .iter()
      .all(|resource| db.latest_version_number(*resource).ok().flatten().is_some())
  })
  .await;
  stream.abort();
  bob
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_uploads_every_resource_with_its_numbers_and_skill_files_and_a_second_workspace_reads_them()
{
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let alice = Sharer::new();
  let (delivery, builder) = alice.delivery();
  let before = style_versions(&alice.db(), alice.style);
  let run = alice.run();

  alice.share(&server).await;

  let alice_moreau = || "Alice Moreau".to_owned();
  assert_eq!(
    on_server(&server, alice.style),
    [(1, 11, alice_moreau()), (2, 13, alice_moreau())],
    "every version, numbers and times kept"
  );
  assert_eq!(on_server(&server, delivery), [(1, 5, alice_moreau())]);
  assert_eq!(on_server(&server, builder), [(1, 3, alice_moreau())]);
  assert_eq!(on_server(&server, alice.skill), [(1, 15, alice_moreau())]);
  let (hash, content) = alice.part.clone();
  assert_eq!(
    server.file(&hash),
    Some(content),
    "the skill's part, uploaded by its hash"
  );
  assert_eq!(
    on_server(&server, alice.unpublished),
    [],
    "a resource never published stays here"
  );

  let db = alice.db();
  assert_eq!(
    style_versions(&db, alice.style),
    before,
    "the same versions, ids and bodies"
  );
  for resource in [alice.style, alice.skill, delivery, builder] {
    let sharing = db.resource_sharing(resource).expect("read");
    assert!(matches!(sharing, Some(ResourceSharing::Shared(_))), "{sharing:?}");
  }
  assert_eq!(db.resource_sharing(alice.unpublished).expect("read"), None);
  let cached = db.cached_version::<Instruction>(alice.style, number(2)).expect("read");
  assert_eq!(cached.expect("the server's v2").publisher.name, "Alice Moreau");
  assert_eq!(alice.run(), run, "the run is unchanged");

  let bob = bob_caches(&server, &[alice.style, builder, delivery]).await;
  let bobs = style_versions(&bob.db(), alice.style);
  assert_eq!(
    bobs
      .iter()
      .map(|version| (version.number().get(), version.body().clone()))
      .collect::<Vec<_>>(),
    [(1, rule("v1")), (2, rule("v2"))]
  );
  let pipeline = bob.db().latest_version::<Pipeline>(delivery).expect("read");
  assert_eq!(
    pipeline.expect("Delivery on the second workspace").body(),
    run.snapshot().body()
  );
  let member = Member::of(&server, BOB);
  let skill = member
    .client
    .versions(&member.token, member.project, alice.skill, None)
    .await;
  let skill = skill.expect("the skill's versions");
  let kept = alice.db().version::<Skill>(alice.skill, number(1)).expect("read");
  let body = serde_json::to_value(kept.expect("v1").body()).expect("a skill serializes");
  assert_eq!(
    skill
      .versions
      .iter()
      .map(|version| (version.number.get(), version.body.clone()))
      .collect::<Vec<_>>(),
    [(1, body)],
    "the second workspace reads the skill with its number"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_make_local_keeps_every_version_and_past_runs_are_unchanged() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let alice = Sharer::new();
  alice.share(&server).await;
  let agent = ConnectionId::generate();
  let tone = ResourceId::generate();
  let named = Reference {
    target: ReferenceTarget::Connection { id: agent },
    name: "Agent".to_owned(),
    role: Some("claude_cli".to_owned()),
  };
  server.publish(
    (tone, SharedResourceKind::Instruction, "Tone"),
    json("tone"),
    vec![named],
  );
  let bob = bob_caches(&server, &[alice.style, tone]).await;
  assert_eq!(
    bob.db().reference_bindings().expect("read").len(),
    1,
    "Tone's reference"
  );
  let (alices, run) = (style_versions(&alice.db(), alice.style), alice.run());
  let bobs = (style_versions(&bob.db(), alice.style), style_versions(&bob.db(), tone));

  alice.db().make_local().expect("made local");
  bob.db().make_local().expect("made local");

  assert_eq!(style_versions(&alice.db(), alice.style), alices, "every version kept");
  assert_eq!(alice.run(), run, "the run is unchanged");
  let pinned = run.snapshot().pipeline();
  let read = alice
    .db()
    .version::<Pipeline>(pinned.resource, pinned.number)
    .expect("read");
  let read = read.expect("the pinned version");
  assert_eq!((read.id(), read.body()), (pinned.version, run.snapshot().body()));
  assert_eq!(
    (style_versions(&bob.db(), alice.style), style_versions(&bob.db(), tone)),
    bobs,
    "every cached version kept"
  );
  for (db, resource) in [(alice.db(), alice.style), (bob.db(), alice.style), (bob.db(), tone)] {
    assert_eq!(db.resource_sharing(resource).expect("read"), None, "a local resource");
  }
  let db = bob.db();
  assert_eq!(db.reference_bindings().expect("read"), []);
  assert_eq!(db.versions_naming(KeptTarget::Connection(agent)).expect("read"), []);

  let mut db = alice.db();
  db.save_draft(alice.style, &Draft::new(rule("v3"), Some(number(2)), at(30)))
    .expect("a draft of v3");
  let v3 = db
    .publish_draft::<Instruction>(alice.style, at(31))
    .expect("a local v3");
  assert_eq!(v3.number().get(), 3, "numbered after the kept versions");
  assert_eq!(on_server(&server, alice.style).len(), 2, "nothing goes to the server");
}
