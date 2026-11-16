//! Flaky tests and quarantine (ORC-259) through the workspace API the Flaky tests page uses
//! (`agent_testing::flaky`) and Orchester's MCP server (`list_flaky_tests`, `quarantine_test`, `release_test`): the
//! flaky history the Test steps kept, a test watched at three runs, and the person's move to quarantine and back —
//! quarantining waits for the person's switch, releasing does not.
//! Run with `cargo test -p orchester-workspace --test flaky_tests flaky_tests`.

mod support;

use std::sync::Arc;

use orchester_core::{
  id::RunId,
  resource::Slug,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  test_step::flaky::{FlakyKind, FlakyOutcome, Standing, TestKey},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  agent_testing::flaky::{self, QuarantineError},
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use self::support::{Fixture, at};

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// Kontur with three runs, whose Test steps saw `subtracts` in `unit` flaky in two of them.
struct Flaky {
  fixture: Fixture,
  runs: Vec<RunId>,
}

impl Flaky {
  fn new() -> Self {
    let mut fixture = Fixture::new();
    let task = fixture.task("Sign in", at(10));
    let runs: Vec<RunId> = (0..3).map(|_| fixture.pending_run(task, at(11)).id()).collect();
    let mut flaky = Self { fixture, runs };
    flaky.saw(0);
    flaky.saw(1);
    flaky
  }

  /// The Test step of run `index` saw `subtracts` flaky, now.
  fn saw(&mut self, index: usize) {
    let outcome = FlakyOutcome {
      test: subtracts(),
      kind: FlakyKind::Flaky,
      run: self.runs[index],
      at: Timestamp::now(),
    };
    self
      .fixture
      .db
      .record_flaky_outcomes(&[outcome])
      .expect("the runner keeps it");
  }

  fn root(&self) -> &std::path::Path {
    self.fixture.repo.path()
  }
}

fn subtracts() -> TestKey {
  TestKey::new(Slug::new("unit").expect("slug"), "subtracts").expect("test")
}

/// The app's session with the project, whose tools are called as the app's MCP server calls them.
struct Desktop {
  workspace: Workspace,
  services: app_mcp::Services,
  runtime: Runtime,
  _data: TempDir,
}

impl Desktop {
  fn new(flaky: &Flaky) -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(flaky.root()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      flaky.root().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)), version);
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      workspace,
      services,
      runtime,
      _data: data,
    }
  }

  fn call(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
    let translate = Translate::new(|message| message.key.to_owned());
    let Value::Object(args) = args.clone() else {
      panic!("arguments are an object");
    };
    self
      .runtime
      .block_on(app_mcp::setup::call(&self.services, &translate, name, args))
      .expect("a setup tool")
  }

  fn ok(&self, name: &str, args: &Value) -> Value {
    self
      .call(name, args)
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn refused(&self, name: &str, args: &Value) -> Value {
    match self.call(name, args) {
      Ok(value) => panic!("{name} did what it should refuse: {value}"),
      Err(error) => error.to_json(|message| message.key.to_owned())["error"].clone(),
    }
  }

  fn let_automation_widen(&self, on: bool) {
    self
      .workspace
      .set_automation_changes_permissions(on)
      .expect("a person turns it on or off");
  }
}

fn named(test: &str) -> Value {
  json!({ "project": "Kontur", "suite": "unit", "test": test })
}

#[test]
fn flaky_tests_a_test_is_watched_at_three_runs_and_the_person_quarantines_it() {
  let mut kontur = Flaky::new();
  let tests = flaky::read(kontur.root(), Timestamp::now()).expect("read");
  assert_eq!(tests.len(), 1);
  assert_eq!(
    (&tests[0].test, tests[0].standing, tests[0].flaky_runs),
    (&subtracts(), Standing::Seen, 2)
  );

  kontur.saw(2);
  let tests = flaky::read(kontur.root(), Timestamp::now()).expect("read");
  assert_eq!((tests[0].standing, tests[0].flaky_runs), (Standing::Watched, 3));
  let history = ProjectDb::open(kontur.root())
    .expect("open")
    .flaky_outcomes()
    .expect("history");
  let runs: Vec<RunId> = history.iter().map(|outcome| outcome.run).collect();
  assert_eq!(runs, kontur.runs, "each outcome with its run, oldest first");

  let now = Timestamp::now();
  assert!(flaky::quarantine(kontur.root(), &subtracts(), now).expect("quarantined"));
  assert!(
    !flaky::quarantine(kontur.root(), &subtracts(), Timestamp::now()).expect("again"),
    "it was quarantined already"
  );
  let tests = flaky::read(kontur.root(), Timestamp::now()).expect("read");
  assert_eq!(tests[0].standing, Standing::Quarantined { since: now });
  let quarantined = ProjectDb::open(kontur.root())
    .expect("open")
    .quarantined_tests()
    .expect("read");
  assert_eq!(quarantined, [(subtracts(), now)]);

  let unknown = TestKey::new(Slug::new("unit").expect("slug"), "adds").expect("test");
  assert!(matches!(
    flaky::quarantine(kontur.root(), &unknown, now),
    Err(QuarantineError::NotFlaky)
  ));

  assert!(flaky::release(kontur.root(), &subtracts()).expect("released"));
  assert!(!flaky::release(kontur.root(), &subtracts()).expect("again"));
  let tests = flaky::read(kontur.root(), Timestamp::now()).expect("read");
  assert_eq!(tests[0].standing, Standing::Watched, "back to watched");
}

#[test]
fn flaky_tests_quarantine_through_mcp_needs_the_persons_switch_and_release_does_not() {
  let kontur = Flaky::new();
  let desktop = Desktop::new(&kontur);
  let listed = desktop.ok("list_flaky_tests", &json!({ "project": "Kontur" }));
  assert_eq!(
    listed["watch"],
    json!({ "after_runs": 3, "window_days": 14 }),
    "{listed}"
  );
  assert_eq!(listed["tests"][0]["suite"], "unit", "{listed}");
  assert_eq!(listed["tests"][0]["test"], "subtracts", "{listed}");
  assert_eq!(listed["tests"][0]["standing"], "seen", "{listed}");
  assert_eq!(listed["tests"][0]["flaky_runs"], 2, "{listed}");

  let refused = desktop.refused("quarantine_test", &named("subtracts"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert_eq!(
    refused["reasons"],
    json!(["appMcp.errors.personOnly", "appMcp.flakyTests.quarantine"])
  );
  assert!(
    ProjectDb::open(kontur.root())
      .expect("open")
      .quarantined_tests()
      .expect("read")
      .is_empty(),
    "nothing was quarantined"
  );

  desktop.let_automation_widen(true);
  let quarantined = desktop.ok("quarantine_test", &named("subtracts"));
  assert_eq!(quarantined["changed"], true, "{quarantined}");
  assert_eq!(quarantined["test"]["standing"], "quarantined", "{quarantined}");
  assert!(quarantined["test"]["quarantined_since"].is_i64(), "{quarantined}");
  let again = desktop.ok("quarantine_test", &named("subtracts"));
  assert_eq!(again["changed"], false, "{again}");
  let not_flaky = desktop.refused("quarantine_test", &named("adds"));
  assert_eq!(not_flaky["code"], "not_flaky", "{not_flaky}");
  let blank = desktop.refused("quarantine_test", &named(" "));
  assert_eq!(blank["code"], "invalid_test", "{blank}");

  // Releasing holds runs to the test again, so it needs no switch.
  desktop.let_automation_widen(false);
  let released = desktop.ok("release_test", &named("subtracts"));
  assert_eq!(released["changed"], true, "{released}");
  assert_eq!(released["test"]["standing"], "seen", "{released}");
  assert_eq!(desktop.ok("release_test", &named("subtracts"))["changed"], false);
  assert_eq!(desktop.ok("release_test", &named("adds"))["test"], Value::Null);
}
