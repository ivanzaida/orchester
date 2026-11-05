//! `run-services`: a task whose run holds real services, for its Services tab (`06 · Task · Services` `cjvRr`). Its
//! draft runs *Test the services* with `orchester-fake-codex`; its brief has the fake start the stack `team`
//! (postgres, cloud-api, team-server, team-web), then cloud-web, and hold them. The services are
//! `orchester-test-command`, beside the fake: team-server writes a few log lines and crashes with code 101 about 45 s
//! after it started, which stops team-web, which needs it; cloud-web waits for an HTTP answer it never gets, so it
//! stays starting for 15 minutes. Each service is approved for every run of the project, by an earlier task's run that
//! was cancelled, so the starts ask nobody, and `fake-hold` lies next to the worktrees, so the step holds its services
//! until it is removed. Press Run on *Invite people to a team server*.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::RunId,
  resource::{CommandArg, CommandProgram},
  run::ProjectScope,
  service::{
    HttpPath, LogPattern, Readiness, ReadyCheck, ReadyTimeout, Service, ServiceApproval, ServiceName, Stack, StackName,
    UiDriver,
  },
  store::ResourceStore,
};
use orchester_workspace::agent_testing::data::{create, service_slug, stack_slug};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::{self, Published},
  run::Drive,
};

const TITLE: &str = "Invite people to a team server";
const BRIEF: &str = "Accept invites to a team. Start the stack team. Then start the service cloud-web. Hold the services.";
const CRITERIA: [&str; 1] = ["An invite accepted twice says it was accepted already."];

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

fn name(value: &str) -> Result<ServiceName, SeedError> {
  ServiceName::new(value).map_err(invalid)
}

/// `orchester-test-command`, which the runner's `test-agent` build makes beside the fake.
fn test_command(fake_codex: &Path) -> PathBuf {
  fake_codex.with_file_name(format!("orchester-test-command{}", std::env::consts::EXE_SUFFIX))
}

/// The service `value` running `program` with `arguments`, ready by `check` within `timeout` seconds, needing `needs`.
fn service(
  (value, program): (&str, &Path),
  arguments: &[&str],
  (check, timeout): (ReadyCheck, u64),
  needs: &[&str],
) -> Result<Service, SeedError> {
  let readiness = Readiness::new(check, ReadyTimeout::from_secs(timeout).map_err(invalid)?);
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument))
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let needs = needs.iter().map(|need| name(need)).collect::<Result<Vec<_>, _>>()?;
  let program = CommandProgram::new(&program.to_string_lossy()).map_err(invalid)?;
  Service::new(name(value)?, program, readiness)
    .with_arguments(arguments)
    .with_needs(needs)
    .map_err(invalid)
}

/// team-server: its first line says it listens, which it is ready by; a few requests later it panics.
const TEAM_SERVER: &[&str] = &[
  "say",
  "INFO listening on 127.0.0.1:{{port}}",
  "sleep",
  "4",
  "say",
  "INFO POST /api/teams 201 in 14 ms",
  "say",
  "INFO POST /api/teams/7f3c/invites 201 in 9 ms",
  "sleep",
  "40",
  "stderr",
  "ERROR accept invite: duplicate key value violates unique constraint \"members_pkey\"",
  "stderr",
  "thread 'tokio-runtime-worker' panicked at crates/team-server/src/invites.rs:88:41",
  "stderr",
  "called `Result::unwrap()` on an `Err` value: Conflict",
  "exit",
  "101",
];

fn services(program: &Path) -> Result<Vec<Service>, SeedError> {
  let port = || (ReadyCheck::PortOpen, 60);
  Ok(vec![
    service(
      ("postgres", program),
      &["say", "LOG database system is ready to accept connections", "listen", "{{port}}"],
      port(),
      &[],
    )?,
    service(
      ("cloud-api", program),
      &["say", "INFO cloud-api serving {{url}}", "listen", "{{port}}"],
      port(),
      &["postgres"],
    )?,
    service(
      ("team-server", program),
      TEAM_SERVER,
      (
        ReadyCheck::LogLine(LogPattern::new("^INFO listening").map_err(invalid)?),
        60,
      ),
      &["cloud-api"],
    )?,
    service(("team-web", program), &["listen", "{{port}}"], port(), &["team-server"])?.with_ui_driver(UiDriver::Native),
    service(
      ("cloud-web", program),
      &["say", "VITE starting the dev server", "sleep", "3600"],
      (ReadyCheck::Http(HttpPath::new("/").map_err(invalid)?), 900),
      &["cloud-api"],
    )?
    .with_ui_driver(UiDriver::Native),
  ])
}

/// An earlier task's run, cancelled, whose start a person approved for every run: the run the rules were made in.
fn earlier_run(kontur: &mut Kontur, pipeline: &Published) -> Result<RunId, SeedError> {
  let mut task = kontur.task(
    ("Try the team stack", "Start the stack team.", &CRITERIA),
    Some(pipeline),
    kontur.ago(120),
  )?;
  kontur.queue(&mut task, pipeline)?;
  let at = kontur.ago(110);
  let mut drive = Drive::start(kontur, &task, pipeline, at)?;
  drive.cancel()?;
  let run = drive.finish();
  kontur.seeded(&task, "cancelled; its start of the stack was approved for every run");
  Ok(run.id())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let fake_codex = kontur.fake_codex.clone().ok_or(SeedError::FakeCodexRequired)?;
  let published = kontur.ago(60);
  let connection = catalogue::codex(&mut kontur.db, Some(&fake_codex), published)?;
  let pipeline = catalogue::tester(&mut kontur.db, connection, published)?;
  let services = services(&test_command(&fake_codex))?;
  let made_in = earlier_run(kontur, &pipeline)?;
  for service in &services {
    let id = create(&mut kontur.db, service_slug(service.name()), service, published)?;
    kontur.db.publish_draft::<Service>(id, published)?;
    let scope = ProjectScope::Service(ServiceApproval::of(service));
    kontur.db.add_project_approval_rule(&scope, (made_in, "seed"), published)?;
  }
  let team = StackName::new("team").map_err(invalid)?;
  let listed = ["postgres", "cloud-api", "team-server", "team-web"]
    .iter()
    .map(|service| name(service))
    .collect::<Result<Vec<_>, _>>()?;
  let stack = Stack::new(team.clone(), listed).map_err(invalid)?;
  let id = create(&mut kontur.db, stack_slug(&team), &stack, published)?;
  kontur.db.publish_draft::<Stack>(id, published)?;
  let hold = kontur.repo.join(".orchester").join("fake-hold");
  std::fs::write(&hold, "hold").map_err(|source| SeedError::Io { path: hold, source })?;
  let task = kontur.task((TITLE, BRIEF, &CRITERIA), Some(&pipeline), kontur.ago(5))?;
  kontur.seeded(&task, "draft; Run holds the stack team and cloud-web, team-server crashes after about 45 s");
  Ok(())
}
