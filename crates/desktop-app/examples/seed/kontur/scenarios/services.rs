//! `services`: Project settings › Services with something in every group (`01 · Services` `G1sdS`): six services,
//! five of them published (`team-web` only a draft, filled as its page draws it, `02` `TGRTt`), three stacks and five
//! test suites of every layer. Any scenario without services shows the tab's empty state (`01b` `CdjWu`).

use std::collections::BTreeMap;

use orchester_core::{
  resource::{
    CommandArg, CommandProgram, CommandTimeout, FieldName, HttpMethod, ReportFormat, Slug, SuiteCommand, SuiteNeeds,
    SuitePath, TestLayer, TestReport, TestSuite, VariableName,
  },
  secret::SecretName,
  service::{
    Capture, CapturePath, HttpPath, OwnedPath, Readiness, ReadyCheck, ReadyTimeout, RelativeFolder, RunData, RunFolder,
    Seed, SeedRequest, SeedRequests, Service, ServiceName, ServiceValue, ServiceVariables, Stack, StackName, UiDriver,
    UiStart,
  },
  store::ResourceStore,
};
use orchester_workspace::agent_testing::data::{create, service_slug, stack_slug};

use crate::kontur::{Kontur, SeedError};

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

fn name(value: &str) -> Result<ServiceName, SeedError> {
  ServiceName::new(value).map_err(invalid)
}

/// `value`, started with `npm run <value>`, ready once its port opens, needing `needs` and owning `owns`.
fn service(value: &str, needs: &[&str], owns: &[&str]) -> Result<Service, SeedError> {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(60).map_err(invalid)?);
  let arguments = ["run", value]
    .into_iter()
    .map(CommandArg::new)
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let needs = needs.iter().map(|need| name(need)).collect::<Result<Vec<_>, _>>()?;
  let owns = owns
    .iter()
    .map(|path| OwnedPath::new(path))
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  Service::new(name(value)?, CommandProgram::new("npm").map_err(invalid)?, readiness)
    .with_arguments(arguments)
    .with_needs(needs)
    .map_err(invalid)?
    .with_owned_paths(owns)
    .map_err(invalid)
}

fn suite(
  program: &str,
  (format, report): (ReportFormat, Option<&str>),
  (layer, needs, minutes): (TestLayer, &[&str], u64),
) -> Result<TestSuite, SeedError> {
  let command = SuiteCommand::new(
    CommandProgram::new(program).map_err(invalid)?,
    vec![CommandArg::new("test").map_err(invalid)?],
  )
  .map_err(invalid)?;
  let report = report.map(SuitePath::new).transpose().map_err(invalid)?;
  let needs = needs
    .iter()
    .map(|need| Slug::new(need).map_err(invalid))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(
    TestSuite::new(command, TestReport::new(format, report).map_err(invalid)?, layer)
      .with_needs(SuiteNeeds::new(needs).map_err(invalid)?)
      .with_timeout(CommandTimeout::from_secs(minutes * 60).map_err(invalid)?),
  )
}

/// `team-web` as its page draws it (`02 · Service editor` `TGRTt`): `npm run dev --port {{port}}` in `web/team`, the
/// team server's URL and its port as plain variables and its session secret by name, ready on `/healthz` within 90 s,
/// seeded by three requests that capture a team and an invite, its files in a run folder of its own, opened by the
/// native driver at `{{url}}/teams` (the design's *browser* is not offered yet) and owning two path patterns.
fn team_web() -> Result<Service, SeedError> {
  let arguments = ["run", "dev", "--port", "{{port}}"]
    .into_iter()
    .map(CommandArg::new)
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let plain = |text: &str| ServiceValue::Plain(text.to_owned());
  let variables = [
    ("VITE_API_URL", plain("{{services.team-server.url}}")),
    ("PORT", plain("{{port}}")),
    (
      "SESSION_SECRET",
      ServiceValue::Secret(SecretName::new("TEAM_SESSION_SECRET").map_err(invalid)?),
    ),
  ]
  .into_iter()
  .map(|(name, value)| Ok((VariableName::new(name).map_err(invalid)?, value)))
  .collect::<Result<BTreeMap<_, _>, SeedError>>()?;
  let readiness = Readiness::new(
    ReadyCheck::Http(HttpPath::new("/healthz").map_err(invalid)?),
    ReadyTimeout::from_secs(90).map_err(invalid)?,
  );
  let owned = ["web/team/**", "packages/team-ui/**"]
    .into_iter()
    .map(OwnedPath::new)
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let service = Service::new(
    name("team-web")?,
    CommandProgram::new("npm").map_err(invalid)?,
    readiness,
  )
  .with_arguments(arguments)
  .with_working_folder(RelativeFolder::new("web/team").map_err(invalid)?)
  .with_variables(ServiceVariables::new(variables).map_err(invalid)?)
  .with_seed(team_seed()?)
  .with_run_data(RunData::RunFolder(RunFolder::new("team-web").map_err(invalid)?))
  .with_ui_driver(UiDriver::Native)
  .with_ui_start(UiStart::new("{{url}}/teams").map_err(invalid)?)
  .with_needs(vec![name("team-server")?])
  .map_err(invalid)?;
  service.with_owned_paths(owned).map_err(invalid)
}

/// `team-web`'s seed (`Requests` `n6vEc2`): a team, an invite to it, and the invite accepted.
fn team_seed() -> Result<Seed, SeedError> {
  let capture = |name: &str, path: &str| -> Result<Capture, SeedError> {
    Ok(Capture::new(
      FieldName::new(name).map_err(invalid)?,
      CapturePath::new(path).map_err(invalid)?,
    ))
  };
  let request = |url: &str, body: Option<&str>, captures: Vec<Capture>| {
    SeedRequest::new(HttpMethod::Post, url, body.map(str::to_owned), Vec::new(), captures).map_err(invalid)
  };
  let requests = vec![
    request(
      "{{services.team-server.url}}/api/teams",
      Some(r#"{"name": "QA team"}"#),
      vec![capture("team_id", "$.id")?],
    )?,
    request(
      "{{services.team-server.url}}/api/teams/{{seed.team_id}}/invites",
      Some(r#"{"email": "qa@example.test"}"#),
      vec![capture("invite_token", "$.token")?],
    )?,
    request(
      "{{services.team-server.url}}/api/invites/{{seed.invite_token}}/accept",
      None,
      Vec::new(),
    )?,
  ];
  Ok(Seed::Requests(SeedRequests::new(requests).map_err(invalid)?))
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let services = [
    (
      service("postgres", &[], &[])?.with_run_data(RunData::DatabaseContainer),
      true,
    ),
    (service("cloud-api", &["postgres"], &["crates/cloud/**"])?, true),
    (
      service("cloud-web", &["cloud-api"], &["web/cloud/**"])?.with_ui_driver(UiDriver::Native),
      true,
    ),
    (
      service("team-server", &["cloud-api"], &["crates/team-server/**"])?,
      true,
    ),
    (team_web()?, false),
    (
      service("desktop-qa", &["cloud-api"], &[])?.with_ui_driver(UiDriver::Mcp),
      true,
    ),
  ];
  for (body, publish) in services {
    let id = create(&mut kontur.db, service_slug(body.name()), &body, kontur.now)?;
    if publish {
      kontur.db.publish_draft::<Service>(id, kontur.now)?;
    }
  }
  for (stack, listed) in [
    ("cloud", &["postgres", "cloud-api", "cloud-web"][..]),
    ("team", &["postgres", "cloud-api", "team-server", "team-web"][..]),
    (
      "full",
      &[
        "postgres",
        "cloud-api",
        "cloud-web",
        "team-server",
        "team-web",
        "desktop-qa",
      ][..],
    ),
  ] {
    let stack = StackName::new(stack).map_err(invalid)?;
    let listed = listed
      .iter()
      .map(|service| name(service))
      .collect::<Result<Vec<_>, _>>()?;
    let body = Stack::new(stack.clone(), listed).map_err(invalid)?;
    create(&mut kontur.db, stack_slug(&stack), &body, kontur.now)?;
  }
  let suites = [
    (
      "lint",
      suite("cargo", (ReportFormat::CargoJson, None), (TestLayer::Lint, &[], 5))?,
    ),
    (
      "rust-unit",
      suite("cargo", (ReportFormat::CargoJson, None), (TestLayer::Unit, &[], 10))?,
    ),
    (
      "web-unit",
      suite("npx", (ReportFormat::VitestJson, None), (TestLayer::Unit, &[], 5))?,
    ),
    (
      "web-e2e",
      suite(
        "npx",
        (ReportFormat::PlaywrightJson, Some("results.json")),
        (TestLayer::E2e, &["cloud"], 15),
      )?,
    ),
    (
      "team-e2e",
      suite(
        "npx",
        (ReportFormat::PlaywrightJson, Some("results.json")),
        (TestLayer::E2e, &["team"], 15),
      )?,
    ),
  ];
  for (slug, body) in suites {
    create(&mut kontur.db, Slug::new(slug).map_err(invalid)?, &body, kontur.now)?;
  }
  Ok(())
}
