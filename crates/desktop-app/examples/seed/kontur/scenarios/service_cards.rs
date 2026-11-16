//! `service-cards`: one task per card a run's services put in its Conversation (`Agent testing · redesign` `eWydp`),
//! each a Feature delivery run at work on Implement, recorded rather than run:
//!
//! - *Start the team web app*: the agent's `start_service team-web` waits for the person (`hvQDS`), its step held
//!   waiting so it can be answered without a runner. postgres and cloud-api are approved for every run as they are,
//!   team-server as it was before its arguments changed, team-web never.
//! - *Invite people to a team server*: cloud-api crashed with code 1 after it was ready, and Orchester stopped
//!   cloud-web, which needs it (`FwHrv`).
//! - *Seed the team database*: postgres's start failed because Docker did not answer (10a of `oXVuM`).
//! - *Check the invite page*: team-web was not ready in 90 s (10c).
//! - *Run the team stack*: a start that waits for 4 *Services at once* slots (10d).
//!
//! The services are published, so a card's Edit opens a service's page. Without a runner the runs read interrupted;
//! the notes stay as recorded.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::ProjectId,
  orchester_tools::{self, OrchesterTool},
  resource::{CommandArg, CommandProgram},
  run::{Invocation, PendingToolApproval, ProjectScope, Run},
  service::{
    HttpPath, Readiness, ReadyAsked, ReadyCheck, ReadyTimeout, Service, ServiceApproval, ServiceCrash, ServiceName,
    ServiceSlotHolder, ServiceSlotWait, ServiceStartFailure, SlotStep, SlotUse, StartFailureCause, start_target,
  },
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::agent_testing::data::{create, service_slug};

use super::cards::{at_work, hold};
use crate::kontur::{Kontur, SeedError};

const CRITERIA: [&str; 1] = ["Invites work end to end."];
/// The agent that asks, Implement's.
const AGENT: &str = "Builder";

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

/// The service `name` running `program` with `arguments`, ready by an HTTP answer on `/healthz` within 90 s, needing
/// `needs`.
fn service(name: &str, (program, arguments): (&str, &[&str]), needs: &[&str]) -> Result<Service, SeedError> {
  let check = ReadyCheck::Http(HttpPath::new("/healthz").map_err(invalid)?);
  let readiness = Readiness::new(check, ReadyTimeout::from_secs(90).map_err(invalid)?);
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument))
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let needs = needs
    .iter()
    .map(|need| ServiceName::new(need).map_err(invalid))
    .collect::<Result<Vec<_>, _>>()?;
  Service::new(
    ServiceName::new(name).map_err(invalid)?,
    CommandProgram::new(program).map_err(invalid)?,
    readiness,
  )
  .with_arguments(arguments)
  .with_needs(needs)
  .map_err(invalid)
}

/// postgres, cloud-api, team-server and team-web, in the order a start of team-web starts them.
fn services() -> Result<[Service; 4], SeedError> {
  Ok([
    service(
      "postgres",
      ("docker", &["run", "--rm", "-p", "{{port}}:5432", "postgres:16"]),
      &[],
    )?,
    service(
      "cloud-api",
      ("cargo", &["run", "-p", "orchester-cloud", "--", "--port", "{{port}}"]),
      &["postgres"],
    )?,
    service(
      "team-server",
      (
        "cargo",
        &["run", "-p", "orchester-team-server", "--", "--port", "{{port}}"],
      ),
      &["cloud-api"],
    )?,
    service(
      "team-web",
      ("npm", &["run", "dev", "--port", "{{port}}"]),
      &["team-server"],
    )?,
  ])
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  let published = kontur.ago(400);
  let services = services()?;
  for service in &services {
    let id = create(&mut kontur.db, service_slug(service.name()), service, published)?;
    kontur.db.publish_draft::<Service>(id, published)?;
  }
  let notes = [
    ("Invite people to a team server", Some(crashed()), "cloud-api crashed"),
    (
      "Seed the team database",
      Some(docker()),
      "postgres could not start: Docker isn't running",
    ),
    (
      "Check the invite page",
      Some(not_ready()),
      "team-web was not ready in 90 s",
    ),
    (
      "Run the team stack",
      None,
      "its start waits for 4 Services at once slots: two runs hold 6 of 8",
    ),
  ];
  let mut keys = Vec::new();
  for (index, (title, problem, state)) in notes.into_iter().enumerate() {
    let minutes = 200 - i64::try_from(index).expect("a short list") * 30;
    let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), kontur.ago(minutes + 20))?;
    let (run, invocation) = at_work(kontur, &mut task, pipeline, kontur.ago(minutes))?;
    let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 2 * 60_000);
    let problem = match problem {
      Some(problem) => problem,
      None => slots_busy(kontur.db.project().id(), &keys, at),
    };
    kontur
      .db
      .append_activity(&invocation, &[(at, AgentEvent::RunnerProblem { problem })])?;
    keys.push((task.key().to_string(), run.number().get()));
    kontur.seeded(&task, state);
  }
  let title = "Start the team web app";
  let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), kontur.ago(50))?;
  let (run, invocation) = at_work(kontur, &mut task, pipeline, kontur.ago(30))?;
  approve_services(kontur, (&run, &invocation), &services)?;
  hold(kontur, run, (&invocation, true))?;
  kontur.seeded(
    &task,
    "start_service team-web waits for you: team-server changed, team-web never ran",
  );
  Ok(())
}

/// Stack team's start waits for 4 of the computer's 8 *Services at once* slots, at `at`: 6 are held by the runs of
/// `keys`' first two tasks (stack cloud for 6 minutes in 04 Test; desktop-qa, cloud-api and postgres for half a minute
/// in 03 Implement), so 2 are free.
fn slots_busy(project: ProjectId, keys: &[(String, u32)], at: Timestamp) -> RunnerProblem {
  let holder = |(task, run): &(String, u32), (step, name): (u32, &str), what: SlotUse, since: i64| ServiceSlotHolder {
    project,
    task: task.clone(),
    run: *run,
    step: Some(SlotStep {
      number: step,
      name: name.to_owned(),
    }),
    what,
    slots: 3,
    since: Timestamp::from_unix_millis(at.unix_millis() - since),
  };
  let names = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
  let holders = keys
    .get(..2)
    .map(|keys| {
      vec![
        holder(
          &keys[0],
          (4, "Test"),
          SlotUse {
            stack: Some("cloud".to_owned()),
            services: names(&["postgres", "cloud-api", "cloud-web"]),
          },
          6 * 60_000,
        ),
        holder(
          &keys[1],
          (3, "Implement"),
          SlotUse {
            stack: None,
            services: names(&["desktop-qa", "cloud-api", "postgres"]),
          },
          30_000,
        ),
      ]
    })
    .unwrap_or_default();
  RunnerProblem::WaitingForServiceSlots(Box::new(ServiceSlotWait {
    slots: 4,
    share: 8,
    limit: 8,
    starts: Some(SlotUse {
      stack: Some("team".to_owned()),
      services: names(&["postgres", "cloud-api", "team-server", "team-web"]),
    }),
    free: Some(2),
    held: Some(6),
    holders,
  }))
}

/// cloud-api exited with code 1 1.4 s after it was started, and Orchester stopped cloud-web, which needs it.
fn crashed() -> RunnerProblem {
  RunnerProblem::ServiceCrashed(ServiceCrash {
    service: "cloud-api".to_owned(),
    exit_code: Some(1),
    uptime_ms: 1400,
    lines: vec![
      "1 out| connecting to postgres://127.0.0.1:41870/cloud".to_owned(),
      "2 err| Error: db error: FATAL: the database system is starting up".to_owned(),
    ],
    stopped: vec!["cloud-web".to_owned()],
  })
}

/// postgres's start failed at once: Docker's engine did not answer; what needs it was not started.
fn docker() -> RunnerProblem {
  RunnerProblem::ServiceStartFailed(Box::new(ServiceStartFailure {
    service: "postgres".to_owned(),
    cause: StartFailureCause::DockerNotAnswering {
      detail: "Docker is not available: the Docker engine did not answer `docker version`: error during connect"
        .to_owned(),
    },
    lines: Vec::new(),
    not_started: vec!["cloud-api".to_owned(), "team-server".to_owned(), "team-web".to_owned()],
    running: Vec::new(),
  }))
}

/// team-web listened on localhost only and never answered its readiness check; what it needs keeps running.
fn not_ready() -> RunnerProblem {
  RunnerProblem::ServiceStartFailed(Box::new(ServiceStartFailure {
    service: "team-web".to_owned(),
    cause: StartFailureCause::NotReady {
      secs: 90,
      asked: ReadyAsked::Http {
        url: "http://127.0.0.1:41873/healthz".to_owned(),
        path: "/healthz".to_owned(),
      },
    },
    lines: vec![
      "1 out| > vite --port 41873".to_owned(),
      "2 out| VITE v5.4.2 ready in 812 ms".to_owned(),
      "3 out| ➜ Local: http://localhost:41873/".to_owned(),
    ],
    not_started: Vec::new(),
    running: vec!["team-server".to_owned(), "cloud-api".to_owned(), "postgres".to_owned()],
  }))
}

/// The agent's `start_service team-web` asks for `services`, two minutes into its turn: postgres and cloud-api are
/// approved for every run as they are, team-server as it was before its arguments changed.
fn approve_services(
  kontur: &mut Kontur,
  (run, invocation): (&Run, &Invocation),
  services: &[Service; 4],
) -> Result<(), SeedError> {
  let earlier = service(
    "team-server",
    ("cargo", &["run", "-p", "orchester-team-server"]),
    &["cloud-api"],
  )?;
  let made = kontur.ago(300);
  for approval in [
    ServiceApproval::of(&services[0]),
    ServiceApproval::of(&services[1]),
    ServiceApproval::of(&earlier),
  ] {
    kontur
      .db
      .add_project_approval_rule(&ProjectScope::Service(approval), (run.id(), "seed"), made)?;
  }
  let asked: Vec<ServiceApproval> = services.iter().map(ServiceApproval::of).collect();
  let request = PendingToolApproval {
    id: format!("{}:start_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command: false,
    scope: Some(r#"{"name":"team-web"}"#.to_owned()),
    server: orchester_tools::SERVER.to_owned(),
    action: OrchesterTool::StartService.as_str().to_owned(),
    target: Some(start_target(&asked)),
    requested_at: Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 2 * 60_000),
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  Ok(())
}
