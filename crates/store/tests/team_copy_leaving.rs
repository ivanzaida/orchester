//! A copy of a resource of this computer's own for the team that never reached the server stays when the project
//! leaves the team, beside the resource it was copied from: each kind keeps one resource to a slug again, the copy
//! taking a free one the way a new resource does.
//! `cargo test -p orchester-store --test team_copy_leaving team_copy_leaving`.

mod support;

use std::{
  collections::{BTreeSet, HashMap},
  fs,
};

use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText, RepositoryAccess, ResourceBody,
    ResourceKind, Slug, Step, StepKey, StepKind,
  },
  sharing::MemberRole,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, SharingLink};
use support::{at, name};
use tempfile::TempDir;

/// A project shared with a team server, whose person is its owner.
fn team_project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("project");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 0,
    last_synced_at: at(100),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  (repo, db)
}

fn instruction() -> Instruction {
  Instruction::new(
    PromptText::new("Be brief.").expect("valid prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

fn pipeline() -> Pipeline {
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
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
  Pipeline::new(vec![step]).expect("a pipeline")
}

/// A resource of this computer's own with a version published, and its copy for the team waiting for the server.
fn own_and_copy<B: ResourceBody + Clone>(db: &mut ProjectDb, body: &B) -> (ResourceId, ResourceId) {
  let (own, _) = db
    .create_published(Slug::new("review").expect("slug"), name("Review"), body, at(10))
    .expect("an own resource, v1 here");
  let own = own.id();
  let copy = db.copy_for_the_team::<B>(own, at(20)).expect("the copy").id();
  (own, copy)
}

fn slugs(db: &ProjectDb, kind: ResourceKind) -> Vec<String> {
  let mut slugs: Vec<String> = db
    .kept_resources(kind)
    .expect("read")
    .iter()
    .map(|resource| resource.slug().as_str().to_owned())
    .collect();
  slugs.sort();
  slugs
}

#[test]
fn team_copy_leaving_gives_a_copy_that_never_reached_the_server_a_free_slug() {
  let (_repo, mut db) = team_project();
  let (own, copy) = own_and_copy(&mut db, &pipeline());
  let slug = |db: &ProjectDb, id| db.resource(id).expect("read").expect("kept").slug().as_str().to_owned();
  assert_eq!(
    (slug(&db, own), slug(&db, copy)),
    ("review".into(), "review".into()),
    "while the project is the team's the two share it"
  );

  db.make_local().expect("made local");

  assert_eq!(
    (slug(&db, own), slug(&db, copy)),
    ("review".into(), "review-2".into()),
    "the one that was there first keeps it"
  );
  assert_eq!(slugs(&db, ResourceKind::Pipeline), ["review", "review-2"]);
}

#[test]
fn team_copy_leaving_holds_every_kind_to_one_resource_to_a_slug() {
  let (_repo, mut db) = team_project();
  own_and_copy(&mut db, &instruction());
  db.make_local().expect("made local");
  assert_eq!(slugs(&db, ResourceKind::Instruction), ["review", "review-2"]);
}
