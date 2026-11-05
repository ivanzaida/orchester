//! Which of the project's published suites a Test step runs, and in which order: a suite runs when it is marked always
//! or when a service it needs — named by it, in a stack it names, or needed by one of those — owns a path the run
//! changed; the rest are left out. They run by layer, lint first, then unit, then e2e, each layer's suites by slug.

use std::collections::BTreeSet;

use crate::{
  resource::{Slug, TestLayer, TestSuite},
  service::{Service, ServiceName, Stack},
};

/// Why a suite runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
  /// It is marked to run always.
  Always,
  /// The service `service`, which it needs, owns `path`, which the run changed.
  Owns { service: ServiceName, path: String },
}

/// A suite the step runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedSuite {
  pub slug: Slug,
  pub suite: TestSuite,
  pub chosen: Chosen,
}

/// The suites of one layer, by slug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedLayer {
  pub layer: TestLayer,
  pub suites: Vec<PlannedSuite>,
}

/// What a Test step runs, layer by layer, and the suites it leaves out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestPlan {
  layers: Vec<PlannedLayer>,
  left_out: Vec<Slug>,
}

impl TestPlan {
  /// The plan for a run that changed `changed` (repository paths with `/`, as git lists them), over the project's
  /// published `suites`, `services` and `stacks`.
  pub fn new(suites: Vec<(Slug, TestSuite)>, services: &[Service], stacks: &[Stack], changed: &[String]) -> Self {
    let mut layers: Vec<PlannedLayer> = TestLayer::ALL
      .into_iter()
      .map(|layer| PlannedLayer {
        layer,
        suites: Vec::new(),
      })
      .collect();
    let mut left_out = Vec::new();
    for (slug, suite) in suites {
      match chosen(&suite, services, stacks, changed) {
        Some(chosen) => {
          let layer = &mut layers[TestLayer::ALL
            .iter()
            .position(|layer| *layer == suite.layer())
            .expect("every layer is in TestLayer::ALL")];
          layer.suites.push(PlannedSuite { slug, suite, chosen });
        }
        None => left_out.push(slug),
      }
    }
    for layer in &mut layers {
      layer.suites.sort_by(|left, right| left.slug.cmp(&right.slug));
    }
    layers.retain(|layer| !layer.suites.is_empty());
    left_out.sort();
    Self { layers, left_out }
  }

  /// The layers that have a suite to run, lint first.
  pub fn layers(&self) -> &[PlannedLayer] {
    &self.layers
  }

  pub fn into_layers(self) -> Vec<PlannedLayer> {
    self.layers
  }

  /// The suites that do not run: none of the services they need owns a changed path, and they are not marked always.
  pub fn left_out(&self) -> &[Slug] {
    &self.left_out
  }
}

/// Why `suite` runs, or `None` when it does not.
fn chosen(suite: &TestSuite, services: &[Service], stacks: &[Stack], changed: &[String]) -> Option<Chosen> {
  if suite.always() {
    return Some(Chosen::Always);
  }
  needed_services(suite, services, stacks).into_iter().find_map(|name| {
    let service = services.iter().find(|service| service.name() == &name)?;
    let path = changed
      .iter()
      .find(|path| service.owned_paths().iter().any(|owned| owned.matches(path)))?;
    Some(Chosen::Owns {
      service: name,
      path: path.clone(),
    })
  })
}

/// The services `suite` starts: each need, a stack's services or a service, as `run_tests` starts them, and what
/// those need in turn; by name.
fn needed_services(suite: &TestSuite, services: &[Service], stacks: &[Stack]) -> BTreeSet<ServiceName> {
  let mut pending: Vec<ServiceName> = Vec::new();
  for need in suite.needs().as_slice() {
    match stacks.iter().find(|stack| stack.name().as_str() == need.as_str()) {
      Some(stack) => pending.extend(stack.services().iter().cloned()),
      None => pending.extend(ServiceName::new(need.as_str())),
    }
  }
  let mut found = BTreeSet::new();
  while let Some(name) = pending.pop() {
    if found.insert(name.clone())
      && let Some(service) = services.iter().find(|service| service.name() == &name)
    {
      pending.extend(service.needs().iter().cloned());
    }
  }
  found
}
