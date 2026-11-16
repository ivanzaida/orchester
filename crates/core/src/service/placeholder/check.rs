use std::collections::BTreeSet;

use super::{Placeholder, PlaceholderError, PlaceholderPlace};
use crate::{
  resource::FieldName,
  service::{ReadyCheck, RunData, Seed, Service, ServiceName, ServiceValue},
};

/// Checks every placeholder of `service` against the project's `services`: each names a service that is defined
/// and that `service` needs itself, each `{{seed.<name>}}` a value an earlier seed request captures, and `{{data}}`
/// is used only by a service whose per-run data is a run folder. The place `ui_open` starts comes after the whole
/// seed, so it may use every capture.
pub fn check_placeholders(service: &Service, services: &[Service]) -> Result<(), PlaceholderError> {
  let mut scope = Scope {
    defined: services.iter().map(Service::name).collect(),
    needs: service.needs(),
    captured: BTreeSet::new(),
    run_folder: matches!(service.run_data(), RunData::RunFolder(_)),
  };
  for (index, argument) in service.arguments().iter().enumerate() {
    scope.check(argument.as_str(), PlaceholderPlace::Argument(index))?;
  }
  for (name, value) in service.variables().iter() {
    if let ServiceValue::Plain(text) = value {
      scope.check(text, PlaceholderPlace::Variable(name.clone()))?;
    }
  }
  if let ReadyCheck::Http(path) = service.readiness().check() {
    scope.check(path.as_str(), PlaceholderPlace::ReadinessPath)?;
  }
  scope.check_seed(service.seed())?;
  if let Some(start) = service.ui_start() {
    scope.check(start.as_str(), PlaceholderPlace::UiStart)?;
  }
  Ok(())
}

/// What the placeholders of one place may use.
struct Scope<'a> {
  defined: BTreeSet<&'a ServiceName>,
  needs: &'a [ServiceName],
  captured: BTreeSet<&'a FieldName>,
  /// Whether the service's per-run data is a run folder, which `{{data}}` names.
  run_folder: bool,
}

impl<'a> Scope<'a> {
  /// Each request may use what the requests before it capture.
  fn check_seed(&mut self, seed: &'a Seed) -> Result<(), PlaceholderError> {
    match seed {
      Seed::None => Ok(()),
      Seed::Command { arguments, .. } => {
        for (index, argument) in arguments.iter().enumerate() {
          self.check(argument.as_str(), PlaceholderPlace::SeedArgument(index))?;
        }
        Ok(())
      }
      Seed::Requests(requests) => {
        for (request, seed_request) in requests.as_slice().iter().enumerate() {
          self.check(seed_request.url(), PlaceholderPlace::SeedUrl { request })?;
          if let Some(body) = seed_request.body() {
            self.check(body, PlaceholderPlace::SeedBody { request })?;
          }
          for header in seed_request.headers() {
            if let ServiceValue::Plain(text) = header.value() {
              let header = header.name().to_owned();
              self.check(text, PlaceholderPlace::SeedHeader { request, header })?;
            }
          }
          self
            .captured
            .extend(seed_request.captures().iter().map(|capture| capture.name()));
        }
        Ok(())
      }
    }
  }

  fn check(&self, text: &str, place: PlaceholderPlace) -> Result<(), PlaceholderError> {
    for placeholder in Placeholder::find_all(text, &place)? {
      match placeholder {
        Placeholder::ServiceUrl(service) | Placeholder::ServicePort(service) => {
          if !self.defined.contains(&service) {
            let service = service.as_str().to_owned();
            return Err(PlaceholderError::UnknownService { place, service });
          }
          if !self.needs.contains(&service) {
            return Err(PlaceholderError::NotNeeded { place, service });
          }
        }
        Placeholder::Seed(capture) if !self.captured.contains(&capture) => {
          let capture = capture.as_str().to_owned();
          return Err(PlaceholderError::UnknownCapture { place, capture });
        }
        Placeholder::Data if !self.run_folder => return Err(PlaceholderError::NoRunFolder { place }),
        Placeholder::Port | Placeholder::Url | Placeholder::Run | Placeholder::Data | Placeholder::Seed(_) => {}
      }
    }
    Ok(())
  }
}
