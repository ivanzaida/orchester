//! The compact *Runs on* picker (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`; K1 `lCApz`, K2 `rjPcV`): 300
//! wide, on `surface_raised` in a `border_strong` stroke, radius `lg`, lifted by the large shadow. From the top: the
//! connections in a row ([`rail`], which scrolls sideways past what fits), one line naming the shown one with its
//! plan's 5-hour use (on two lines where it has more to say, [`who`]), Search and the models in a list of a fixed four
//! rows ([`models`]), and the efforts the chosen model offers as one segmented row ([`effort`]), or for a connection
//! that lists none a field to type one in ([`foot`]). Its height changes only by the notes under the efforts.
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
mod foot;
mod keys;
mod model;
mod models;
mod rail;
mod view;
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
  actions::{Actions, Focus, Handles},
  foot::TypedEffort,
  models::SearchBox,
  view::View,
};
pub use self::{
  keys::{Cursor, Key, Outcome, Place, Shape},
  model::{Chosen, Connection, Detail, LIST_ROWS, Mark, Model, Notice, NoticeTone, Pick, filter_models},
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
  notes: Arc<[Notice]>,
  on_pick: OnPick,
  on_close: OnClose,
}

/// Compared by what it shows; its two actions by which they are.
impl PartialEq for RunsOnPicker {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.connections == other.connections
      && self.chosen == other.chosen
      && self.notes == other.notes
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
      notes: Arc::from([]),
      on_pick: Arc::new(on_pick),
      on_close: Arc::new(on_close),
    }
  }

  /// Notes under the efforts whichever connection is shown (a refusal, a warning about what choosing replaces).
  pub fn notes(mut self, notes: Vec<Notice>) -> Self {
    self.notes = notes.into();
    self
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
  /// The field that takes the focus on the next pass: Search on open, and when a key was typed elsewhere in the
  /// picker; the typed effort when the keys move onto it.
  focus: Signal<Focus>,
  /// The effort typed for a connection that lists none, and the chosen effort last seen (when it changes the field
  /// shows it).
  own_effort: TypedEffort,
  seen_effort: Signal<String>,
  /// The list was scrolled to the chosen model, once, when the picker opened.
  scrolled: Signal<bool>,
  search: ElementRef,
  scroll: ScrollState,
  rail_scroll: ScrollState,
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
      focus: ctx.signal(Focus::Search),
      own_effort: TypedEffort {
        value: ctx.signal(String::new()),
        field: ElementRef::new(),
      },
      seen_effort: ctx.signal(String::new()),
      scrolled: ctx.signal(false),
      search: ElementRef::new(),
      scroll: ScrollState::new(),
      rail_scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunsOnPicker>().clone();
    self.settle(ctx);
    self.show_chosen_effort(&props);
    let view = self.view(&props);
    self.reveal_shown(&props.connections, view.shown);
    self.reveal_chosen(&view);
    let id: &str = &props.id;
    let actions = Actions {
      panel: self.handles(),
      props: props.clone(),
      shown: view.shown,
    };
    let cursor = self.cursor.get();
    let looking = view.targets.shape.borrowed();
    let keys_at = |place| cursor.shown(place, &looking);
    let empty = match (view.bad_name, view.searched) {
      (true, _) => (ctx.t("ui.runsOnPicker.badModelName"), Some(PaletteColor::TextMuted)),
      (false, true) => (ctx.t("ui.runsOnPicker.noMatch"), None),
      (false, false) => (ctx.t("ui.runsOnPicker.noModels"), None),
    };
    let own_model = view.shown.is_some_and(|at| props.connections[at].typed_model);
    let head = self.head(ctx, &props, (view.shown, keys_at(Place::Rail)), &actions);
    let search = SearchBox {
      query: self.query.clone(),
      field: self.search.clone(),
      typed: {
        let handles = self.handles();
        Arc::new(move || handles.typed())
      },
    };
    let efforts = self.efforts(ctx, &props, &view, (&actions, keys_at(Place::Effort)));
    let notes = self.notes(&props, view.shown);
    Column::new()
      .id(id.to_owned())
      .width(Dimension::Px(WIDTH))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .box_shadow(ShadowStyle::Lg)
      .on_key_down(actions.keys(view.targets.clone()))
      .child(head)
      .child(models::search(ctx, id, &search, own_model))
      .child(models::list(
        ctx,
        (id, &self.scroll),
        (&view.found, view.own.as_deref(), (&empty.0, empty.1)),
        (view.model, keys_at(Place::List), cursor.at.is_some()),
        &actions.model_chooser(view.targets.models.clone()),
      ))
      .child(efforts)
      .with_children(notes)
  }
}

impl Panel {
  /// The field asked for takes the focus.
  fn settle(&self, ctx: &mut Ctx) {
    match self.focus.get() {
      Focus::Stay => return,
      Focus::Search => ctx.focus(&self.search),
      Focus::Effort => ctx.focus(&self.own_effort.field),
    }
    self.focus.set(Focus::Stay);
  }

  /// Scrolls the list to the chosen model once, as the picker opens, so that it is never hidden (`Many models`
  /// `pGj8M`).
  fn reveal_chosen(&self, view: &View<'_>) {
    if self.scrolled.get_untracked() {
      return;
    }
    self.scrolled.set(true);
    if let Some(at) = view
      .model
      .and_then(|model| view.found.iter().position(|found| &*found.id == model))
    {
      models::reveal(&self.scroll, at);
    }
  }

  /// The typed effort field shows the effort chosen whenever that changes.
  fn show_chosen_effort(&self, props: &RunsOnPicker) {
    let chosen = props.chosen.effort.as_deref().unwrap_or_default();
    if self.seen_effort.get_untracked() != chosen {
      self.seen_effort.set(chosen.to_owned());
      self.own_effort.value.set(chosen.to_owned());
    }
  }

  /// The efforts: the segmented row, or for a connection that lists none the field that takes one.
  fn efforts(
    &self,
    ctx: &mut Ctx,
    props: &RunsOnPicker,
    view: &View<'_>,
    (actions, keys_at): (&Actions, Option<usize>),
  ) -> Element {
    let id: &str = &props.id;
    if view.typed_effort {
      let typed = actions.clone();
      let submit: Arc<dyn Fn() + Send + Sync> = Arc::new(move || typed.typed_effort());
      return foot::typed_effort(
        ctx,
        id,
        (&self.own_effort, view.model.is_some(), keys_at.is_some()),
        &submit,
      );
    }
    effort::row(
      ctx,
      id,
      (view.efforts, props.chosen.effort.as_deref()),
      keys_at,
      &actions.effort_chooser(),
    )
  }

  /// The notes under the efforts: the shown connection's, then the picker's own.
  fn notes(&self, props: &RunsOnPicker, shown: Option<usize>) -> Option<Element> {
    let own = shown.and_then(|at| props.connections[at].notice.as_ref());
    let notes: Vec<&Notice> = own.into_iter().chain(props.notes.iter()).collect();
    (!notes.is_empty()).then(|| foot::notes(&props.id, &notes))
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
      own_effort: self.own_effort.clone(),
      scroll: self.scroll.clone(),
      rail_scroll: self.rail_scroll.clone(),
    }
  }
}
