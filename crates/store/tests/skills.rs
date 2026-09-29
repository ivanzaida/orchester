//! Skills (ORC-15): the skill resource and its reference files as blobs, the repository agent files
//! setting, drafts imported from the repository, and the migration that makes skills a resource
//! kind.

mod support;

use std::sync::Arc;

use orchester_core::{
  agent_files::{
    RepositoryAgentFiles,
    import::{Candidate, DraftContent, Found, FoundFile, candidates},
  },
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Draft, Instruction, InstructionScope, PublishError, ResourceKind, Skill, SkillFile, SkillFilePath, Slug},
  store::{BlobStore, RepositoryImportStore, ResourceStore, SettingsStore},
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

use self::support::{at, name};

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  std::fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

#[test]
fn skills_are_versioned_with_their_reference_files_in_blobs() {
  let (_repo, mut db) = project();
  let content = b"cargo test --workspace".as_slice();
  let hash = db.put_blob("text/markdown", content).expect("stored");
  assert_eq!(db.put_blob("text/markdown", content).expect("stored again"), hash);
  assert_eq!(db.blob(&hash).expect("read"), content);
  let file = SkillFile {
    path: SkillFilePath::new("references/checks.md").expect("path"),
    content: hash,
    size: content.len() as u64,
  };
  let blank = Skill::new(
    String::new(),
    String::new(),
    vec![file.clone()],
    InstructionScope::Project,
  )
  .expect("draft");
  let id = db
    .create_resource(
      Slug::new("run-checks").expect("slug"),
      name("run-checks"),
      &blank,
      at(1),
    )
    .expect("create")
    .id();
  assert!(matches!(
    db.publish_draft::<Skill>(id, at(2)),
    Err(StoreError::Publish(PublishError::BlankSkillDescription))
  ));
  let written = Skill::new(
    "Runs the checks.".into(),
    "Run them all.".into(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("draft");
  db.save_draft(id, &Draft::new(written.clone(), None, at(3)))
    .expect("save");
  let version = db.publish_draft::<Skill>(id, at(4)).expect("publish");
  assert_eq!(version.body(), &written);
  let listed: Vec<_> = db
    .resources(ResourceKind::Skill)
    .expect("list")
    .iter()
    .map(|r| r.id())
    .collect();
  assert_eq!(listed, [id]);
  assert!(db.resources(ResourceKind::Instruction).expect("list").is_empty());
  assert!(matches!(
    db.draft::<Instruction>(id),
    Err(StoreError::ResourceKindMismatch { .. })
  ));
}

#[test]
fn repository_agent_files_are_on_until_turned_off() {
  let (repo, mut db) = project();
  assert_eq!(db.repository_agent_files().expect("read"), RepositoryAgentFiles::On);
  db.save_repository_agent_files(RepositoryAgentFiles::Off).expect("save");
  drop(db);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(db.repository_agent_files().expect("read"), RepositoryAgentFiles::Off);
  db.save_repository_agent_files(RepositoryAgentFiles::On).expect("save");
  assert_eq!(db.repository_agent_files().expect("read"), RepositoryAgentFiles::On);
}

/// AGENTS.md and one skill with a reference file, as drafts: the skill's file stored first.
fn drafts(db: &mut ProjectDb) -> Vec<orchester_core::agent_files::import::RepositoryDraft> {
  let items = candidates(vec![
    Found::Instructions {
      path: "AGENTS.md".into(),
      content: b"Run the checks.".to_vec(),
    },
    Found::Skill {
      folder: ".claude/skills/run-checks".into(),
      markdown: Some(b"---\nname: run-checks\ndescription: Runs the checks.\n---\nBody".to_vec()),
      files: vec![FoundFile {
        path: SkillFilePath::new("references/checks.md").expect("path"),
        content: Arc::from(b"cargo test".as_slice()),
      }],
      unsupported: Vec::new(),
    },
  ]);
  let (mut instructions, mut skills) = (Vec::new(), Vec::new());
  items
    .iter()
    .map(|item| {
      let candidate = item.outcome.as_ref().expect("importable");
      match candidate {
        Candidate::Instruction { .. } => candidate.draft(&item.source, &mut instructions, Vec::new()),
        Candidate::Skill { files, .. } => {
          let stored = files
            .iter()
            .map(|file| SkillFile {
              path: file.path.clone(),
              content: db.put_blob("text/markdown", &file.content).expect("blob"),
              size: file.content.len() as u64,
            })
            .collect();
          candidate.draft(&item.source, &mut skills, stored)
        }
      }
      .expect("draft")
    })
    .collect()
}

#[test]
fn imported_drafts_are_created_once_with_where_they_came_from() {
  let (_repo, mut db) = project();
  let drafts = drafts(&mut db);
  let key = IdempotencyKey::generate();
  let created = db.import_repository_drafts(key, &drafts, at(5)).expect("import");
  assert_eq!(
    created.iter().map(|resource| resource.kind()).collect::<Vec<_>>(),
    [ResourceKind::Instruction, ResourceKind::Skill]
  );
  assert_eq!(
    db.import_repository_drafts(key, &drafts, at(6)).expect("retried"),
    created,
    "a retried import creates nothing new"
  );
  let skill = created[1].id();
  let draft = db.draft::<Skill>(skill).expect("read").expect("a draft");
  let DraftContent::Skill(expected) = &drafts[1].content else {
    panic!("a skill")
  };
  assert_eq!(draft.body(), expected);
  assert_eq!(draft.based_on(), None);
  assert_eq!(
    db.latest_version::<Skill>(skill).expect("read"),
    None,
    "nothing is published"
  );
  assert_eq!(db.blob(&draft.body().files()[0].content).expect("blob"), b"cargo test");
  let origin = db.repository_origin(skill).expect("read").expect("imported");
  assert_eq!(
    (origin.source.as_str(), origin.imported_at),
    (".claude/skills/run-checks", at(5))
  );
  assert_eq!(
    db.repository_origin(created[0].id())
      .expect("read")
      .expect("imported")
      .source,
    "AGENTS.md"
  );
}

#[test]
fn an_import_whose_slug_is_taken_creates_nothing() {
  let (_repo, mut db) = project();
  let drafts = drafts(&mut db);
  let taken = Skill::new(String::new(), String::new(), Vec::new(), InstructionScope::Project).expect("draft");
  db.create_resource(Slug::new("run-checks").expect("slug"), name("Taken"), &taken, at(1))
    .expect("create");
  let refused = db.import_repository_drafts(IdempotencyKey::generate(), &drafts, at(2));
  assert!(matches!(
    refused,
    Err(StoreError::DuplicateSlug {
      kind: ResourceKind::Skill,
      ..
    })
  ));
  assert!(db.resources(ResourceKind::Instruction).expect("list").is_empty());
}

#[test]
fn skills_migration_keeps_every_resource_and_its_order() {
  let repo = tempfile::tempdir().expect("temporary directory");
  let old = ProjectDb::create_at_version(repo.path(), 22).expect("schema 22");
  old
    .execute_batch(
      "INSERT INTO project (id, key, name) VALUES ('01920000-0000-7000-8000-000000000001', 'KON', 'Kontur');
       INSERT INTO resources (id, kind, slug, name, created_at) VALUES
         ('01920000-0000-7000-8000-00000000000b', 'tool', 'b-tool', 'B tool', 1),
         ('01920000-0000-7000-8000-00000000000a', 'tool', 'a-tool', 'A tool', 2);
       INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES
         ('01920000-0000-7000-8000-00000000000b', '{}', NULL, 1);",
    )
    .expect("rows of schema 22");
  drop(old);
  let db = ProjectDb::open(repo.path()).expect("migrate");
  let tools: Vec<String> = db
    .resources(ResourceKind::Tool)
    .expect("list")
    .iter()
    .map(|resource| resource.slug().as_str().to_owned())
    .collect();
  assert_eq!(tools, ["a-tool", "b-tool"]);
  drop(db);
  let connection = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  let order: Vec<String> = connection
    .prepare("SELECT slug FROM resources ORDER BY rowid")
    .expect("prepare")
    .query_map([], |row| row.get(0))
    .expect("query")
    .collect::<Result<_, _>>()
    .expect("rows");
  assert_eq!(order, ["b-tool", "a-tool"], "rows keep their rowid");
  let drafts: i64 = connection
    .query_row("SELECT COUNT(*) FROM resource_drafts", [], |row| row.get(0))
    .expect("count");
  assert_eq!(drafts, 1);
}
