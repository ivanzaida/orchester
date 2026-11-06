//! One Compose service as an Orchester service: its image is the program, run as a fresh container per start
//! (`RunData::DatabaseContainer`, which publishes the image's lowest exposed port on the service's port), its
//! `command` the container's arguments, its `environment` the variables that have a value of their own, its
//! `depends_on` its needs, and its `healthcheck` the readiness check where it asks a URL ([`super::health`]). Every
//! other key is reported, not dropped.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  resource::{CommandArg, CommandProgram, VariableName},
  service::{RunData, Service, ServiceName, ServiceValue, ServiceVariables},
};
use yaml_rust2::Yaml;

use super::{super::Unmapped, health};
use crate::{
  agent_testing::import::{no_field, yaml},
  messages::Message,
};

/// The keys this mapping carries into the service, or reports apart.
const MAPPED: &[&str] = &[
  "image",
  "build",
  "command",
  "environment",
  "depends_on",
  "ports",
  "healthcheck",
];

/// The service `name` that the Compose service `key` describes, with what of it is not carried; `Err` says why it
/// is not proposed.
pub(super) fn map(
  name: ServiceName,
  (key, node): (&str, &Yaml),
  names: &BTreeMap<String, ServiceName>,
) -> Result<(Service, Vec<Unmapped>), Message> {
  let at = |field: &str| format!("services.{key}.{field}");
  let invalid = |error: String| Message::new("services.import.skipped.invalid").with("reason", error);
  let image = yaml::text(&node["image"]).unwrap_or_default();
  let program = CommandProgram::new(&image).map_err(|error| invalid(error.to_string()))?;
  let mut unmapped = Vec::new();
  if let Some(context) = super::build_context(&node["build"]) {
    unmapped.push(Unmapped {
      key: at("build"),
      reason: Message::new("services.import.unmapped.build")
        .with("image", image)
        .with("context", context),
    });
  }
  let arguments = arguments(&node["command"], &at("command"), &mut unmapped);
  let variables = variables(&node["environment"], &at("environment"), &mut unmapped);
  let needs = needs((key, &node["depends_on"]), names, &mut unmapped);
  ports(&node["ports"], &at("ports"), &mut unmapped);
  let readiness = health::readiness(&node["healthcheck"], &at("healthcheck"), &mut unmapped);
  unmapped.extend(
    yaml::entries(node)
      .into_iter()
      .filter(|(field, _)| !MAPPED.contains(&field.as_str()))
      .map(|(field, _)| no_field(at(&field))),
  );
  let service = Service::new(name, program, readiness)
    .with_arguments(arguments)
    .with_variables(ServiceVariables::new(variables).map_err(|error| invalid(error.to_string()))?)
    .with_needs(needs)
    .map_err(|error| invalid(error.to_string()))?
    .with_run_data(RunData::DatabaseContainer);
  Ok((service, unmapped))
}

/// The container's arguments: `command` as a list, or as one text of plain words. A text a shell would read (quotes,
/// escapes, variables) is reported, since no shell splits it here.
fn arguments(command: &Yaml, key: &str, unmapped: &mut Vec<Unmapped>) -> Vec<CommandArg> {
  let words: Vec<String> = match command {
    Yaml::Null | Yaml::BadValue => return Vec::new(),
    Yaml::Array(items) => {
      let Some(words) = items.iter().map(yaml::text).collect::<Option<Vec<_>>>() else {
        unmapped.push(no_field(key.to_owned()));
        return Vec::new();
      };
      words
    }
    node => {
      let text = yaml::text(node).unwrap_or_default();
      if text.contains(['"', '\'', '\\', '$', '`', '|', '&', ';', '<', '>']) {
        unmapped.push(Unmapped {
          key: key.to_owned(),
          reason: Message::new("services.import.unmapped.commandShell"),
        });
        return Vec::new();
      }
      text.split_whitespace().map(str::to_owned).collect()
    }
  };
  match words.iter().map(|word| CommandArg::new(word)).collect() {
    Ok(arguments) => arguments,
    Err(error) => {
      unmapped.push(Unmapped {
        key: key.to_owned(),
        reason: Message::new("services.import.unmapped.invalid").with("reason", error.to_string()),
      });
      Vec::new()
    }
  }
}

/// The variables with a value of their own. One whose value Compose takes from where it runs (`${TOKEN}`, or none
/// at all) is reported, so the person gives it a value or a project secret.
fn variables(environment: &Yaml, key: &str, unmapped: &mut Vec<Unmapped>) -> BTreeMap<VariableName, ServiceValue> {
  let pairs: Vec<(String, Option<String>)> = match environment {
    Yaml::Array(items) => items
      .iter()
      .filter_map(yaml::text)
      .map(|item| match item.split_once('=') {
        Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
        None => (item, None),
      })
      .collect(),
    node => yaml::entries(node)
      .into_iter()
      .map(|(name, value)| (name, yaml::text(value)))
      .collect(),
  };
  let mut variables = BTreeMap::new();
  for (name, value) in pairs {
    let at = format!("{key}.{name}");
    let Ok(variable) = VariableName::new(&name) else {
      unmapped.push(Unmapped {
        key: at,
        reason: Message::new("services.import.unmapped.variableName").with("name", name),
      });
      continue;
    };
    match value {
      Some(value) if !value.contains('$') && !value.contains('\0') => {
        variables.insert(variable, ServiceValue::Plain(value));
      }
      _ => unmapped.push(Unmapped {
        key: at,
        reason: Message::new("services.import.unmapped.environment").with("name", name),
      }),
    }
  }
  variables
}

/// The services `depends_on` names that are proposed from the same file, each once. A need of another file, or
/// one waited for until it ends rather than until it is ready, is reported.
fn needs(
  (service, depends_on): (&str, &Yaml),
  names: &BTreeMap<String, ServiceName>,
  unmapped: &mut Vec<Unmapped>,
) -> Vec<ServiceName> {
  let key = format!("services.{service}.depends_on");
  let listed: Vec<(String, Option<&Yaml>)> = match depends_on {
    Yaml::Array(items) => items.iter().filter_map(yaml::text).map(|need| (need, None)).collect(),
    node => yaml::entries(node)
      .into_iter()
      .map(|(need, settings)| (need, Some(settings)))
      .collect(),
  };
  let mut needs = Vec::new();
  let mut seen = BTreeSet::new();
  for (need, settings) in listed {
    let at = format!("{key}.{need}");
    let Some(name) = names.get(&need).filter(|_| need != service) else {
      unmapped.push(Unmapped {
        key: at,
        reason: Message::new("services.import.unmapped.need").with("need", need),
      });
      continue;
    };
    if seen.insert(name.clone()) {
      needs.push(name.clone());
    }
    for (field, value) in settings.map(yaml::entries).unwrap_or_default() {
      let waits_until_ready = field == "condition"
        && yaml::text(value).is_some_and(|condition| condition != "service_completed_successfully");
      if !waits_until_ready {
        unmapped.push(no_field(format!("{at}.{field}")));
      }
    }
  }
  needs
}

/// The container ports `ports` publishes. Orchester publishes one, the image's lowest exposed port, on the port it
/// gives the service; more than one is reported.
fn ports(ports: &Yaml, key: &str, unmapped: &mut Vec<Unmapped>) {
  let targets: BTreeSet<String> = ports
    .as_vec()
    .map(|items| items.iter().filter_map(target).collect())
    .unwrap_or_default();
  if targets.len() > 1 {
    let listed: Vec<String> = targets.into_iter().collect();
    unmapped.push(Unmapped {
      key: key.to_owned(),
      reason: Message::new("services.import.unmapped.ports").with("ports", listed.join(", ")),
    });
  }
}

/// The container port of one `ports` entry: `[HOST:]CONTAINER[/PROTOCOL]` as text, or `target` of the long form.
fn target(entry: &Yaml) -> Option<String> {
  if let Some(target) = yaml::text(&entry["target"]) {
    return Some(target);
  }
  let text = yaml::text(entry)?;
  let port = text.rsplit(':').next()?;
  Some(port.split('/').next().unwrap_or(port).to_owned())
}
