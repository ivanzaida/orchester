//! The compact *Runs on* picker (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`; K1 `lCApz`, K2 `rjPcV`): 300
//! wide, on `surface_raised` in a `border_strong` stroke, radius `lg`, lifted by the large shadow. From the top: the
//! connections in a row ([`rail`]), one line naming the shown one with its plan's 5-hour use ([`who`]), Search and the
//! models in a list of a fixed four rows ([`models`]), and the efforts the chosen model offers as one segmented row
//! ([`effort`]). It has no foot, so it never changes height.
//!
//! It knows nothing of what opens it: its caller gives the connections with their models and what is chosen, and gets
//! each choice back as a [`Pick`] through `on_pick`; it applies nothing itself, so what it shows chosen is what its
//! caller passes next. Keyboard ([`keys`]): Search has the focus on open, a key typed anywhere in the picker goes to
//! it, ← → move along the rail, ↑ ↓ in the list, Enter chooses where the keys are (a connection that cannot be chosen
//! says why on the line under the rail) and Esc calls `on_close`. A connection that cannot be chosen sits at 40 % and
//! its tooltip says why.

mod effort;
mod keys;
mod model;
mod models;
mod rail;
mod who;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::Column,
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

pub use self::{
  effort::effort_word,
  keys::{Cursor, Key, Outcome, Place, Shape},
  model::{Chosen, Connection, LIST_ROWS, Mark, Model, Pick, filter_models},
  models::list_height,
};
use self::{models::SearchBox, rail::Choose};
use crate::theme::ExtraSpacing;

/// The picker's width (`ZcGGy`).
pub const WIDTH: f32 = 300.0;

type OnPick = Arc<dyn Fn(Pick) + Send + Sync>;
type OnClose = Arc<dyn Fn() + Send + Sync>;

/// The picker over `connections` with `chosen` chosen, named `id` (its elements' ids start with it).
#[derive(Clone)]
pub struct RunsOnPicker {
  id: Arc<str>,
  connections: Arc<[Connection]>,
  chosen: Chosen,
  on_pick: OnPick,
  on_close: OnClose,
}

/// Compared by what it shows; its two actions by which they are.
impl PartialEq for RunsOnPicker {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.connections == other.connections
      && self.chosen == other.chosen
      && Arc::ptr_eq(&self.on_pick, &other.on_pick)
      && Arc::ptr_eq(&self.on_close, &other.on_close)
  }
}

impl DevtoolsInspectable for RunsOnPicker {}

impl RunsOnPicker {
  pub fn new(
    id: impl Into<Arc<str>>,
    (connections, chosen): (Vec<Connection>, Chosen),
    on_pick: impl Fn(Pick) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      connections: connections.into(),
      chosen,
      on_pick: Arc::new(on_pick),
      on_close: Arc::new(on_close),
    }
  }

  /// The picker, mounted in `ctx` under its id so it keeps its state across passes.
  pub fn element(self, ctx: &mut Ctx) -> Element {
    let key = format!("{}-picker", self.id);
    ctx.mount_keyed::<Panel>(&key, self)
  }
}

/// The picker's state: the connection shown, what Search holds, where the keys are and where the list is scrolled.
struct Panel {
  /// The connection shown, by its id, and the chosen one last seen (when it changes the picker shows it).
  viewing: Signal<String>,
  seen: Signal<String>,
  query: Signal<String>,
  cursor: Signal<Cursor>,
  /// The connection the keys tried to choose that cannot be chosen: the line under the rail says why.
  refused: Signal<Option<usize>>,
  /// Search takes the focus on the next pass: on open, and when a key was typed elsewhere in the picker.
  focus: Signal<bool>,
  search: ElementRef,
  scroll: ScrollState,
}

impl Component for Panel {
  type Props = RunsOnPicker;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      viewing: ctx.signal(String::new()),
      seen: ctx.signal(String::new()),
      query: ctx.signal(String::new()),
      cursor: ctx.signal(Cursor::START),
      refused: ctx.signal(None),
      focus: ctx.signal(true),
      search: ElementRef::new(),
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunsOnPicker>().clone();
    let shown = self.shown_connection(&props);
    if self.focus.get() {
      ctx.focus(&self.search);
      self.focus.set(false);
    }
    let id: &str = &props.id;
    let connections = &props.connections;
    let query = self.query.get();
    let found = shown.map_or_else(Vec::new, |at| filter_models(&connections[at].models, &query));
    let on_chosen = shown.is_some_and(|at| connections[at].id == props.chosen.connection);
    let model = props.chosen.model.as_deref().filter(|_| on_chosen);
    let shape = OwnedShape {
      rail: connections
        .iter()
        .map(|connection| connection.unavailable.is_none())
        .collect(),
      viewing: shown,
      list: found.len(),
      chosen: found.iter().position(|found| Some(&*found.id) == model),
      searched: !query.trim().is_empty(),
    };
    let ids: Vec<Arc<str>> = found.iter().map(|found| found.id.clone()).collect();
    let actions = Actions {
      panel: self.handles(),
      props: props.clone(),
      shown,
    };
    let cursor = self.cursor.get();
    let looking = shape.borrowed();
    let empty = ctx.t(if shape.searched {
      "ui.runsOnPicker.noMatch"
    } else {
      "ui.runsOnPicker.noModels"
    });
    let efforts = shown
      .filter(|_| on_chosen)
      .and_then(|at| {
        connections[at]
          .models
          .iter()
          .find(|candidate| Some(&*candidate.id) == model)
      })
      .map(|offered| offered.efforts.as_slice());
    let head = self.head(ctx, &props, (shown, cursor.shown(Place::Rail, &looking)), &actions);
    let search = SearchBox {
      query: self.query.clone(),
      field: self.search.clone(),
      typed: {
        let handles = self.handles();
        Arc::new(move || handles.typed())
      },
    };
    Column::new()
      .id(id.to_owned())
      .width(Dimension::Px(WIDTH))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .box_shadow(ShadowStyle::Lg)
      .on_key_down(actions.keys(shape.clone(), ids.clone()))
      .child(head)
      .child(models::search(ctx, id, &search))
      .child(models::list(
        ctx,
        (id, &self.scroll),
        (&found, &empty),
        (model, cursor.shown(Place::List, &looking)),
        &actions.model_chooser(ids),
      ))
      .child(effort::row(
        ctx,
        id,
        (efforts, props.chosen.effort.as_deref()),
        &actions.effort_chooser(),
      ))
  }
}

impl Panel {
  /// The rail and the line under it: of the connection shown, or of one the keys tried to choose that cannot be.
  fn head(
    &self,
    ctx: &mut Ctx,
    props: &RunsOnPicker,
    (shown, keys_at): (Option<usize>, Option<usize>),
    actions: &Actions,
  ) -> Column {
    let id: &str = &props.id;
    let refused = self.refused.get().and_then(|at| props.connections.get(at));
    let described = refused.or(shown.map(|at| &props.connections[at]));
    let why = refused.and_then(|connection| connection.unavailable.as_ref());
    Column::new()
      .id(format!("{id}-connection"))
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Snug.role())
          .right(ExtraSpacing::Snug.role())
          .bottom(SpacingSize::Sm)
          .left(ExtraSpacing::Snug.role()),
      )
      .spacing(ExtraSpacing::Snug.role())
      .child(rail::rail(
        ctx,
        id,
        &props.connections,
        (shown, keys_at),
        &actions.connection_chooser(),
      ))
      .with_children(described.map(|connection| who::who(ctx, id, connection, why)))
  }

  /// The place on the rail of the connection shown: the one the picker was told to show, else the first that can be
  /// chosen, else the first. A change of what is chosen shows that connection.
  fn shown_connection(&self, props: &RunsOnPicker) -> Option<usize> {
    let chosen = &*props.chosen.connection;
    let wanted = if self.seen.get_untracked() == chosen {
      self.viewing.get()
    } else {
      self.seen.set(chosen.to_owned());
      self.viewing.set(chosen.to_owned());
      chosen.to_owned()
    };
    let connections = &props.connections;
    connections
      .iter()
      .position(|connection| *connection.id == *wanted)
      .or_else(|| {
        connections
          .iter()
          .position(|connection| connection.unavailable.is_none())
      })
      .or((!connections.is_empty()).then_some(0))
  }

  fn handles(&self) -> Handles {
    Handles {
      viewing: self.viewing.clone(),
      query: self.query.clone(),
      cursor: self.cursor.clone(),
      refused: self.refused.clone(),
      focus: self.focus.clone(),
      scroll: self.scroll.clone(),
    }
  }
}

/// [`Shape`] owned, for the key handler to keep.
#[derive(Clone)]
struct OwnedShape {
  rail: Vec<bool>,
  viewing: Option<usize>,
  list: usize,
  chosen: Option<usize>,
  searched: bool,
}

impl OwnedShape {
  fn borrowed(&self) -> Shape<'_> {
    Shape {
      rail: &self.rail,
      viewing: self.viewing,
      list: self.list,
      chosen: self.chosen,
      searched: self.searched,
    }
  }
}

/// The picker's signals, which choosing and the keys change.
#[derive(Clone)]
struct Handles {
  viewing: Signal<String>,
  query: Signal<String>,
  cursor: Signal<Cursor>,
  refused: Signal<Option<usize>>,
  focus: Signal<bool>,
  scroll: ScrollState,
}

impl Handles {
  /// What was typed in Search: the keys go to the models it finds, from the first.
  fn typed(&self) {
    self.cursor.update(Cursor::typed);
    self.refused.set(None);
  }
}

/// What choosing in the picker does.
#[derive(Clone)]
struct Actions {
  panel: Handles,
  props: RunsOnPicker,
  /// The place on the rail of the connection shown.
  shown: Option<usize>,
}

impl Actions {
  fn connection_chooser(&self) -> Choose {
    let actions = self.clone();
    Arc::new(move |at| actions.connection(at))
  }

  /// What choosing the model at a place in the list does, the list holding the models `ids`.
  fn model_chooser(&self, ids: Vec<Arc<str>>) -> Choose {
    let actions = self.clone();
    Arc::new(move |at| {
      if let Some(model) = ids.get(at) {
        actions.model(model);
      }
    })
  }

  fn effort_chooser(&self) -> Arc<dyn Fn(Option<Arc<str>>) + Send + Sync> {
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

  /// Hands back `model` of the connection shown unless it is what is chosen already.
  fn model(&self, model: &Arc<str>) {
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

  /// Hands back `effort` unless it is what is chosen already.
  fn effort(&self, effort: Option<Arc<str>>) {
    if self.props.chosen.effort != effort {
      (self.props.on_pick)(Pick::Effort(effort));
    }
  }

  /// The handler of the picker's keys over `shape`, the list holding the models `ids`.
  fn keys(&self, shape: OwnedShape, ids: Vec<Arc<str>>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
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
        _ => None,
      };
      let panel = &actions.panel;
      let took = match (key, event.key.as_str()) {
        (Some(key), _) => actions.press(key, &shape, &ids),
        (None, "Escape") => {
          (actions.props.on_close)();
          true
        }
        (None, typed)
          if typed.chars().count() == 1 && !(event.ctrl || event.alt || event.meta || event.text_input_focused) =>
        {
          panel.query.update(|query| query.push_str(typed));
          panel.typed();
          panel.focus.set(true);
          true
        }
        (None, _) => false,
      };
      if took {
        event.prevent_default();
      }
    }
  }

  /// `key` with the keys over `shape`; whether the picker took it. The arrows are always its own, as Search has the
  /// focus and would move its caret otherwise; Enter only where it chooses something.
  fn press(&self, key: Key, shape: &OwnedShape, ids: &[Arc<str>]) -> bool {
    let panel = &self.panel;
    let mut cursor = panel.cursor.get_untracked();
    match cursor.press(key, &shape.borrowed()) {
      Outcome::Ignored => return key != Key::Enter,
      Outcome::Moved => {
        panel.refused.set(None);
        panel.cursor.set(cursor);
        if let (Place::List, Some(at)) = (cursor.place, cursor.at) {
          models::reveal(&panel.scroll, at);
        }
      }
      Outcome::Refused(at) => {
        panel.cursor.set(cursor);
        panel.refused.set(Some(at));
      }
      Outcome::Connection(at) => self.connection(at),
      Outcome::Model(at) => {
        if let Some(model) = ids.get(at) {
          self.model(model);
        }
      }
    }
    true
  }
}
