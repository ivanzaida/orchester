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
  /// A part the page does not edit yet: the readiness path, the seed or where `ui_open` starts.
  Elsewhere,
}

/// The first placeholder of `service` that cannot be used, with the field it is written in; publishing waits until
/// none is left.
pub fn placeholder_problem(service: &Service, services: &[Entry<Service>]) -> Option<(Field, Message)> {
  let all = with_edited(service, services);
  let error = check_placeholders(service, &all).err()?;
  Some(placeholder_message(&error))
}

/// What is wrong with a placeholder the page can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wrong {
  Unclosed,
  Malformed,
  UnknownField,
  UnknownService,
  NotNeeded,
  UnknownCapture,
}

impl Wrong {
  /// Its message's key in an argument and in a variable.
  const fn keys(self) -> (&'static str, &'static str) {
    match self {
      Self::Unclosed => (
        "services.editor.placeholder.argument.unclosed",
        "services.editor.placeholder.variable.unclosed",
      ),
      Self::Malformed => (
        "services.editor.placeholder.argument.malformed",
        "services.editor.placeholder.variable.malformed",
      ),
      Self::UnknownField => (
        "services.editor.placeholder.argument.unknownField",
        "services.editor.placeholder.variable.unknownField",
      ),
      Self::UnknownService => (
        "services.editor.placeholder.argument.unknownService",
        "services.editor.placeholder.variable.unknownService",
      ),
      Self::NotNeeded => (
        "services.editor.placeholder.argument.notNeeded",
        "services.editor.placeholder.variable.notNeeded",
      ),
      Self::UnknownCapture => (
        "services.editor.placeholder.argument.unknownCapture",
        "services.editor.placeholder.variable.unknownCapture",
      ),
    }
  }
}

const ELSEWHERE: &str = "services.editor.placeholder.elsewhere";

fn placeholder_message(error: &PlaceholderError) -> (Field, Message) {
  let (place, wrong, value) = match error {
    PlaceholderError::Unclosed { place } => (place, Some(Wrong::Unclosed), None),
    PlaceholderError::Malformed { place } => (place, Some(Wrong::Malformed), None),
    PlaceholderError::UnknownField { place, placeholder } => {
      (place, Some(Wrong::UnknownField), Some(placeholder.clone()))
    }
    PlaceholderError::UnknownService { place, service } => (place, Some(Wrong::UnknownService), Some(service.clone())),
    PlaceholderError::NotNeeded { place, service } => (place, Some(Wrong::NotNeeded), Some(service.to_string())),
    PlaceholderError::UnknownCapture { place, capture } => (place, Some(Wrong::UnknownCapture), Some(capture.clone())),
    // Only a start fills placeholders; a check never finds a value missing.
    PlaceholderError::NoValue { place, .. } => (place, None, None),
  };
  let Some(wrong) = wrong else {
    return (Field::Elsewhere, Message::new(ELSEWHERE));
  };
  let (argument, variable) = wrong.keys();
  let (field, message) = match place {
    PlaceholderPlace::Argument(index) => (
      Field::Arguments,
      Message::new(argument).with("line", (index + 1).to_string()),
    ),
    PlaceholderPlace::Variable(name) => (
      Field::Variables,
      Message::new(variable).with("name", name.as_str().to_owned()),
    ),
    _ => return (Field::Elsewhere, Message::new(ELSEWHERE)),
  };
  (
    field,
    value.map_or(message.clone(), |value| message.with("value", value)),
  )
}
