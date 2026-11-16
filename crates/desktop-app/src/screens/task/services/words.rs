//! What the Services tab says of a run's services, each a whole message of its own (`run_services.*`): the stack line
//! (`T92RJ`), a service's state, driver, port, address detail and uptime (`qWnt1` … `T0YEZ`), and the note under them
//! (`p0Oloj`).

use std::{sync::Arc, time::Duration};

use lurq::app::ctx::Ctx;
use orchester_core::{
  service::{ReadyCheck, Service, UiDriver},
  time::Timestamp,
};
use orchester_ipc::services::{RunService, RunServiceState, ServiceStarter};
use orchester_workspace::{display, tasks::detail::StepView};

/// What separates two facts of the stack line and two names of a list.
const FACTS: &str = " · ";
const LIST: &str = ", ";

/// "04 Test".
pub fn step_label(ctx: &Ctx, step: &StepView) -> Arc<str> {
  ctx.t_args(
    "run_services.step",
    [
      ("number", format!("{:02}", step.number)),
      ("step", step.name.to_string()),
    ],
  )
}

/// `names` as one list: `team`, `team and cloud-web`, `team, cloud-api and cloud-web`.
fn listed(ctx: &Ctx, names: &[&str]) -> String {
  match names {
    [] => String::new(),
    [one] => (*one).to_owned(),
    [first @ .., last] => ctx
      .t_args(
        "run_services.header.and",
        [("first", first.join(LIST)), ("last", (*last).to_owned())],
      )
      .to_string(),
  }
}

/// The stack line's title: the stacks the run started and the services it started on their own, `Stack team and
/// cloud-web`; a service another of them needs came up with that one and is not named. Without a stack, `Started web`.
pub fn title(ctx: &Ctx, services: &[RunService], definitions: &[Service]) -> Arc<str> {
  let mut stacks: Vec<&str> = Vec::new();
  for stack in services.iter().filter_map(|service| service.stack.as_deref()) {
    if !stacks.contains(&stack) {
      stacks.push(stack);
    }
  }
  let needed = |name: &str| {
    services.iter().any(|other| {
      other.stack.is_none()
        && definitions
          .iter()
          .find(|service| service.name().as_str() == other.name)
          .is_some_and(|service| service.needs().iter().any(|need| need.as_str() == name))
    })
  };
  let alone = services
    .iter()
    .filter(|service| service.stack.is_none() && !needed(&service.name))
    .map(|service| service.name.as_str());
  let names: Vec<&str> = stacks.iter().copied().chain(alone).collect();
  let key = if stacks.is_empty() {
    "run_services.header.started"
  } else {
    "run_services.header.stack"
  };
  ctx.t_args(key, [("names", listed(ctx, &names))])
}

/// The stack line's facts: how many services, who started them and when, `5 services · started by 04 Test at 15:12,
/// cloud-web by you at 15:24`.
pub fn detail(ctx: &Ctx, services: &[RunService], step: Option<&StepView>) -> String {
  let count = if services.len() == 1 {
    ctx.t("run_services.header.count.one")
  } else {
    ctx.t_args(
      "run_services.header.count.other",
      [("count", services.len().to_string())],
    )
  };
  let mut started: Vec<String> = Vec::new();
  if let Some(first) = services
    .iter()
    .filter(|service| service.started_by == ServiceStarter::Agent)
    .map(|service| service.started_at)
    .min()
  {
    let time = display::clock(first);
    started.push(match step {
      Some(step) => ctx
        .t_args(
          "run_services.header.startedBy",
          [("step", step_label(ctx, step).to_string()), ("time", time)],
        )
        .to_string(),
      None => ctx
        .t_args("run_services.header.startedByAgent", [("time", time)])
        .to_string(),
    });
  }
  started.extend(
    services
      .iter()
      .filter(|service| service.started_by == ServiceStarter::Person)
      .map(|service| {
        ctx
          .t_args(
            "run_services.header.personStarted",
            [
              ("name", service.name.clone()),
              ("time", display::clock(service.started_at)),
            ],
          )
          .to_string()
      }),
  );
  let mut facts = vec![count.to_string()];
  if !started.is_empty() {
    facts.push(started.join(LIST));
  }
  facts.join(FACTS)
}

/// The i18n key of a state's word.
pub const fn state_key(state: RunServiceState) -> &'static str {
  match state {
    RunServiceState::Starting => "run_services.state.starting",
    RunServiceState::Ready => "run_services.state.ready",
    RunServiceState::Crashed { .. } => "run_services.state.crashed",
    RunServiceState::Stopped => "run_services.state.stopped",
  }
}

/// A service's driver as its chip names it (`Chip · no UI` `AYJwd`).
pub fn driver(ctx: &Ctx, driver: UiDriver) -> Arc<str> {
  ctx.t(match driver {
    UiDriver::None => "run_services.driver.none",
    UiDriver::Native => "run_services.driver.native",
    UiDriver::Mcp => "run_services.driver.mcp",
    UiDriver::Browser => "run_services.driver.browser",
    UiDriver::Cdp => "run_services.driver.cdp",
  })
}

pub fn port(ctx: &Ctx, port: u16) -> Arc<str> {
  ctx.t_args("run_services.port", [("port", port.to_string())])
}

/// How long a service has run, `18m 02s` or `2h 05m`.
pub fn uptime(ctx: &Ctx, ran: Duration) -> Arc<str> {
  let total = ran.as_secs();
  let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
  if hours > 0 {
    ctx.t_args(
      "run_services.uptime.hours",
      [("hours", hours.to_string()), ("minutes", format!("{minutes:02}"))],
    )
  } else {
    ctx.t_args(
      "run_services.uptime.minutes",
      [("minutes", minutes.to_string()), ("seconds", format!("{seconds:02}"))],
    )
  }
}

/// What a starting service waits for, as its definition's readiness says, and for how long of its timeout:
/// `Waiting for HTTP 2xx on / · 12 s of 90 s`.
pub fn waiting(ctx: &Ctx, service: &RunService, definition: Option<&Service>, ran: Duration) -> Arc<str> {
  let elapsed = ran.as_secs().to_string();
  let Some(readiness) = definition.map(Service::readiness) else {
    return ctx.t_args("run_services.waiting.unknown", [("elapsed", elapsed)]);
  };
  let timeout = readiness.timeout().secs().to_string();
  match readiness.check() {
    ReadyCheck::PortOpen => ctx.t_args(
      "run_services.waiting.port",
      [
        ("port", service.port.to_string()),
        ("elapsed", elapsed),
        ("timeout", timeout),
      ],
    ),
    ReadyCheck::Http(path) => ctx.t_args(
      "run_services.waiting.http",
      [
        ("path", path.as_str().to_owned()),
        ("elapsed", elapsed),
        ("timeout", timeout),
      ],
    ),
    ReadyCheck::LogLine(_) => ctx.t_args(
      "run_services.waiting.line",
      [("elapsed", elapsed), ("timeout", timeout)],
    ),
  }
}

/// Why a service that does not run stopped: `Exited with code 101 at 15:24:33`, `Stopped: it needs team-server`.
pub fn ended(ctx: &Ctx, service: &RunService) -> Arc<str> {
  let time = service.ended_at.map_or_else(String::new, display::clock_seconds);
  match (service.state, service.stopped_for.as_deref()) {
    (RunServiceState::Crashed { code: Some(code) }, _) => {
      ctx.t_args("run_services.exited", [("code", code.to_string()), ("time", time)])
    }
    (RunServiceState::Crashed { code: None }, _) => ctx.t_args("run_services.ended", [("time", time)]),
    (_, Some(need)) => ctx.t_args("run_services.stoppedFor", [("name", need.to_owned())]),
    _ => ctx.t_args("run_services.stoppedAt", [("time", time)]),
  }
}

/// How long since `since`, by the clock now.
pub fn since(since: Timestamp) -> Duration {
  let millis = Timestamp::now().unix_millis().saturating_sub(since.unix_millis());
  Duration::from_millis(u64::try_from(millis).unwrap_or_default())
}
