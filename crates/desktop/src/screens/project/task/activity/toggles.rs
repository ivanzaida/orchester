//! Rows a person opened or closed in the conversation timeline: a step, a tool group, an entry
//! with output. A person's choice overrides the start state and is kept while the timeline grows.

use std::{collections::HashMap, sync::Arc};

use lurq::core::Signal;

/// Rows a person opened or closed, by row key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::project::task) struct OpenRows(HashMap<Arc<str>, bool>);

impl lurq::app::component::DevtoolsInspectable for OpenRows {}

/// Open or closed per row key, where a person changed the start state.
#[derive(Clone)]
pub(in crate::screens::project::task) struct Toggles(pub(in crate::screens::project::task) Signal<OpenRows>);

impl Toggles {
  pub(in crate::screens::project::task) fn is_open(&self, key: &str, default: bool) -> bool {
    self.0.get().0.get(key).copied().unwrap_or(default)
  }

  pub(in crate::screens::project::task) fn flip(&self, key: &Arc<str>, default: bool) {
    let key = key.clone();
    self.0.update(move |rows| {
      let open = rows.0.get(&key).copied().unwrap_or(default);
      rows.0.insert(key, !open);
    });
  }
}
