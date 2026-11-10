//! What a pass of the compact picker shows, worked out from its props and what Search holds.

use std::sync::Arc;

use super::{
  Panel, RunsOnPicker,
  actions::{OwnedShape, Targets},
  model::{Model, filter_models},
};

/// What a pass shows.
pub(super) struct View<'a> {
  /// The place on the rail of the connection shown.
  pub(super) shown: Option<usize>,
  /// The models of the connection shown that Search finds.
  pub(super) found: Vec<&'a Model>,
  /// The model Search holds that the connection shown does not list and takes anyway (`typed_model`).
  pub(super) own: Option<Arc<str>>,
  /// The chosen model, when it is on the connection shown, and the efforts it offers.
  pub(super) model: Option<&'a str>,
  pub(super) efforts: Option<&'a [Arc<str>]>,
  /// The connection shown lists no efforts: its effort is typed.
  pub(super) typed_effort: bool,
  pub(super) searched: bool,
  pub(super) targets: Targets,
}

impl Panel {
  /// The connection shown, the models Search finds on it and the efforts the chosen model offers, with what the keys
  /// move over and hand back.
  pub(super) fn view<'a>(&self, props: &'a RunsOnPicker) -> View<'a> {
    let shown = self.shown_connection(props);
    let connections = &props.connections;
    let shown_connection = shown.map(|at| &connections[at]);
    let query = self.query.get();
    let found = shown_connection.map_or_else(Vec::new, |shown| filter_models(&shown.models, &query));
    let named = query.trim();
    let own = shown_connection
      .filter(|shown| shown.typed_model && !named.is_empty() && !shown.models.iter().any(|model| &*model.id == named))
      .map(|_| Arc::<str>::from(named));
    let on_chosen = shown_connection.is_some_and(|shown| shown.id == props.chosen.connection);
    let model = props.chosen.model.as_deref().filter(|_| on_chosen);
    let typed_effort = shown_connection.is_some_and(|shown| shown.typed_effort);
    let efforts = shown_connection
      .filter(|_| on_chosen)
      .and_then(|shown| shown.models.iter().find(|candidate| Some(&*candidate.id) == model))
      .map(|offered| offered.efforts.as_slice());
    // *Default* first, then what the model offers; none at all while there is no model chosen, or where it is typed.
    let levels: Vec<Option<Arc<str>>> = efforts.filter(|_| !typed_effort).map_or_else(Vec::new, |offered| {
      std::iter::once(None).chain(offered.iter().cloned().map(Some)).collect()
    });
    let searched = !named.is_empty();
    let typed_here = typed_effort && model.is_some();
    let shape = OwnedShape {
      rail: connections
        .iter()
        .map(|connection| connection.unavailable.is_none())
        .collect(),
      viewing: shown,
      list: found.len() + usize::from(own.is_some()),
      chosen: found.iter().position(|found| Some(&*found.id) == model),
      searched,
      efforts: if typed_here { 1 } else { levels.len() },
      effort: if typed_here {
        Some(0)
      } else {
        levels
          .iter()
          .position(|level| level.as_deref() == props.chosen.effort.as_deref())
      },
    };
    let targets = Targets {
      shape,
      models: found.iter().map(|found| found.id.clone()).chain(own.clone()).collect(),
      efforts: levels,
      typed_effort: typed_here,
    };
    View {
      shown,
      found,
      own,
      model,
      efforts,
      typed_effort,
      searched,
      targets,
    }
  }
}
