use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{ModelId, ResourceKind, ReviewerDestinations, Slug, TeamReviewer},
  run::review::Probability,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

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
  // Reapply the new table rebuild with published versions referring to existing resource rows.
  let raw = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("raw database");
  let schema: u32 = raw
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("schema");
  raw
    .pragma_update(None, "user_version", schema - 1)
    .expect("reapply migration");
  drop(raw);
  let db = ProjectDb::open(repo.path()).expect("reopen and migrate");
  assert_eq!(
    db.resources(ResourceKind::TeamReviewer).expect("resources").as_slice(),
    std::slice::from_ref(&resource)
  );
  let versions = db.versions::<TeamReviewer>(resource.id()).expect("versions");
  assert_eq!(versions.len(), 2);
  assert_eq!(versions[0].body(), &reviewer);
  assert_eq!(versions[1].body(), &stricter);
}
