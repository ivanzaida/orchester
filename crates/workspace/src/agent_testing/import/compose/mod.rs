//! A `docker compose` file as proposals: each service with an image as a service whose program is that image, run as
//! a fresh container per start ([`service`]), and the file's services together as a stack named after the Compose
//! project (its `name`, else its folder). A service Compose builds from source has no image to start, so it is passed
//! over with why.

mod health;
mod service;

use std::collections::BTreeMap;

use orchester_core::service::{ServiceName, Stack};
use yaml_rust2::Yaml;

use super::{Draft, Reading, Source, checkout::Checkout, folder_of, no_field, yaml};
use crate::messages::Message;

/// The top-level keys the stack carries.
const STACK_KEYS: &[&str] = &["services", "name"];

/// Proposes the services and the stack of the compose file `file`.
pub(super) fn read(checkout: &Checkout, file: &str, reading: &mut Reading) {
  let document = match checkout.read(file).and_then(|text| yaml::load(&text)) {
    Ok(Some(document)) => document,
    Ok(None) => return reading.unreadable(file, Message::new("services.import.unreadable.noServices")),
    Err(reason) => return reading.unreadable(file, reason),
  };
  let top = yaml::entries(&document);
  let Some((_, services)) = top.iter().find(|(key, _)| key == "services") else {
    return reading.unreadable(file, Message::new("services.import.unreadable.noServices"));
  };
  let names = name_services(file, services, reading);
  let mut started = Vec::new();
  for (key, node) in yaml::entries(services) {
    let Some(name) = names.get(&key) else { continue };
    let source = Source {
      file: file.to_owned(),
      key: format!("services.{key}"),
    };
    match service::map(name.clone(), (&key, node), &names) {
      Ok((service, unmapped)) => {
        started.push(name.clone());
        reading.propose(Draft::Service(Box::new(service)), source, unmapped);
      }
      Err(reason) => reading.skip(source, reason),
    }
  }
  propose_stack(file, &top, started, reading);
}

/// The names the services with an image take, by their keys in the file; the others are passed over with why.
fn name_services(file: &str, services: &Yaml, reading: &mut Reading) -> BTreeMap<String, ServiceName> {
  let mut names = BTreeMap::new();
  for (key, node) in yaml::entries(services) {
    let source = Source {
      file: file.to_owned(),
      key: format!("services.{key}"),
    };
    if node.as_hash().is_none() {
      reading.skip(source, Message::new("services.import.skipped.notAMapping"));
      continue;
    }
    if yaml::text(&node["image"]).is_none() {
      let build = build_context(&node["build"]);
      let reason = match build {
        Some(context) => Message::new("services.import.skipped.builds").with("context", context),
        None => Message::new("services.import.skipped.noImage"),
      };
      reading.skip(source, reason);
      continue;
    }
    let name = reading.names.service(&key, "service");
    names.insert(key, name);
  }
  names
}

/// The folder Compose builds a service's image from: `build` as text, or its `context`.
fn build_context(build: &Yaml) -> Option<String> {
  yaml::text(build).or_else(|| yaml::text(&build["context"]))
}

/// The stack of the services proposed from `file`; none when it proposed none.
fn propose_stack(file: &str, top: &[(String, &Yaml)], services: Vec<ServiceName>, reading: &mut Reading) {
  if services.is_empty() {
    return;
  }
  let project = top
    .iter()
    .find(|(key, _)| key == "name")
    .and_then(|(_, name)| yaml::text(name));
  let folder = folder_of(file);
  let wanted = project.unwrap_or_else(|| folder.rsplit('/').next().unwrap_or(folder).to_owned());
  let name = reading.names.stack(&wanted, "compose");
  let unmapped = top
    .iter()
    .filter(|(key, _)| !STACK_KEYS.contains(&key.as_str()))
    .map(|(key, _)| no_field(key.clone()))
    .collect();
  let stack = Stack::new(name, services).expect("the services of one file have names of their own");
  let source = Source {
    file: file.to_owned(),
    key: "services".to_owned(),
  };
  reading.propose(Draft::Stack(stack), source, unmapped);
}
