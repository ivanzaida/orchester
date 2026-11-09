mod support;

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram, ModelId, ResourceKind, ReviewerDestinations, Slug, TeamReviewer, Tool},
  run::review::Probability,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use support::copied_at_version;

/// The migrations after the one that rebuilt `resources` for the team reviewer's kind (`team_reviewer`). One appended
/// later raises it, so the rewound schema stays the one before that migration.
const LATER_MIGRATIONS: u32 = 21;

#[test]
fn team_reviewer_versions_persist_without_changing_published_bodies() {
  let repo = tempfile::tempdir().expect("repo");
  std::fs::create_dir(repo.path().join(".git")).expect("git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("TEAM").expect("key"),
    Name::new("Team").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("database");
  let now = Timestamp::from_unix_millis(1);
  let reviewer = TeamReviewer::Decision {
    model: ModelId::new("decision-1").expect("model"),
    lowest_threshold: Probability::new(0.8).expect("threshold"),
    destinations: ReviewerDestinations::MembersComputers,
  };
  let resource = db
    .create_resource(
      Slug::new("team-reviewer").expect("slug"),
      Name::new("Team reviewer").expect("name"),
      &reviewer,
      now,
    )
    .expect("create resource");
  let first = db.publish_draft::<TeamReviewer>(resource.id(), now).expect("publish");
  let mut draft = first.to_draft(now);
  let mut stricter = reviewer.clone();
  if let TeamReviewer::Decision { lowest_threshold, .. } = &mut stricter {
    *lowest_threshold = Probability::new(0.9).expect("threshold");
  }
  draft.edit(stricter.clone(), now);
  db.save_draft(resource.id(), &draft).expect("save");
  db.publish_draft::<TeamReviewer>(resource.id(), now)
    .expect("second publish");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.resources(ResourceKind::TeamReviewer).expect("resources").as_slice(),
    std::slice::from_ref(&resource)
  );
  let versions = db.versions::<TeamReviewer>(resource.id()).expect("versions");
  assert_eq!(versions.len(), 2);
  assert_eq!(versions[0].body(), &reviewer);
  assert_eq!(versions[1].body(), &stricter);
}

/// The rebuild of `resources` for the team reviewer's kind keeps the published versions that refer to its existing
/// rows: a resource with two versions, in a database from before the rebuild, has both after it. (A team reviewer
/// cannot be in that database: its kind is the rebuild's.)
#[test]
fn team_reviewer_the_resources_rebuild_keeps_the_published_versions_of_existing_rows() {
  let repo = tempfile::tempdir().expect("repo");
  std::fs::create_dir(repo.path().join(".git")).expect("git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("TEAM").expect("key"),
    Name::new("Team").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("database");
  let now = Timestamp::from_unix_millis(1);
  let tool = |program: &str| {
    Tool::command(
      "",
      CommandAction::new(CommandProgram::new(program).expect("program"), Vec::new()),
    )
  };
  let resource = db
    .create_resource(
      Slug::new("lint").expect("slug"),
      Name::new("Lint").expect("name"),
      &tool("lint"),
      now,
    )
    .expect("create resource");
  let first = db.publish_draft::<Tool>(resource.id(), now).expect("publish");
  let mut draft = first.to_draft(now);
  draft.edit(tool("lint-more"), now);
  db.save_draft(resource.id(), &draft).expect("save");
  db.publish_draft::<Tool>(resource.id(), now).expect("second publish");
  drop(db);

  let current = repo.path().join(".orchester/orchester.db");
  let schema: u32 = rusqlite::Connection::open(&current)
    .expect("raw database")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("schema");
  // The same rows in a database as it was before that migration: the ones after it are not run again over it.
  let old = copied_at_version(&current, schema - LATER_MIGRATIONS - 1);
  let db = ProjectDb::open(old.path()).expect("reopen and migrate");
  assert_eq!(
    db.resources(ResourceKind::Tool).expect("resources").as_slice(),
    std::slice::from_ref(&resource)
  );
  let versions = db.versions::<Tool>(resource.id()).expect("versions");
  assert_eq!(versions.len(), 2);
  assert_eq!(versions[0].body(), &tool("lint"));
  assert_eq!(versions[1].body(), &tool("lint-more"));
}
