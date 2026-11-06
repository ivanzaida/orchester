//! The names proposals are given: each read as a slug from what the checkout calls it, and kept apart from the names
//! earlier proposals of its kind took, so two files that both call a service `postgres` give `postgres` and
//! `postgres-2`.

use std::collections::BTreeSet;

use orchester_core::{
  resource::Slug,
  service::{ServiceName, StackName},
};

/// The names the proposals took so far, per kind.
#[derive(Default)]
pub(super) struct Names {
  services: BTreeSet<String>,
  stacks: BTreeSet<String>,
}

impl Names {
  /// A service name of its own read from `wanted`, else from `fallback`.
  pub(super) fn service(&mut self, wanted: &str, fallback: &str) -> ServiceName {
    let name = unique(&mut self.services, wanted, fallback);
    ServiceName::new(&name).expect("a unique name is a slug")
  }

  /// A stack name of its own read from `wanted`, else from `fallback`.
  pub(super) fn stack(&mut self, wanted: &str, fallback: &str) -> StackName {
    let name = unique(&mut self.stacks, wanted, fallback);
    StackName::new(&name).expect("a unique name is a slug")
  }
}

/// `wanted` as a slug (`fallback`'s when it has no ASCII letter or digit), with `-2`, `-3`, … after it while `taken`
/// holds it.
fn unique(taken: &mut BTreeSet<String>, wanted: &str, fallback: &str) -> String {
  let base = Slug::from_name(wanted)
    .or_else(|| Slug::from_name(fallback))
    .map_or_else(|| "service".to_owned(), String::from);
  if taken.insert(base.clone()) {
    return base;
  }
  (2..)
    .map(|number| {
      let suffix = format!("-{number}");
      let mut head = base.clone();
      head.truncate(Slug::MAX_CHARS - suffix.len());
      format!("{}{suffix}", head.trim_end_matches('-'))
    })
    .find(|name| taken.insert(name.clone()))
    .expect("an endless count finds a free name")
}
