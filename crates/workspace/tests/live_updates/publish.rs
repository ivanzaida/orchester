//! An admin publishes the team's configuration from this computer (`docs/architecture/server.md`, "Shared
//! configuration", rows 36 and 72) through `RemoteResourceStore`, against the fake team server: online the version gets
//! the server's number at once; offline it is used here at once and gets its number on reconnect; two publishes from
//! one base give the second `version_conflict`, and the admin publishes theirs as the next version; a new skill is
//! created with its parts uploaded by hash. A member has no drafts of the team's resources: this computer refuses
//! them, and so does the server when this computer took the member for an admin.

use std::{
  collections::BTreeSet,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_api::{
  FileStored, ResourceCreate, ResourceCreated, ResourcesImport, SharedResource, SharedResourceKind, SharedVersion,
  VersionPublish, Versions,
};
use orchester_core::{
  blob::ContentHash,
  id::ResourceId,
  name::Name,
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Skill, SkillFile, SkillFilePath,
    Slug, Version, VersionNumber,
  },
  sharing::MemberRole,
  store::{BlobStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState, ResourceSharing, WaitingPublish};
use orchester_workspace::sharing::{
  SharingError,
  sync::{ConfigServer, RemoteResourceStore},
};
use serde_json::Value;

use super::{ALICE, BOB, Member, TeamServer, Workspace, seen_within};

/// How long a computer may take to cache the resource from its stream: these tests are not about how soon.
const CACHED: Duration = Duration::from_secs(5);

/// A person's server as their store publishes to it; it cannot be reached while `offline` is set.
pub(super) struct Publisher {
  member: Member,
  offline: Arc<AtomicBool>,
}

impl Publisher {
  /// `token`'s member's server, which can be reached.
  pub(super) fn online(server: &TeamServer, token: &str) -> Self {
    Self {
      member: Member::of(server, token),
      offline: Arc::new(AtomicBool::new(false)),
    }
  }
}

impl ConfigServer for Publisher {
  fn writable(&self) -> Result<(), SharingError> {
    if self.offline.load(Ordering::SeqCst) {
      return Err(SharingError::Offline);
    }
    Ok(())
  }

  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .versions(&member.token, member.project, resource, after)
        .await?,
    )
  }

  async fn create_resource(&self, create: ResourceCreate) -> Result<ResourceCreated, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .create_resource(&member.token, member.project, &create)
        .await?,
    )
  }

  async fn publish_version(
    &self,
    resource: ResourceId,
    publish: VersionPublish,
  ) -> Result<SharedVersion, SharingError> {
    let member = &self.member;
    let published = member
      .client
      .publish_version(&member.token, member.project, resource, &publish);
    Ok(published.await?)
  }

  async fn upload_file(&self, hash: ContentHash, content: Vec<u8>) -> Result<FileStored, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .upload_file(&member.token, member.project, &hash, content)
        .await?,
    )
  }

  async fn import_resources(&self, import: ResourcesImport) -> Result<Vec<SharedResource>, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .import_resources(&member.token, member.project, &import)
        .await?,
    )
  }
}

/// A person's computer: their project linked to the server's, and whether the server can be reached from it.
pub(super) struct Computer {
  pub(super) workspace: Workspace,
  token: &'static str,
  offline: Arc<AtomicBool>,
}

impl Computer {
  pub(super) fn new(server: &TeamServer, token: &'static str, role: MemberRole) -> Self {
    Self {
      workspace: Workspace::linked_as(server, role),
      token,
      offline: Arc::new(AtomicBool::new(false)),
    }
  }

  fn db(&self) -> ProjectDb {
    self.workspace.db()
  }

  pub(super) fn go_offline(&self, offline: bool) {
    self.offline.store(offline, Ordering::SeqCst);
  }

  /// Follows the project's stream until `resource`'s v1 is cached here, then stops following.
  async fn cache_first(&self, server: &TeamServer, resource: ResourceId) {
    let stream = self.workspace.follow(Member::of(server, self.token));
    seen_within(CACHED, "the resource's v1", || latest(&self.db(), resource).is_some()).await;
    stream.abort();
  }

  /// Runs `work` with the person's store on a worker of the runtime, as the app does.
  pub(super) async fn store<T: Send + 'static>(
    &self,
    server: &TeamServer,
    work: impl FnOnce(&mut RemoteResourceStore<Publisher>) -> T + Send + 'static,
  ) -> T {
    let publisher = Publisher {
      member: Member::of(server, self.token),
      offline: self.offline.clone(),
    };
    let root = self.workspace.root.clone();
    tokio::task::spawn_blocking(move || {
      let db = ProjectDb::open(&root).expect("the project database");
      let mut store = RemoteResourceStore::new(db, publisher).expect("on the runtime");
      work(&mut store)
    })
    .await
    .expect("the store's work")
  }

  /// Saves a draft of `text` on version `on` of `resource` and publishes it.
  async fn publish(
    &self,
    server: &TeamServer,
    resource: ResourceId,
    on: u32,
    text: &str,
  ) -> Result<Version<Instruction>, SharingError> {
    let body = rule(text);
    self
      .store(server, move |store| {
        let draft = Draft::new(body, Some(number(on)), Timestamp::now());
        store.save_draft(resource, &draft)?;
        store.publish_draft::<Instruction>(resource, Timestamp::now())
      })
      .await
  }

  fn waiting(&self) -> Vec<WaitingPublish> {
    self.db().waiting_publishes().expect("the waiting versions")
  }

  /// Who published version `on` of `resource`, as the server said; `None` while the server has not numbered it.
  fn publisher(&self, resource: ResourceId, on: u32) -> Option<String> {
    let cached = self.db().cached_version::<Instruction>(resource, number(on));
    cached.expect("read").map(|version| version.publisher.name)
  }
}

pub(super) fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("a prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("a valid instruction")
}

pub(super) fn json(text: &str) -> Value {
  serde_json::to_value(rule(text)).expect("an instruction serializes")
}

pub(super) fn number(value: u32) -> VersionNumber {
  VersionNumber::new(value.try_into().expect("above zero"))
}

fn latest(db: &ProjectDb, resource: ResourceId) -> Option<Version<Instruction>> {
  // The resource is not in the cache until its first version is read.
  db.latest_version::<Instruction>(resource).ok().flatten()
}

/// A server whose project has the instruction Style at v1, published by Alice, its owner; Bob is a member.
fn style() -> (TeamServer, ResourceId) {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let style = ResourceId::generate();
  server.publish(
    (style, SharedResourceKind::Instruction, "Style"),
    json("v1"),
    Vec::new(),
  );
  (server, style)
}

/// Each version of `resource` on the server: its number, the version it was based on, its publisher.
fn on_server(server: &TeamServer, resource: ResourceId) -> Vec<(u32, Option<u32>, String)> {
  let versions = server.versions_of(resource);
  versions
    .into_iter()
    .map(|version| {
      (
        version.number.get(),
        version.based_on.map(VersionNumber::get),
        version.published.by.display_name,
      )
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_owner_publishes_online_and_offline_and_the_offline_version_gets_its_number_on_reconnect() {
  let (server, style) = style();
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  alice.cache_first(&server, style).await;

  let v2 = alice.publish(&server, style, 1, "v2").await.expect("published");
  assert_eq!(v2.number().get(), 2);
  assert_eq!(alice.publisher(style, 2).as_deref(), Some("Alice Moreau"));
  assert_eq!(alice.waiting(), []);

  alice.go_offline(true);
  let v3 = alice.publish(&server, style, 2, "v3").await.expect("published here");
  assert_eq!(v3.number().get(), 3, "its number here until the server's");
  let waiting = alice.waiting();
  assert_eq!(waiting.len(), 1, "{waiting:?}");
  assert_eq!(
    (waiting[0].number.get(), waiting[0].state.clone()),
    (3, PublishState::Waiting)
  );
  let used = latest(&alice.db(), style).expect("a latest version");
  assert_eq!(used.body(), &rule("v3"), "the admin's new runs pin it at once");
  assert_eq!(alice.publisher(style, 3), None, "published here, not sent");
  assert_eq!(on_server(&server, style).len(), 2, "nothing was sent");

  alice.go_offline(false);
  let sent = alice.store(&server, |store| store.send_waiting()).await;
  sent.expect("sent on reconnect");
  assert_eq!(alice.waiting(), []);
  assert_eq!(alice.publisher(style, 3).as_deref(), Some("Alice Moreau"));
  assert_eq!(
    on_server(&server, style),
    [
      (1, None, "Alice Moreau".to_owned()),
      (2, Some(1), "Alice Moreau".to_owned()),
      (3, Some(2), "Alice Moreau".to_owned()),
    ]
  );
  let kept = latest(&alice.db(), style).expect("a latest version");
  assert_eq!(
    (kept.id(), kept.number().get()),
    (v3.id(), 3),
    "the same version, numbered"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_two_publishes_from_one_base_give_the_second_a_version_conflict_the_admin_decides() {
  let (server, style) = style();
  server.make_admin(BOB);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let bob = Computer::new(&server, BOB, MemberRole::Member);
  bob.db().set_server_admin(true).expect("the server's admin");
  alice.cache_first(&server, style).await;
  bob.cache_first(&server, style).await;

  alice.publish(&server, style, 1, "Alice's").await.expect("published");
  let refused = bob.publish(&server, style, 1, "Bob's").await;
  assert!(
    matches!(refused, Err(SharingError::VersionConflict { latest }) if latest.get() == 2),
    "{refused:?}"
  );
  let waiting = bob.waiting();
  assert_eq!(waiting.len(), 1, "{waiting:?}");
  assert_eq!(
    (waiting[0].number.get(), waiting[0].state.clone()),
    (3, PublishState::Conflict { latest: number(2) }),
    "Alice's v2 took its number here"
  );
  assert_eq!(bob.publisher(style, 2).as_deref(), Some("Alice Moreau"));
  let kept = bob.db().version::<Instruction>(style, number(2)).expect("read");
  assert_eq!(kept.expect("cached").body(), &rule("Alice's"));

  let version = waiting[0].version;
  bob
    .store(&server, move |store| store.publish_again(version))
    .await
    .expect("published as the next version");
  assert_eq!(bob.waiting(), []);
  assert_eq!(bob.publisher(style, 3).as_deref(), Some("Bob Lind"));
  assert_eq!(on_server(&server, style)[2], (3, Some(2), "Bob Lind".to_owned()));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_the_libraries_read_each_resources_publisher_and_the_versions_that_wait_here() {
  let (server, style) = style();
  server.make_admin(BOB);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let bob = Computer::new(&server, BOB, MemberRole::Member);
  bob.db().set_server_admin(true).expect("the server's admin");
  alice.cache_first(&server, style).await;
  bob.cache_first(&server, style).await;

  alice.publish(&server, style, 1, "Alice's").await.expect("published");
  bob
    .publish(&server, style, 1, "Bob's")
    .await
    .expect_err("met Alice's v2");
  let standings = bob.db().shared_standings().expect("the standings");
  let standing = standings
    .iter()
    .find(|s| s.resource == style)
    .expect("Style is the team's");
  let confirmed = standing.confirmed.as_ref().expect("v2 is numbered");
  assert_eq!(
    (confirmed.number.get(), confirmed.publisher.as_deref()),
    (2, Some("Alice Moreau"))
  );
  assert_eq!(standing.waiting.len(), 1, "{standing:?}");
  assert_eq!(
    (standing.waiting[0].based_on, standing.waiting[0].state.clone()),
    (Some(number(1)), PublishState::Conflict { latest: number(2) })
  );

  alice.go_offline(true);
  alice
    .publish(&server, style, 2, "Alice's v3")
    .await
    .expect("published here");
  let standings = alice.db().shared_standings().expect("the standings");
  let standing = standings
    .iter()
    .find(|s| s.resource == style)
    .expect("Style is the team's");
  assert_eq!(
    standing.confirmed.as_ref().map(|c| c.number.get()),
    Some(2),
    "v3 is not numbered yet"
  );
  assert_eq!(
    standing.waiting.iter().map(|w| w.state.clone()).collect::<Vec<_>>(),
    [PublishState::Waiting]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_members_draft_and_publish_answer_not_admin_here_and_from_the_server() {
  let (server, style) = style();
  let bob = Computer::new(&server, BOB, MemberRole::Member);
  bob.cache_first(&server, style).await;

  let refused = bob.publish(&server, style, 1, "Bob's").await;
  assert!(matches!(refused, Err(SharingError::NotAdmin)), "{refused:?}");
  let (draft, publish, create) = bob
    .store(&server, move |store| {
      let draft = store.draft::<Instruction>(style);
      let publish = store.publish_draft::<Instruction>(style, Timestamp::now());
      let slug = Slug::new("tone").expect("slug");
      let create = store.create_resource(slug, Name::new("Tone").expect("name"), &rule("Tone"), Timestamp::now());
      (draft, publish, create)
    })
    .await;
  assert!(matches!(draft, Ok(None)), "a member has no drafts: {draft:?}");
  assert!(matches!(publish, Err(SharingError::NotAdmin)), "{publish:?}");
  assert!(matches!(create, Err(SharingError::NotAdmin)), "{create:?}");

  // This computer took Bob for the server's admin; the server does not.
  bob.db().set_server_admin(true).expect("recorded");
  let refused = bob.publish(&server, style, 1, "Bob's").await;
  assert!(matches!(refused, Err(SharingError::NotAdmin)), "{refused:?}");
  let waiting = bob.waiting();
  assert_eq!(
    waiting.iter().map(|waiting| waiting.state.clone()).collect::<Vec<_>>(),
    [PublishState::Refused {
      code: "not_admin".to_owned()
    }]
  );
  assert_eq!(on_server(&server, style).len(), 1, "the server took nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_new_skill_is_created_on_the_server_with_its_parts_uploaded_by_hash() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let content = b"One line per change.".to_vec();
  let hash = alice.db().put_blob("text/markdown", &content).expect("a part");
  let file = SkillFile {
    path: SkillFilePath::new("references/format.md").expect("path"),
    content: hash.clone(),
    size: content.len() as u64,
  };
  let skill = Skill::new(
    "Writing release notes.".to_owned(),
    "# Release notes".to_owned(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("a skill");

  let (id, published) = alice
    .store(&server, move |store| {
      let slug = Slug::new("release-notes").expect("slug");
      let name = Name::new("Release notes").expect("name");
      let created = store.create_resource(slug, name, &skill, Timestamp::now())?;
      let published = store.publish_draft::<Skill>(created.id(), Timestamp::now())?;
      Ok::<_, SharingError>((created.id(), published))
    })
    .await
    .expect("created on the server");
  assert_eq!(published.number().get(), 1);
  assert_eq!(server.file(&hash), Some(content), "the part, uploaded by its hash");
  assert_eq!(server.versions_of(id).len(), 1);
  assert!(matches!(
    alice.db().resource_sharing(id).expect("read"),
    Some(ResourceSharing::Shared(_))
  ));
  assert_eq!(alice.waiting(), []);
}
