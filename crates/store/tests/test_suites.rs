//! Test suites in the project database (ORC-244): their bodies round-trip through drafts and
//! published versions, and the migration that makes them a resource kind keeps every resource.
//! Run with `cargo test -p orchester-store --test test_suites test_suites`.

mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  resource::{
    ArtifactGlob, CommandArg, CommandProgram, CommandTimeout, Draft, ReportFormat, ResourceKind, Slug, SuiteCommand,
    SuiteNeeds, SuitePath, SuiteVariable, SuiteVariables, TestLayer, TestReport, TestSuite, Tool, VariableName,
  },
  secret::SecretName,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;

use self::support::{at, copied_at_version, name, started_run};

fn suite(layer: TestLayer, args: &[&str]) -> TestSuite {
  let command = SuiteCommand::new(
    CommandProgram::new("npx").expect("valid program"),
    args
      .iter()
      .map(|arg| CommandArg::new(arg).expect("valid argument"))
      .collect(),
  )
  .expect("valid command");
  let report = TestReport::new(
    ReportFormat::PlaywrightJson,
    Some(SuitePath::new("results.json").expect("valid path")),
  )
  .expect("valid report");
  let variables = SuiteVariables::new(BTreeMap::from([(
    VariableName::new("CLOUD_TOKEN").expect("name"),
    SuiteVariable::Secret(SecretName::new("CLOUD_TOKEN").expect("secret name")),
  )]))
  .expect("valid variables");
  TestSuite::new(command, report, layer)
    .with_working_folder(Some(SuitePath::new("web/cloud").expect("valid path")))
    .with_variables(variables)
    .with_needs(SuiteNeeds::new(vec![Slug::new("cloud").expect("slug")]).expect("valid needs"))
    .with_artifacts(vec![ArtifactGlob::new("test-results/**").expect("valid glob")])
    .with_timeout(CommandTimeout::from_secs(900).expect("valid timeout"))
}

#[test]
fn test_suites_round_trip_through_drafts_and_versions() {
  let (repo, mut db, ..) = started_run();
  let first = suite(TestLayer::E2e, &["playwright", "test", "--grep", "{{filter}}"]);
  let id = db
    .create_resource(Slug::new("cloud-e2e").expect("slug"), name("Cloud e2e"), &first, at(10))
    .expect("create")
    .id();
  assert_eq!(db.draft::<TestSuite>(id).expect("read").expect("draft").body(), &first);
  let v1 = db.publish_draft::<TestSuite>(id, at(11)).expect("publish");
  assert_eq!((v1.number().get(), v1.body()), (1, &first));

  let second = suite(TestLayer::Unit, &["vitest", "run", "{{filter}}"]).run_always(true);
  db.save_draft(id, &Draft::new(second.clone(), Some(v1.number()), at(12)))
    .expect("save");
  let v2 = db.publish_draft::<TestSuite>(id, at(13)).expect("publish again");
  assert_eq!(v2.number().get(), 2);
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("reopen");
  let versions = db.versions::<TestSuite>(id).expect("versions");
  assert_eq!(
    versions.iter().map(|version| version.body()).collect::<Vec<_>>(),
    [&first, &second]
  );
  assert_eq!(
    db.draft::<TestSuite>(id).expect("read"),
    None,
    "publishing used the draft"
  );
  let listed: Vec<_> = db
    .resources(ResourceKind::TestSuite)
    .expect("list")
    .iter()
    .map(|resource| (resource.id(), resource.kind()))
    .collect();
  assert_eq!(listed, [(id, ResourceKind::TestSuite)]);
  assert!(matches!(
    db.draft::<Tool>(id),
    Err(StoreError::ResourceKindMismatch {
      expected: ResourceKind::Tool,
      found: ResourceKind::TestSuite,
      ..
    })
  ));
}

#[test]
fn test_suites_a_stored_body_that_breaks_a_rule_is_refused() {
  let (repo, mut db, ..) = started_run();
  let id = db
    .create_resource(
      Slug::new("cloud-e2e").expect("slug"),
      name("Cloud e2e"),
      &suite(TestLayer::E2e, &["playwright", "test"]),
      at(10),
    )
    .expect("create")
    .id();
  let raw = Connection::open(database(repo.path())).expect("open raw database");
  raw
    .execute(
      "UPDATE resource_drafts SET body = json_set(body, '$.body.report.format', 'xunit')",
      [],
    )
    .expect("store a broken body");
  assert!(matches!(db.draft::<TestSuite>(id), Err(StoreError::Corrupt { .. })));
}

#[test]
fn test_suites_migration_keeps_every_resource_and_its_order() {
  let (current, db, ..) = started_run();
  drop(db);
  let latest = schema_version(&database(current.path()));
  // The newest schema whose resources cannot be test suites, wherever later migrations moved the latest.
  let before = (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !takes_test_suites(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before test suites");
  let old = copied_at_version(&database(current.path()), before);
  let expected = resource_rows(&database(old.path()));
  assert!(!expected.is_empty(), "the started run has resources to keep");

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&database(old.path())), latest);
  let id = upgraded
    .create_resource(
      Slug::new("cloud-e2e").expect("slug"),
      name("Cloud e2e"),
      &suite(TestLayer::E2e, &["playwright", "test"]),
      at(20),
    )
    .expect("a test suite after the migration")
    .id();
  drop(upgraded);
  let rows = resource_rows(&database(old.path()));
  assert_eq!(
    rows[..expected.len()],
    expected[..],
    "rows keep their rowid and content"
  );
  assert_eq!(rows.len(), expected.len() + 1);
  assert_eq!(
    rows.last().map(|row| (row.0.as_str(), row.1.as_str())),
    Some((id.to_string().as_str(), "test_suite"))
  );
}

fn database(repo: &Path) -> std::path::PathBuf {
  repo.join(".orchester").join("orchester.db")
}

fn schema_version(database: &Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}

fn takes_test_suites(connection: &Connection) -> bool {
  let sql: String = connection
    .query_row("SELECT sql FROM sqlite_master WHERE name = 'resources'", [], |row| {
      row.get(0)
    })
    .expect("the resources table");
  sql.contains("'test_suite'")
}

/// Every resource row as (id, kind, slug, name), in `rowid` order.
fn resource_rows(database: &Path) -> Vec<(String, String, String, String)> {
  Connection::open(database)
    .expect("open")
    .prepare("SELECT id, kind, slug, name FROM resources ORDER BY rowid")
    .expect("prepare")
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
    .expect("query")
    .collect::<Result<_, _>>()
    .expect("rows")
}
