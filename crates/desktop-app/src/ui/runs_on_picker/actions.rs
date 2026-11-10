//! What choosing in the compact picker does, by the pointer or by the keys: the picker's signals, the handler of its
//! keys and the hand-back of each choice. The keys' own rules are in [`keys`](super::keys).

use std::sync::Arc;

use lurq::{app::events::KeyboardEvent, core::Signal, layout::layout_kind::ScrollState};

use super::{
  RunsOnPicker,
  foot::TypedEffort,
  keys::{Cursor, Key, Outcome, Place, Shape},
  model::Pick,
  models, rail,
  rail::Choose,
};

/// [`Shape`] owned, for the key handler to keep.
#[derive(Clone)]
pub(super) struct OwnedShape {
  pub(super) rail: Vec<bool>,
  pub(super) viewing: Option<usize>,
  pub(super) list: usize,
  pub(super) chosen: Option<usize>,
  pub(super) searched: bool,
  pub(super) efforts: usize,
  pub(super) effort: Option<usize>,
}

impl OwnedShape {
  pub(super) fn borrowed(&self) -> Shape<'_> {
    Shape {
      rail: &self.rail,
      viewing: self.viewing,
      list: self.list,
      chosen: self.chosen,
      searched: self.searched,
      efforts: self.efforts,
      effort: self.effort,
    }
  }
}

/// What the keys move over and hand back: the shape, the model at each place in the list and the effort at each
/// segment of the effort row (`None` is *Default*), or, where the effort is typed, that the one place of the row is the
/// field.
#[derive(Clone)]
pub(super) struct Targets {
  pub(super) shape: OwnedShape,
  pub(super) models: Vec<Arc<str>>,
  pub(super) efforts: Vec<Option<Arc<str>>>,
  pub(super) typed_effort: bool,
}

/// The field that takes the focus on the next pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
  Stay,
  Search,
  Effort,
}

impl lurq::app::component::DevtoolsInspectable for Focus {}

/// The picker's signals, which choosing and the keys change.
#[derive(Clone)]
pub(super) struct Handles {
  pub(super) viewing: Signal<String>,
  pub(super) query: Signal<String>,
  pub(super) cursor: Signal<Cursor>,
  pub(super) refused: Signal<Option<usize>>,
  pub(super) focus: Signal<Focus>,
  pub(super) own_effort: TypedEffort,
  pub(super) scroll: ScrollState,
  pub(super) rail_scroll: ScrollState,
}

impl Handles {
  /// What was typed in Search: the keys go to the models it finds, from the first.
  pub(super) fn typed(&self) {
    self.cursor.update(Cursor::typed);
    self.refused.set(None);
  }
}

/// What choosing in the picker does.
#[derive(Clone)]
pub(super) struct Actions {
  pub(super) panel: Handles,
  pub(super) props: RunsOnPicker,
  /// The place on the rail of the connection shown.
  pub(super) shown: Option<usize>,
}

impl Actions {
  pub(super) fn connection_chooser(&self) -> Choose {
    let actions = self.clone();
    Arc::new(move |at| actions.connection(at))
  }

  /// What choosing the model at a place in the list does, the list holding the models `ids`.
  pub(super) fn model_chooser(&self, ids: Vec<Arc<str>>) -> Choose {
    let actions = self.clone();
    Arc::new(move |at| {
      if let Some(model) = ids.get(at) {
        actions.model(model);
      }
    })
  }

  pub(super) fn effort_chooser(&self) -> Arc<dyn Fn(Option<Arc<str>>) + Send + Sync> {
    let actions = self.clone();
    Arc::new(move |effort| actions.effort(effort))
  }

  /// Shows the connection at `at` on the rail and hands it back unless it is what is chosen already.
  fn connection(&self, at: usize) {
    let Some(connection) = self.props.connections.get(at) else {
      return;
    };
    let panel = &self.panel;
    panel.refused.set(None);
    panel.cursor.set(Cursor::START);
    if self.shown != Some(at) {
      panel.viewing.set(connection.id.to_string());
      panel.query.set(String::new());
      panel.scroll.scroll_to_top_pending();
    }
    if connection.id != self.props.chosen.connection {
      (self.props.on_pick)(Pick::Connection(connection.id.clone()));
    }
  }

  /// Hands back `model` of the connection shown unless it is what is chosen already. A choice made takes the reason a
  /// refused connection gave away from the line under the rail.
  fn model(&self, model: &Arc<str>) {
    self.panel.refused.set(None);
    let Some(connection) = self.shown.and_then(|at| self.props.connections.get(at)) else {
      return;
    };
    let chosen = &self.props.chosen;
    if connection.id == chosen.connection && chosen.model.as_ref() == Some(model) {
      return;
    }
    (self.props.on_pick)(Pick::Model {
      connection: connection.id.clone(),
      model: model.clone(),
    });
  }

  /// Hands back what the typed effort field holds, trimmed; nothing typed is *Default*.
  pub(super) fn typed_effort(&self) {
    let typed = self.panel.own_effort.value.get_untracked();
    let typed = typed.trim();
    self.effort((!typed.is_empty()).then(|| Arc::from(typed)));
  }

  /// Hands back `effort` unless it is what is chosen already; like a model, it takes a refusal's reason away.
  fn effort(&self, effort: Option<Arc<str>>) {
    self.panel.refused.set(None);
    if self.props.chosen.effort != effort {
      (self.props.on_pick)(Pick::Effort(effort));
    }
  }

  /// The handler of the picker's keys over `targets`.
  pub(super) fn keys(&self, targets: Targets) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
    let actions = self.clone();
    move |event: KeyboardEvent| {
      if event.default_prevented() || event.composing {
        return;
      }
      let key = match event.key.as_str() {
        "ArrowLeft" => Some(Key::Left),
        "ArrowRight" => Some(Key::Right),
        "ArrowUp" => Some(Key::Up),
        "ArrowDown" => Some(Key::Down),
        "Enter" => Some(Key::Enter),
        " " => Some(Key::Space),
        "Tab" if event.shift => Some(Key::Previous),
        "Tab" => Some(Key::Next),
        _ => None,
      };
      let panel = &actions.panel;
      // In the typed effort, Enter is its own and a space is typed.
      if matches!(event.key.as_str(), "Enter" | " ") && panel.own_effort.field.focused() {
        return;
      }
      let took = match (key, event.key.as_str()) {
        (Some(key), _) => actions.press(key, &targets),
        (None, "Escape") => {
          (actions.props.on_close)();
          true
        }
        (None, typed)
          if typed.chars().count() == 1 && !(event.ctrl || event.alt || event.meta || event.text_input_focused) =>
        {
          panel.query.update(|query| query.push_str(typed));
          panel.typed();
          panel.focus.set(Focus::Search);
          true
        }
        (None, _) => false,
      };
      if took {
        event.prevent_default();
      }
    }
  }

  /// `key` with the keys over `targets`; whether the picker took it. The arrows and Tab are always its own, as Search
  /// has the focus and would move its caret (or the focus) otherwise; Enter and Space only where they choose something,
  /// so that Space is typed into Search anywhere but on the effort row.
  fn press(&self, key: Key, targets: &Targets) -> bool {
    let panel = &self.panel;
    let mut cursor = panel.cursor.get_untracked();
    match cursor.press(key, &targets.shape.borrowed()) {
      Outcome::Ignored => return !matches!(key, Key::Enter | Key::Space),
      Outcome::Moved => {
        panel.refused.set(None);
        panel.cursor.set(cursor);
        panel.focus.set(if cursor.place == Place::Effort && targets.typed_effort {
          Focus::Effort
        } else {
          Focus::Search
        });
        self.reveal(cursor);
      }
      Outcome::Refused(at) => {
        panel.cursor.set(cursor);
        panel.refused.set(Some(at));
      }
      Outcome::Connection(at) => self.connection(at),
      Outcome::Model(at) => {
        if let Some(model) = targets.models.get(at) {
          self.model(model);
        }
      }
      Outcome::Effort(_) if targets.typed_effort => self.typed_effort(),
      Outcome::Effort(at) => {
        if let Some(effort) = targets.efforts.get(at) {
          self.effort(effort.clone());
        }
      }
    }
    true
  }

  /// Scrolls the rail or the list so that the item the keys moved to shows.
  fn reveal(&self, cursor: Cursor) {
    match (cursor.place, cursor.at) {
      (Place::List, Some(at)) => models::reveal(&self.panel.scroll, at),
      (Place::Rail, Some(at)) => rail::reveal(&self.panel.rail_scroll, &self.props.connections, at),
      _ => {}
    }
  }
}
