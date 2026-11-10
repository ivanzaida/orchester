//! Share uploads the project's configuration with its history, and Make local drops it (`docs/architecture/server.md`,
//! "Migration", S5), against the fake team server: every pipeline, profile, instruction and skill goes up with its id
//! and every published version with its number and time, a skill's parts first by their hash, while a resource that
//! was never published stays here; the sharer's versions become the server's with their ids and bodies, and a second
//! workspace reads them from its stream. Make local keeps no copy of it: the cached resources and versions go, with the
//! references and the bindings, while the project's own resources stay and the runs keep their snapshots.

use std::time::Duration;

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ResourceId, RunId},
  name::Name,
  resource::{
    Draft, Instruction, InstructionScope, Pipeline, ResourceKind, Skill, SkillFile, SkillFilePath, Slug,
    TeamAgentProfile, TeamReviewer, Version,
  },
  run::Run,
  sharing::MemberRole,
  store::{BlobStore, ResourceStore, RunStore},
};
use orchester_store::{ProjectDb, ReferenceTarget as KeptTarget, ResourceSharing, StoreError};
use orchester_workspace::sharing::{
  ShareRefusalReason, SharingError,
  sync::{keep_uploaded_configuration, upload_configuration},
};

use super::{
  ALICE, BOB, Member, TeamServer, Workspace, link,
  publish::{Publisher, json, number, one_step, rule},
  seen_within,
  support::{Fixture, at},
};

/// How long the second workspace may take to cache the uploaded resources from its stream.
const CACHED: Duration = Duration::from_secs(5);

/// Alice's local project: the fixture's pipeline Delivery and profile Builder at v1, the instruction Style at v1 and
/// v2, the skill Release notes at v1 with one part, an instruction Notes never published, and a run that pinned
/// Delivery's v1.
pub(super) struct Sharer {
  pub(super) fixture: Fixture,
  style: ResourceId,
  skill: ResourceId,
  unpublished: ResourceId,
  part: (ContentHash, Vec<u8>),
  run_id: RunId,
}

impl Sharer {
  pub(super) fn new() -> Self {
    let mut fixture = Fixture::new();
    fixture.connect_builder(AgentProvider::CodexCli);
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

  pub(super) fn db(&self) -> ProjectDb {
    ProjectDb::open(self.fixture.repo.path()).expect("the project database")
  }

  /// The fixture's pipeline Delivery and its one profile, Builder.
  pub(super) fn delivery(&self) -> (ResourceId, ResourceId) {
    let profiles = self.db().resources(ResourceKind::AgentProfile).expect("profiles");
    (self.run().snapshot().pipeline().resource, profiles[0].id())
  }

  fn run(&self) -> Run {
    self.db().run(self.run_id).expect("read").expect("the run")
  }

  /// Shares the project with `server` as its owner: the configuration goes up, the project is linked, and the uploaded
  /// resources are kept as the server's.
  pub(super) async fn share(&self, server: &TeamServer) {
    let publisher = Publisher::online(server, ALICE);
    let root = self.fixture.repo.path().to_owned();
    let uploaded = upload_configuration(&publisher, &root).await.expect("uploaded");
    link(&mut self.db(), server, MemberRole::Owner);
    keep_uploaded_configuration(&publisher, &root, uploaded)
      .await
      .expect("kept as the server's");
  }
}

pub(super) fn slug(value: &str) -> Slug {
  Slug::new(value).expect("slug")
}

pub(super) fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn style_versions(db: &ProjectDb, style: ResourceId) -> Vec<Version<Instruction>> {
  db.versions::<Instruction>(style).expect("Style's versions")
}

/// Each version of `resource` on the server: its number, when it was published, its publisher.
pub(super) fn on_server(server: &TeamServer, resource: ResourceId) -> Vec<(u32, i64, String)> {
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

/// A team resource names no connection of a computer: Share uploads a profile in the team's form, with the CLI kind of
/// its connection, and the references that body names.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_uploads_a_profile_in_the_teams_form_with_the_cli_kind_of_its_connection() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Sharer::new();
  let (_, builder) = alice.delivery();

  alice.share(&server).await;

  let sent = server.versions_of(builder).pop().expect("v1 on the server");
  let team: TeamAgentProfile = serde_json::from_value(sent.body.clone()).expect("a team profile");
  assert_eq!(team.cli(), AgentProvider::CodexCli);
  assert!(sent.body.get("connection").is_none(), "{}", sent.body);
  assert_eq!(sent.references, ReferenceTarget::of_profile(&team));
  assert_eq!(
    sent.references,
    [ReferenceTarget::CliKind {
      cli: AgentProvider::CodexCli
    }]
  );
}

/// A team reviewer names a CLI kind and no connection: Share uploads it as it is, with the CLI kind among its
/// references, and a decision reviewer with none.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_uploads_a_team_reviewer_with_its_cli_kind_as_the_reference() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let language: TeamReviewer = serde_json::from_value(
    serde_json::json!({"kind": "language", "cli": "claude_code_cli", "model": "m-1", "effort": "low"}),
  )
  .expect("a language reviewer");
  let decision: TeamReviewer = serde_json::from_value(serde_json::json!({
    "kind": "decision", "model": "d-1", "lowest_threshold": 0.8, "destinations": {"kind": "members_computers"}
  }))
  .expect("a decision reviewer");
  let db = &mut alice.fixture.db;
  let mut ids = Vec::new();
  for (label, body) in [("language-reviewer", &language), ("decision-reviewer", &decision)] {
    let id = db
      .create_resource(slug(label), name(label), body, at(40))
      .expect("a reviewer")
      .id();
    db.publish_draft::<TeamReviewer>(id, at(41)).expect("v1");
    ids.push(id);
  }

  alice.share(&server).await;

  let sent = server.versions_of(ids[0]).pop().expect("the language reviewer's v1");
  assert_eq!(sent.body, serde_json::to_value(&language).expect("json"));
  assert!(sent.body.get("connection").is_none(), "{}", sent.body);
  assert_eq!(
    sent.references,
    [ReferenceTarget::CliKind {
      cli: AgentProvider::ClaudeCodeCli
    }]
  );
  let sent = server.versions_of(ids[1]).pop().expect("the decision reviewer's v1");
  assert_eq!(sent.body, serde_json::to_value(&decision).expect("json"));
  assert!(sent.references.is_empty(), "{:?}", sent.references);
}

/// Share uploads the pipelines with their ids, so a pipeline running a profile it does not upload would give the team
/// an id the server does not have: it is refused and nothing is sent.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_uploads_nothing_when_a_pipeline_runs_a_profile_it_does_not_upload() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Sharer::new();
  let (delivery, _) = alice.delivery();
  let mut db = alice.db();
  let hotfix = db
    .create_resource(
      slug("hotfix"),
      name("Hotfix"),
      &one_step(ResourceId::generate()),
      at(30),
    )
    .expect("Hotfix")
    .id();
  db.publish_draft::<Pipeline>(hotfix, at(31)).expect("v1");
  let publisher = Publisher::online(&server, ALICE);

  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;

  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  assert_eq!(refusals.len(), 1, "{refusals:?}");
  assert_eq!(refusals[0].resource, hotfix);
  assert_eq!(refusals[0].name, "Hotfix");
  assert!(matches!(
    refusals[0].reason,
    ShareRefusalReason::PipelineNamesNonTeam { .. }
  ));
  assert_eq!(on_server(&server, delivery), [], "nothing was uploaded");
}

/// Nothing of the resource is left in the project: not the row, not a cached version, not a listing of its kind.
fn assert_resource_gone(db: &ProjectDb, resource: ResourceId) {
  assert!(
    matches!(db.resource_sharing(resource), Err(StoreError::ResourceNotFound(_))),
    "the resource is gone"
  );
  assert_eq!(
    db.latest_version_number(resource).expect("read"),
    None,
    "no cached version"
  );
  for kind in [
    ResourceKind::Pipeline,
    ResourceKind::AgentProfile,
    ResourceKind::Skill,
    ResourceKind::Instruction,
  ] {
    let listed = db.resources(kind).expect("read");
    assert!(
      listed.iter().all(|found| found.id() != resource),
      "not listed as {kind:?}"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_make_local_keeps_no_copy_of_the_teams_configuration_and_past_runs_are_unchanged() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let alice = Sharer::new();
  alice.share(&server).await;
  let tone = ResourceId::generate();
  let named = ReferenceTarget::TeamResource {
    kind: SharedResourceKind::Skill,
    id: alice.skill,
  };
  server.publish(
    (tone, SharedResourceKind::Instruction, "Tone"),
    json("tone"),
    vec![named],
  );
  let bob = bob_caches(&server, &[alice.style, tone]).await;
  assert_eq!(
    bob
      .db()
      .cached_version::<Instruction>(tone, orchester_core::resource::VersionNumber::FIRST)
      .expect("read")
      .expect("cached")
      .references,
    [KeptTarget::TeamResource {
      kind: ResourceKind::Skill,
      id: alice.skill,
    }],
    "Tone's reference is cached"
  );
  let (delivery, builder) = alice.delivery();
  let run = alice.run();
  alice.db().make_local().expect("made local");
  bob.db().make_local().expect("made local");

  assert_eq!(alice.run(), run, "the run is unchanged");
  let gone = [
    (alice.db(), alice.style),
    (alice.db(), alice.skill),
    (alice.db(), delivery),
    (alice.db(), builder),
    (bob.db(), alice.style),
    (bob.db(), tone),
  ];
  for (db, resource) in gone {
    assert_resource_gone(&db, resource);
  }
  assert!(
    matches!(
      alice.db().versions::<Instruction>(alice.style),
      Err(StoreError::ResourceNotFound(_))
    ),
    "no cached versions of a gone resource"
  );
  let notes = alice.db().resources(ResourceKind::Instruction).expect("read");
  assert_eq!(
    notes.iter().map(|found| found.id()).collect::<Vec<_>>(),
    [alice.unpublished],
    "Alice's own instruction stays"
  );
  assert!(
    alice
      .db()
      .draft::<Instruction>(alice.unpublished)
      .expect("read")
      .is_some(),
    "with its draft"
  );
  assert_eq!(
    on_server(&server, alice.style).len(),
    2,
    "the server keeps the team's resources"
  );
}
