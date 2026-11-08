//! What a row of Services says under its name (`Hint` `c2xbY`, `Htg11`, `m8Lt7`): facts of the service, stack or
//! suite, each a whole message of its own, joined by ` · ` as the design writes them.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  resource::{CommandTimeout, ReportFormat, TestLayer, TestSuite},
  service::{RunData, Service, ServiceName, Stack, StartPlan, UiDriver},
};
use orchester_workspace::agent_testing::data::Entry;

/// What separates two facts of a hint, and two steps of a stack's start.
const FACTS: &str = " · ";
const STEPS: &str = " → ";
pub(super) const LIST: &str = ", ";

/// A service's driver as its chip names it (`Chip · no UI` `GCK3E`).
pub(super) fn driver(ctx: &Ctx, driver: UiDriver) -> Arc<str> {
  ctx.t(match driver {
    UiDriver::None => "project_settings.services.driver.none",
    UiDriver::Native => "project_settings.services.driver.native",
    UiDriver::Mcp => "project_settings.services.driver.mcp",
    UiDriver::Browser => "project_settings.services.driver.browser",
    UiDriver::Cdp => "project_settings.services.driver.cdp",
  })
}

/// A suite's layer as its chip names it (`Chip · lint` `b7yybX`).
pub(super) fn layer(ctx: &Ctx, layer: TestLayer) -> Arc<str> {
  ctx.t(match layer {
    TestLayer::Lint => "project_settings.services.layer.lint",
    TestLayer::Unit => "project_settings.services.layer.unit",
    TestLayer::E2e => "project_settings.services.layer.e2e",
  })
}

/// What a service needs, the paths it owns and the data a run gets of its own: `needs postgres · owns crates/cloud/**`.
pub(super) fn service_hint(ctx: &Ctx, service: &Service) -> String {
  let needs = if service.needs().is_empty() {
    ctx.t("project_settings.services.needsNothing")
  } else {
    let names = service.needs().iter().map(ServiceName::as_str).collect::<Vec<_>>();
    ctx.t_args("project_settings.services.needs", [("names", names.join(LIST))])
  };
  let owns = (!service.owned_paths().is_empty()).then(|| {
    let paths = service
      .owned_paths()
      .iter()
      .map(|path| path.as_str())
      .collect::<Vec<_>>();
    ctx.t_args("project_settings.services.owns", [("paths", paths.join(LIST))])
  });
  let data = match service.run_data() {
    RunData::None => None,
    RunData::DatabaseContainer => Some(ctx.t("project_settings.services.runData.database")),
    RunData::RunFolder(_) => Some(ctx.t("project_settings.services.runData.folder")),
  };
  join([Some(needs), owns, data])
}

/// The order `stack` starts its services in, from the project's `services`: the steps one after another, the services
/// of one step together; every service the project defines in a stack of more than one is `all 6 services`. When its
/// needs cannot be resolved, the services it lists.
pub(super) fn stack_hint(ctx: &Ctx, stack: &Stack, services: &[Service]) -> String {
  let listed = stack.services();
  if listed.len() > 1 && listed.len() == services.len() && services.iter().all(|s| listed.contains(s.name())) {
    return ctx
      .t_args(
        "project_settings.services.stackAll",
        [("count", listed.len().to_string())],
      )
      .to_string();
  }
  match StartPlan::for_stack(services, stack) {
    Ok(plan) => plan
      .steps()
      .iter()
      .map(|step| step.iter().map(ServiceName::as_str).collect::<Vec<_>>().join(LIST))
      .collect::<Vec<_>>()
      .join(STEPS),
    Err(_) => listed.iter().map(ServiceName::as_str).collect::<Vec<_>>().join(LIST),
  }
}

/// The Services at once slots `stack` takes: one per service it starts with its needs, or per service it lists when
/// they cannot be resolved.
pub(super) fn slots(ctx: &Ctx, stack: &Stack, services: &[Service]) -> Arc<str> {
  let count = StartPlan::for_stack(services, stack)
    .map_or(stack.services().len(), |plan| plan.steps().iter().map(Vec::len).sum());
  if count == 1 {
    ctx.t("project_settings.services.slots.one")
  } else {
    ctx.t_args("project_settings.services.slots.other", [("count", count.to_string())])
  }
}

/// A suite's report, what it needs and its timeout: `Playwright json · needs stack cloud · 15 min`; a need that names
/// one of `stacks` says it is a stack.
pub(super) fn suite_hint(ctx: &Ctx, suite: &TestSuite, stacks: &[Entry<Stack>]) -> String {
  let needs = suite.needs().as_slice();
  let needs = if needs.is_empty() {
    ctx.t("project_settings.services.noServices")
  } else {
    let names = needs
      .iter()
      .map(|need| {
        if stacks.iter().any(|stack| *stack.name == *need.as_str()) {
          ctx
            .t_args("project_settings.services.stackNeed", [("name", need.as_str())])
            .to_string()
        } else {
          need.as_str().to_owned()
        }
      })
      .collect::<Vec<_>>();
    ctx.t_args("project_settings.services.needs", [("names", names.join(LIST))])
  };
  join([
    Some(report(ctx, suite.report().format())),
    Some(needs),
    Some(timeout(ctx, suite.timeout())),
  ])
}

/// A report format as Services and a suite's Report group name it (`cargo json`, `Playwright json`).
pub(super) fn report(ctx: &Ctx, format: ReportFormat) -> Arc<str> {
  ctx.t(match format {
    ReportFormat::CargoJson => "project_settings.services.report.cargoJson",
    ReportFormat::JunitXml => "project_settings.services.report.junitXml",
    ReportFormat::VitestJson => "project_settings.services.report.vitestJson",
    ReportFormat::PlaywrightJson => "project_settings.services.report.playwrightJson",
    ReportFormat::Tap => "project_settings.services.report.tap",
  })
}

/// A timeout in whole minutes, or in seconds when it is not one.
fn timeout(ctx: &Ctx, timeout: CommandTimeout) -> Arc<str> {
  let secs = timeout.secs();
  if secs.is_multiple_of(60) {
    ctx.t_args(
      "project_settings.services.minutes",
      [("count", (secs / 60).to_string())],
    )
  } else {
    ctx.t_args("project_settings.services.seconds", [("count", secs.to_string())])
  }
}

/// The facts given, each a whole message, joined by ` · `.
pub(super) fn join<const N: usize>(facts: [Option<Arc<str>>; N]) -> String {
  facts
    .into_iter()
    .flatten()
    .map(|fact| fact.to_string())
    .collect::<Vec<_>>()
    .join(FACTS)
}
