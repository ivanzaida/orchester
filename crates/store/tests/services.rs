//! Services and stacks in the project database (ORC-269): their bodies round-trip through drafts and published
//! versions, publishing checks their needs against the published services, and the migration that makes them
//! resource kinds keeps every resource.
//! Run with `cargo test -p orchester-store --test services services`.

mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::ResourceId,
  resource::{
    CommandArg, CommandProgram, Draft, FieldName, HttpMethod, PublishError, ResourceKind, Slug, Tool, VariableName,
  },
  secret::SecretName,
  service::{
    Capture, CapturePath, HttpPath, NeedsError, OwnedPath, Readiness, ReadyCheck, ReadyTimeout, RelativeFolder,
    RunData, RunFolder, Seed, SeedHeader, SeedRequest, SeedRequests, Service, ServiceName, ServiceValue,
    ServiceVariables, Stack, StackName, UiDriver, UiStart,
  },
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;

use self::support::{at, copied_at_version, name, started_run};

fn service_name(value: &str) -> ServiceName {
  ServiceName::new(value).expect("a kebab-case name")
}

fn service(value: &str, needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  Service::new(
    service_name(value),
    CommandProgram::new("npm").expect("a program"),
    readiness,
  )
  .with_needs(needs.iter().map(|need| service_name(need)).collect())
  .expect("needs other services once each")
}

/// A service with every part a definition has.
fn team_server() -> Service {
  let readiness = Readiness::new(
    ReadyCheck::Http(HttpPath::new("/healthz").expect("a path")),
    ReadyTimeout::from_secs(90).expect("in range"),
  );
  let variables = ServiceVariables::new(BTreeMap::from([
    (
      VariableName::new("RUST_LOG").expect("a name"),
      ServiceValue::Plain("info".into()),
    ),
    (
      VariableName::new("TEAM_TOKEN").expect("a name"),
      ServiceValue::Secret(SecretName::new("TEAM_TOKEN").expect("a secret name")),
    ),
  ]))
  .expect("variables");
  let request = SeedRequest::new(
    HttpMethod::Post,
    "{{url}}/api/teams",
    Some(r#"{"name":"QA"}"#.into()),
    vec![
      SeedHeader::new(
        "Authorization",
        ServiceValue::Secret(SecretName::new("TEAM_TOKEN").expect("name")),
      )
      .expect("header"),
    ],
    vec![Capture::new(
      FieldName::new("team_id").expect("a field"),
      CapturePath::new("$.id").expect("a path"),
    )],
  )
  .expect("a request");
  Service::new(
    service_name("team-server"),
    CommandProgram::new("cargo").expect("a program"),
    readiness,
  )
  .with_arguments(
    ["run", "--", "--port", "{{port}}"]
      .iter()
      .map(|argument| CommandArg::new(argument).expect("an argument"))
      .collect(),
  )
  .with_working_folder(RelativeFolder::new("server").expect("a folder"))
  .with_variables(variables)
  .with_needs(vec![service_name("database")])
  .expect("needs")
  .with_seed(Seed::Requests(SeedRequests::new(vec![request]).expect("requests")))
  .with_run_data(RunData::RunFolder(
    RunFolder::new("runs/{{run}}/team").expect("a folder"),
  ))
  .with_ui_driver(UiDriver::Mcp)
  .with_ui_start(UiStart::new("{{url}}/teams/{{seed.team_id}}").expect("a place"))
  .with_owned_paths(vec![OwnedPath::new("server/**").expect("a path")])
  .expect("owned paths")
}

fn stack(value: &str, services: &[&str]) -> Stack {
  Stack::new(
    StackName::new(value).expect("a name"),
    services.iter().map(|service| service_name(service)).collect(),
  )
  .expect("a stack")
}

fn create_service(db: &mut ProjectDb, service: &Service) -> ResourceId {
  let slug = Slug::new(service.name().as_str()).expect("a slug");
  db.create_resource(slug, name(service.name().as_str()), service, at(10))
    .expect("create")
    .id()
}

fn refused_needs<T: std::fmt::Debug>(result: Result<T, StoreError>) -> NeedsError {
  match result {
    Err(StoreError::Publish(PublishError::Needs(error))) => error,
    other => panic!("refused for its needs: {other:?}"),
  }
}

#[test]
fn services_and_stacks_round_trip_through_drafts_and_versions() {
  let (repo, mut db, ..) = started_run();
  let database = create_service(&mut db, &service("database", &[]));
  db.publish_draft::<Service>(database, at(11))
    .expect("publish the database");
  let first = team_server();
  let id = create_service(&mut db, &first);
  assert_eq!(db.draft::<Service>(id).expect("read").expect("draft").body(), &first);
  let v1 = db.publish_draft::<Service>(id, at(12)).expect("publish");
  assert_eq!((v1.number().get(), v1.body()), (1, &first));
  let second = first.clone().with_ui_driver(UiDriver::None).with_seed(Seed::Command {
    program: CommandProgram::new("npm").expect("a program"),
    arguments: vec![CommandArg::new("run").expect("an argument")],
  });
  db.save_draft(id, &Draft::new(second.clone(), Some(v1.number()), at(13)))
    .expect("save");
  db.publish_draft::<Service>(id, at(14)).expect("publish again");

  let full = stack("full", &["team-server", "database"]);
  let stack_id = db
    .create_resource(Slug::new("full").expect("slug"), name("Full"), &full, at(15))
    .expect("create a stack")
    .id();
  db.publish_draft::<Stack>(stack_id, at(16)).expect("publish the stack");
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("reopen");
  let versions = db.versions::<Service>(id).expect("versions");
  assert_eq!(
    versions.iter().map(|version| version.body()).collect::<Vec<_>>(),
    [&first, &second]
  );
  assert_eq!(
    db.draft::<Service>(id).expect("read"),
    None,
    "publishing used the draft"
  );
  let stacks = db.versions::<Stack>(stack_id).expect("stack versions");
  assert_eq!(stacks.iter().map(|version| version.body()).collect::<Vec<_>>(), [&full]);
  let listed: Vec<_> = db
    .resources(ResourceKind::Service)
    .expect("list")
    .iter()
    .map(|resource| resource.slug().as_str().to_owned())
    .collect();
  assert_eq!(listed, ["database", "team-server"]);
  assert_eq!(db.resources(ResourceKind::Stack).expect("list").len(), 1);
  assert!(matches!(
    db.draft::<Tool>(id),
    Err(StoreError::ResourceKindMismatch {
      expected: ResourceKind::Tool,
      found: ResourceKind::Service,
      ..
    })
  ));
}

#[test]
fn services_publishing_refuses_a_missing_need() {
  let (_repo, mut db, ..) = started_run();
  let web = create_service(&mut db, &service("web", &["api"]));
  assert_eq!(
    refused_needs(db.publish_draft::<Service>(web, at(11))),
    NeedsError::Missing {
      name: service_name("api"),
      needed_by: Some(service_name("web")),
    }
  );
  let api = create_service(&mut db, &service("api", &[]));
  assert!(
    matches!(
      refused_needs(db.publish_draft::<Service>(web, at(12))),
      NeedsError::Missing { .. }
    ),
    "a need with only a draft is not published"
  );
  assert!(
    db.draft::<Service>(web).expect("read").is_some(),
    "the refused draft stays"
  );

  db.publish_draft::<Service>(api, at(13))
    .expect("publish what is needed");
  db.publish_draft::<Service>(web, at(14)).expect("then what needs it");

  let full = db
    .create_resource(
      Slug::new("full").expect("slug"),
      name("Full"),
      &stack("full", &["web", "worker"]),
      at(15),
    )
    .expect("a stack")
    .id();
  assert_eq!(
    refused_needs(db.publish_draft::<Stack>(full, at(16))),
    NeedsError::Missing {
      name: service_name("worker"),
      needed_by: None,
    },
    "a stack names only published services"
  );
}

#[test]
fn services_publishing_refuses_a_cycle_of_needs() {
  let (_repo, mut db, ..) = started_run();
  let api = create_service(&mut db, &service("api", &[]));
  db.publish_draft::<Service>(api, at(11)).expect("publish");
  let web = create_service(&mut db, &service("web", &["api"]));
  db.publish_draft::<Service>(web, at(12)).expect("publish");

  let latest = db.latest_version::<Service>(api).expect("read").expect("a version");
  db.save_draft(
    api,
    &Draft::new(service("api", &["web"]), Some(latest.number()), at(13)),
  )
  .expect("save");
  assert_eq!(
    refused_needs(db.publish_draft::<Service>(api, at(14))),
    NeedsError::Cycle(vec![service_name("api"), service_name("web"), service_name("api")])
  );
  assert_eq!(
    db.latest_version::<Service>(api)
      .expect("read")
      .map(|version| version.number().get()),
    Some(1),
    "nothing was published"
  );
}

#[test]
fn services_a_stored_body_that_breaks_a_rule_is_refused() {
  let (repo, mut db, ..) = started_run();
  let id = create_service(&mut db, &team_server());
  let raw = Connection::open(database(repo.path())).expect("open raw database");
  raw
    .execute(
      "UPDATE resource_drafts SET body = json_set(body, '$.body.working_folder', '../outside')",
      [],
    )
    .expect("store a broken body");
  assert!(matches!(db.draft::<Service>(id), Err(StoreError::Corrupt { .. })));
}

#[test]
fn services_migration_keeps_every_resource_and_its_order() {
  let (current, db, ..) = started_run();
  drop(db);
  let latest = schema_version(&database(current.path()));
  // The newest schema whose resources cannot be services, wherever later migrations moved the latest.
  let before = (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !takes_services(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before services");
  let old = copied_at_version(&database(current.path()), before);
  let expected = resource_rows(&database(old.path()));
  assert!(!expected.is_empty(), "the started run has resources to keep");

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&database(old.path())), latest);
  let id = create_service(&mut upgraded, &service("api", &[]));
  let stack_id = upgraded
    .create_resource(
      Slug::new("full").expect("slug"),
      name("Full"),
      &stack("full", &["api"]),
      at(20),
    )
    .expect("a stack after the migration")
    .id();
  drop(upgraded);
  let rows = resource_rows(&database(old.path()));
  assert_eq!(
    rows[..expected.len()],
    expected[..],
    "rows keep their rowid and content"
  );
  assert_eq!(
    rows[expected.len()..]
      .iter()
      .map(|row| (row.0.as_str(), row.1.as_str()))
      .collect::<Vec<_>>(),
    [
      (id.to_string().as_str(), "service"),
      (stack_id.to_string().as_str(), "stack")
    ]
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

fn takes_services(connection: &Connection) -> bool {
  let sql: String = connection
    .query_row("SELECT sql FROM sqlite_master WHERE name = 'resources'", [], |row| {
      row.get(0)
    })
    .expect("the resources table");
  sql.contains("'service'")
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
