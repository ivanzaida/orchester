//! The compact *Runs on* picker (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`; K1 `lCApz`, K2 `rjPcV`): 300
//! wide, on `surface_raised` in a `border_strong` stroke, radius `lg`, lifted by the large shadow. From the top: the
//! connections in a row ([`rail`], which scrolls sideways past what fits), one line naming the shown one with its
//! plan's 5-hour use ([`who`]), Search and the models in a list of a fixed four rows ([`models`]), and the efforts the
//! chosen model offers as one segmented row ([`effort`]). It has no foot, so it never changes height.
//!
//! It knows nothing of what opens it: its caller gives the connections with their models and what is chosen, and gets
//! each choice back as a [`Pick`] through `on_pick`; it applies nothing itself, so what it shows chosen is what its
//! caller passes next. Keyboard ([`keys`], [`actions`]): Search has the focus on open and a key typed anywhere in the
//! picker goes to it; Tab and Shift+Tab move the keys between the rail, the models and the effort row, where the
//! current item shows the app's focus ring; ← → move within the rail and the efforts, ↑ ↓ in the list, Enter (and Space
//! on an effort) chooses where the keys are (a connection that cannot be chosen says why on the line under the rail)
//! and Esc calls `on_close`. A connection that cannot be chosen sits at 40 % and its tooltip says why.

mod actions;
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
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::Column,
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use self::{
  actions::{Actions, Handles, OwnedShape, Targets},
  models::SearchBox,
};
pub use self::{
  keys::{Cursor, Key, Outcome, Place, Shape},
  model::{Chosen, Connection, LIST_ROWS, Mark, Model, Pick, filter_models},
  models::list_height,
};
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

/// The picker's state: the connection shown, what Search holds, where the keys are and where the list and the rail are
/// scrolled.
struct Panel {
  /// The connection shown, by its id, and the chosen one last seen (when it changes the picker shows it).
  viewing: Signal<String>,
  seen: Signal<String>,
  /// The place on the rail last scrolled into view because it became the connection shown.
  revealed: Signal<Option<usize>>,
  query: Signal<String>,
  cursor: Signal<Cursor>,
  /// The connection the keys tried to choose that cannot be chosen: the line under the rail says why.
  refused: Signal<Option<usize>>,
  /// Search takes the focus on the next pass: on open, and when a key was typed elsewhere in the picker.
  focus: Signal<bool>,
  search: ElementRef,
  scroll: ScrollState,
  rail_scroll: ScrollState,
}

/// What a pass shows, worked out from the props and what Search holds.
struct View<'a> {
  /// The place on the rail of the connection shown.
  shown: Option<usize>,
  /// The models of the connection shown that Search finds.
  found: Vec<&'a Model>,
  /// The chosen model, when it is on the connection shown, and the efforts it offers.
  model: Option<&'a str>,
  efforts: Option<&'a [Arc<str>]>,
  searched: bool,
  targets: Targets,
}

impl Component for Panel {
  type Props = RunsOnPicker;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      viewing: ctx.signal(String::new()),
      seen: ctx.signal(String::new()),
      revealed: ctx.signal(None),
      query: ctx.signal(String::new()),
      cursor: ctx.signal(Cursor::START),
      refused: ctx.signal(None),
      focus: ctx.signal(true),
      search: ElementRef::new(),
      scroll: ScrollState::new(),
      rail_scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunsOnPicker>().clone();
    if self.focus.get() {
      ctx.focus(&self.search);
      self.focus.set(false);
    }
    let view = self.view(&props);
    self.reveal_shown(&props.connections, view.shown);
    let id: &str = &props.id;
    let actions = Actions {
      panel: self.handles(),
      props: props.clone(),
      shown: view.shown,
    };
    let cursor = self.cursor.get();
    let looking = view.targets.shape.borrowed();
    let keys_at = |place| cursor.shown(place, &looking);
    let empty = ctx.t(if view.searched {
      "ui.runsOnPicker.noMatch"
    } else {
      "ui.runsOnPicker.noModels"
    });
    let head = self.head(ctx, &props, (view.shown, keys_at(Place::Rail)), &actions);
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
      .on_key_down(actions.keys(view.targets.clone()))
      .child(head)
      .child(models::search(ctx, id, &search))
      .child(models::list(
        ctx,
        (id, &self.scroll),
        (&view.found, &empty),
        (view.model, keys_at(Place::List), cursor.at.is_some()),
        &actions.model_chooser(view.targets.models.clone()),
      ))
      .child(effort::row(
        ctx,
        id,
        (view.efforts, props.chosen.effort.as_deref()),
        keys_at(Place::Effort),
        &actions.effort_chooser(),
      ))
  }
}

impl Panel {
  /// The connection shown, the models Search finds on it and the efforts the chosen model offers, with what the keys
  /// move over and hand back.
  fn view<'a>(&self, props: &'a RunsOnPicker) -> View<'a> {
    let shown = self.shown_connection(props);
    let connections = &props.connections;
    let query = self.query.get();
    let found = shown.map_or_else(Vec::new, |at| filter_models(&connections[at].models, &query));
    let on_chosen = shown.is_some_and(|at| connections[at].id == props.chosen.connection);
    let model = props.chosen.model.as_deref().filter(|_| on_chosen);
    let efforts = shown
      .filter(|_| on_chosen)
      .and_then(|at| {
        connections[at]
          .models
          .iter()
          .find(|candidate| Some(&*candidate.id) == model)
      })
      .map(|offered| offered.efforts.as_slice());
    // *Default* first, then what the model offers; none at all while there is no model chosen.
    let levels: Vec<Option<Arc<str>>> = efforts.map_or_else(Vec::new, |offered| {
      std::iter::once(None).chain(offered.iter().cloned().map(Some)).collect()
    });
    let searched = !query.trim().is_empty();
    let shape = OwnedShape {
      rail: connections
        .iter()
        .map(|connection| connection.unavailable.is_none())
        .collect(),
      viewing: shown,
      list: found.len(),
      chosen: found.iter().position(|found| Some(&*found.id) == model),
      searched,
      efforts: levels.len(),
      effort: levels
        .iter()
        .position(|level| level.as_deref() == props.chosen.effort.as_deref()),
    };
    let targets = Targets {
      shape,
      models: found.iter().map(|found| found.id.clone()).collect(),
      efforts: levels,
    };
    View {
      shown,
      found,
      model,
      efforts,
      searched,
      targets,
    }
  }

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
        (id, &self.rail_scroll),
        &props.connections,
        (shown, keys_at),
        &actions.connection_chooser(),
      ))
      .with_children(described.map(|connection| who::who(ctx, id, connection, why)))
  }

  /// Scrolls the rail to the connection shown when it became the shown one (on open, and when the chosen connection
  /// changes), so that it is never hidden; a rail the person scrolled by hand stays where it is otherwise.
  fn reveal_shown(&self, connections: &[Connection], shown: Option<usize>) {
    if self.revealed.get_untracked() == shown {
      return;
    }
    self.revealed.set(shown);
    if let Some(at) = shown {
      rail::reveal(&self.rail_scroll, connections, at);
    }
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
      rail_scroll: self.rail_scroll.clone(),
    }
  }
}
