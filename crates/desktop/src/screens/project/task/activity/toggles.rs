//! Rows a person opened or closed in the conversation: a step, a tool chip, an entry with output. A
//! person's choice overrides the start state and is kept while the timeline grows.

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

  /// Opens every row of `keys`, as *Show them* does for the steps of a fold.
  pub(in crate::screens::project::task) fn open_all(&self, keys: &[Arc<str>]) {
    let keys = keys.to_vec();
    self.0.update(move |rows| {
      for key in keys {
        rows.0.insert(key, true);
      }
    });
  }

  pub(in crate::screens::project::task) fn flip(&self, key: &Arc<str>, default: bool) {
    let key = key.clone();
    self.0.update(move |rows| {
      let open = rows.0.get(&key).copied().unwrap_or(default);
      rows.0.insert(key, !open);
    });
  }
}
