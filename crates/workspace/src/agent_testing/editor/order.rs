//! What the service editor works out from the project's services as they are now (each its draft if it has one,
//! otherwise its latest version): the order a start brings the service and what it needs up in (`Start order`
//! `Dybbp`, `Sow5a`), and whether its placeholders name services it needs ([`check_placeholders`]), said where each
//! is written.

use std::sync::Arc;

use orchester_core::service::{
  NeedsError, PlaceholderError, PlaceholderPlace, ReadyCheck, Service, StartPlan, check_placeholders,
};

use crate::{
  agent_testing::data::Entry,
  messages::{Message, needs_error},
};

/// A service in the order a start brings it up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
  pub name: Arc<str>,
  /// When it counts as ready.
  pub ready: Ready,
  /// The service being edited, which starts last.
  pub this: bool,
}

/// How a started service tells it is ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ready {
  PortOpen,
  /// An HTTP answer on the path.
  Http(Arc<str>),
  LogLine,
}

impl From<&ReadyCheck> for Ready {
  fn from(check: &ReadyCheck) -> Self {
    match check {
      ReadyCheck::PortOpen => Self::PortOpen,
      ReadyCheck::Http(path) => Self::Http(path.as_str().into()),
      ReadyCheck::LogLine(_) => Self::LogLine,
    }
  }
}

/// The project's services with `service` in place of the one of its name.
fn with_edited(service: &Service, services: &[Entry<Service>]) -> Vec<Service> {
  services
    .iter()
    .map(|entry| &entry.body)
    .filter(|other| other.name() != service.name())
    .chain([service])
    .cloned()
    .collect()
}

/// The order a start of `service` brings it and what it needs up in, from the project's `services`: step by step,
/// each after what it needs; services of one step start together and keep the order the needs list them in.
pub fn start_order(service: &Service, services: &[Entry<Service>]) -> Result<Vec<Started>, Message> {
  let all = with_edited(service, services);
  let plan = StartPlan::for_service(&all, service.name()).map_err(|error| order_error(&error))?;
  Ok(
    plan
      .steps()
      .iter()
      .flatten()
      .filter_map(|name| all.iter().find(|other| other.name() == name))
      .map(|started| Started {
        name: started.name().as_str().into(),
        ready: started.readiness().check().into(),
        this: started.name() == service.name(),
      })
      .collect(),
  )
}

/// Why the needs do not resolve: a need names a service the project no longer has, or they form a cycle.
fn order_error(error: &NeedsError) -> Message {
  match error {
    NeedsError::Missing {
      name,
      needed_by: Some(service),
    } => Message::new("services.editor.errors.needMissing")
      .with("service", service.to_string())
      .with("need", name.to_string()),
    other => needs_error(other),
  }
}

/// The field a placeholder problem is said under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
  Arguments,
  Variables,
  /// The path of the HTTP readiness check.
  Readiness,
  /// A seed request, counted from 0, or the seed command's arguments.
  Seed(Option<usize>),
  /// Where `ui_open` starts.
  UiStart,
  /// What no field holds: a value only a start fills, which a check never finds missing.
  Elsewhere,
}

/// The first placeholder of `service` that cannot be used, with the field it is written in; publishing waits until
/// none is left.
pub fn placeholder_problem(service: &Service, services: &[Entry<Service>]) -> Option<(Field, Message)> {
  let all = with_edited(service, services);
  let error = check_placeholders(service, &all).err()?;
  Some(placeholder_message(&error))
}

/// What is wrong with a placeholder the page can point at, in the order of [`KEYS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wrong {
  Unclosed,
  Malformed,
  UnknownField,
  UnknownService,
  NotNeeded,
  UnknownCapture,
  NoRunFolder,
}

/// Where a placeholder is written, as its message names it, in the order of [`KEYS`]' columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
  Argument,
  Variable,
  Readiness,
  SeedRequest,
  SeedArgument,
  UiStart,
}

/// Each wrong's message key, one row per [`Wrong`] and one column per [`Spot`].
const KEYS: [[&str; 6]; 7] = [
  [
    "services.editor.placeholder.argument.unclosed",
    "services.editor.placeholder.variable.unclosed",
    "services.editor.placeholder.readiness.unclosed",
    "services.editor.placeholder.seed.unclosed",
    "services.editor.placeholder.seedArgument.unclosed",
    "services.editor.placeholder.uiStart.unclosed",
  ],
  [
    "services.editor.placeholder.argument.malformed",
    "services.editor.placeholder.variable.malformed",
    "services.editor.placeholder.readiness.malformed",
    "services.editor.placeholder.seed.malformed",
    "services.editor.placeholder.seedArgument.malformed",
    "services.editor.placeholder.uiStart.malformed",
  ],
  [
    "services.editor.placeholder.argument.unknownField",
    "services.editor.placeholder.variable.unknownField",
    "services.editor.placeholder.readiness.unknownField",
    "services.editor.placeholder.seed.unknownField",
    "services.editor.placeholder.seedArgument.unknownField",
    "services.editor.placeholder.uiStart.unknownField",
  ],
  [
    "services.editor.placeholder.argument.unknownService",
    "services.editor.placeholder.variable.unknownService",
    "services.editor.placeholder.readiness.unknownService",
    "services.editor.placeholder.seed.unknownService",
    "services.editor.placeholder.seedArgument.unknownService",
    "services.editor.placeholder.uiStart.unknownService",
  ],
  [
    "services.editor.placeholder.argument.notNeeded",
    "services.editor.placeholder.variable.notNeeded",
    "services.editor.placeholder.readiness.notNeeded",
    "services.editor.placeholder.seed.notNeeded",
    "services.editor.placeholder.seedArgument.notNeeded",
    "services.editor.placeholder.uiStart.notNeeded",
  ],
  [
    "services.editor.placeholder.argument.unknownCapture",
    "services.editor.placeholder.variable.unknownCapture",
    "services.editor.placeholder.readiness.unknownCapture",
    "services.editor.placeholder.seed.unknownCapture",
    "services.editor.placeholder.seedArgument.unknownCapture",
    "services.editor.placeholder.uiStart.unknownCapture",
  ],
  [
    "services.editor.placeholder.argument.noRunFolder",
    "services.editor.placeholder.variable.noRunFolder",
    "services.editor.placeholder.readiness.noRunFolder",
    "services.editor.placeholder.seed.noRunFolder",
    "services.editor.placeholder.seedArgument.noRunFolder",
    "services.editor.placeholder.uiStart.noRunFolder",
  ],
];

const ELSEWHERE: &str = "services.editor.placeholder.elsewhere";

/// The field `place` is in, its message's spot and the value naming where in it.
fn spot(place: &PlaceholderPlace) -> (Field, Spot, Option<(&'static str, String)>) {
  match place {
    PlaceholderPlace::Argument(index) => (
      Field::Arguments,
      Spot::Argument,
      Some(("line", (index + 1).to_string())),
    ),
    PlaceholderPlace::Variable(name) => (
      Field::Variables,
      Spot::Variable,
      Some(("name", name.as_str().to_owned())),
    ),
    PlaceholderPlace::ReadinessPath => (Field::Readiness, Spot::Readiness, None),
    PlaceholderPlace::SeedUrl { request }
    | PlaceholderPlace::SeedBody { request }
    | PlaceholderPlace::SeedHeader { request, .. } => (
      Field::Seed(Some(*request)),
      Spot::SeedRequest,
      Some(("request", (request + 1).to_string())),
    ),
    PlaceholderPlace::SeedArgument(index) => (
      Field::Seed(None),
      Spot::SeedArgument,
      Some(("line", (index + 1).to_string())),
    ),
    PlaceholderPlace::UiStart => (Field::UiStart, Spot::UiStart, None),
  }
}

fn placeholder_message(error: &PlaceholderError) -> (Field, Message) {
  let (place, wrong, value) = match error {
    PlaceholderError::Unclosed { place } => (place, Wrong::Unclosed, None),
    PlaceholderError::Malformed { place } => (place, Wrong::Malformed, None),
    PlaceholderError::UnknownField { place, placeholder } => (place, Wrong::UnknownField, Some(placeholder.clone())),
    PlaceholderError::UnknownService { place, service } => (place, Wrong::UnknownService, Some(service.clone())),
    PlaceholderError::NotNeeded { place, service } => (place, Wrong::NotNeeded, Some(service.to_string())),
    PlaceholderError::UnknownCapture { place, capture } => (place, Wrong::UnknownCapture, Some(capture.clone())),
    PlaceholderError::NoRunFolder { place } => (place, Wrong::NoRunFolder, None),
    // Only a start fills placeholders; a check never finds a value missing.
    PlaceholderError::NoValue { .. } => return (Field::Elsewhere, Message::new(ELSEWHERE)),
  };
  let (field, spot, at) = spot(place);
  let message = Message::new(KEYS[wrong as usize][spot as usize]);
  let message = match at {
    Some((name, at)) => message.with(name, at),
    None => message,
  };
  (
    field,
    value.map_or(message.clone(), |value| message.with("value", value)),
  )
}
