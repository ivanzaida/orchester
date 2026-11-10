//! The Agent picker (`Agent picker` `Zn8BM` in K4 `CwLfL`; open under a step's Agent field in P10 `HjgZ8`): 320 wide,
//! on `surface_raised` in a `border_strong` stroke, radius `lg`, lifted by the large shadow. From the top: Search, a
//! list of a fixed height that scrolls, grouped *Agent profiles* and *Tools* ([`model`]), and the foot with *+ New
//! agent profile* and *Agents ↗* ([`view`]). Each row is an item's mark, its name over what it runs on, and a check
//! when it is the chosen one; one that is not ready says why in `warning` in place of what it runs on, and can still be
//! chosen.
//!
//! It knows nothing of what opens it: its caller gives the items and which is chosen, and gets each choice back as the
//! item's key through `on_pick`; it applies nothing itself, so what it shows chosen is what its caller passes next.
//! Keyboard: Search has the focus on open and typing filters; ↑ and ↓ move the keys over the items (the first press
//! shows the chosen one), Enter chooses where they are and Esc calls `on_close`.

mod model;
mod view;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle},
  },
  components::Column,
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension},
};

use self::view::{Listing, SearchBox};
pub use self::{
  model::{Group, Item, Mark, Row, extent, filter, keys_at, moved, rows},
  view::mark,
};

/// The picker's width (`Zn8BM`).
pub const WIDTH: f32 = 320.0;
/// The list's height (`List` `f2sV4`): always this, so the picker never changes height.
pub const LIST_HEIGHT: f32 = 224.0;
/// An item's row (`Agent · Builder` `k4ZDcS`), a group's heading (`eVtiH`), Search (`A4pzjX`) and the foot (`T8nQb5`).
pub const ROW_HEIGHT: f32 = 40.0;
pub const GROUP_HEIGHT: f32 = 24.0;
const SEARCH_HEIGHT: f32 = 32.0;
const FOOT_HEIGHT: f32 = 34.0;

type OnPick = Arc<dyn Fn(Arc<str>) + Send + Sync>;
type Action = Arc<dyn Fn() + Send + Sync>;

/// The picker over `items` with the item whose key is `chosen` chosen, named `id` (its elements' ids start with it).
#[derive(Clone)]
pub struct AgentPicker {
  id: Arc<str>,
  items: Arc<[Item]>,
  chosen: Option<Arc<str>>,
  on_pick: OnPick,
  on_close: Action,
  on_new: Action,
  on_agents: Action,
}

/// Compared by what it shows; its actions by which they are.
impl PartialEq for AgentPicker {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.items == other.items
      && self.chosen == other.chosen
      && Arc::ptr_eq(&self.on_pick, &other.on_pick)
      && Arc::ptr_eq(&self.on_close, &other.on_close)
      && Arc::ptr_eq(&self.on_new, &other.on_new)
      && Arc::ptr_eq(&self.on_agents, &other.on_agents)
  }
}

impl DevtoolsInspectable for AgentPicker {}

/// What the picker's callers say of what it does with its foot and its end.
pub struct Actions {
  pub on_pick: Box<dyn Fn(Arc<str>) + Send + Sync>,
  /// Esc was pressed.
  pub on_close: Box<dyn Fn() + Send + Sync>,
  /// *+ New agent profile* was pressed.
  pub on_new: Box<dyn Fn() + Send + Sync>,
  /// *Agents ↗* was pressed.
  pub on_agents: Box<dyn Fn() + Send + Sync>,
}

impl AgentPicker {
  pub fn new(id: impl Into<Arc<str>>, (items, chosen): (Vec<Item>, Option<Arc<str>>), actions: Actions) -> Self {
    Self {
      id: id.into(),
      items: items.into(),
      chosen,
      on_pick: Arc::from(actions.on_pick),
      on_close: Arc::from(actions.on_close),
      on_new: Arc::from(actions.on_new),
      on_agents: Arc::from(actions.on_agents),
    }
  }

  /// The picker, mounted in `ctx` under its id so it keeps its state across passes.
  pub fn element(self, ctx: &mut Ctx) -> Element {
    let key = format!("{}-picker", self.id);
    ctx.mount_keyed::<Panel>(&key, self)
  }
}

/// The picker's state: what Search holds, where the keys are and where the list is scrolled.
struct Panel {
  query: Signal<String>,
  /// Where an arrow put the keys, among the items shown; `None` until one did.
  keys: Signal<Option<usize>>,
  /// Search takes the focus on the first pass.
  focused: Signal<bool>,
  /// The list was scrolled to the chosen item, once, when the picker opened.
  scrolled: Signal<bool>,
  search: ElementRef,
  scroll: ScrollState,
}

impl Component for Panel {
  type Props = AgentPicker;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      query: ctx.signal(String::new()),
      keys: ctx.signal(None),
      focused: ctx.signal(false),
      scrolled: ctx.signal(false),
      search: ElementRef::new(),
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AgentPicker>().clone();
    if !self.focused.get_untracked() {
      self.focused.set(true);
      ctx.focus(&self.search);
    }
    let query = self.query.get();
    let found = filter(&props.items, &query);
    let rows = rows(&props.items, &found);
    let searched = !query.trim().is_empty();
    let chosen_at = found
      .iter()
      .position(|&at| Some(&props.items[at].key) == props.chosen.as_ref());
    self.reveal_chosen(&rows, chosen_at);
    let pressed = self.keys.get();
    let keys = keys_at(pressed, searched, found.len());
    let choose: view::Choose = {
      let (found, items, on_pick) = (found.clone(), props.items.clone(), props.on_pick.clone());
      Arc::new(move |shown| {
        if let Some(item) = found.get(shown).map(|&at| &items[at]) {
          on_pick(item.key.clone());
        }
      })
    };
    let empty = ctx.t(if props.items.is_empty() {
      "ui.agentPicker.noAgents"
    } else {
      "ui.agentPicker.noMatch"
    });
    let search = SearchBox {
      query: self.query.clone(),
      field: self.search.clone(),
      typed: {
        let keys = self.keys.clone();
        Arc::new(move || keys.set(None))
      },
    };
    let keyboard = Keys {
      keys: self.keys.clone(),
      scroll: self.scroll.clone(),
      rows: rows.clone(),
      at: (keys, chosen_at, found.len()),
      choose: choose.clone(),
      on_close: props.on_close.clone(),
    };
    let id: &str = &props.id;
    let (new, agents) = (props.on_new.clone(), props.on_agents.clone());
    let listing = Listing {
      rows: &rows,
      items: &props.items,
      chosen: props.chosen.as_deref(),
      keys: (keys, pressed.is_some()),
      empty: &empty,
    };
    let list = view::list(ctx, (id, &self.scroll), &listing, &choose);
    Column::new()
      .id(id.to_owned())
      .width(Dimension::Px(WIDTH))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .box_shadow(ShadowStyle::Lg)
      .on_key_down(move |event: KeyboardEvent| keyboard.press(&event))
      .child(view::search(ctx, id, &search))
      .child(list)
      .child(view::foot(ctx, id, move || new(), move || agents()))
  }
}

impl Panel {
  /// Scrolls the list to the chosen item once, as the picker opens, so that it is never hidden.
  fn reveal_chosen(&self, rows: &[Row], chosen: Option<usize>) {
    if self.scrolled.get_untracked() {
      return;
    }
    self.scrolled.set(true);
    if let Some(shown) = chosen {
      reveal(&self.scroll, rows, shown);
    }
  }
}

/// Scrolls the list held by `scroll` so that the item shown at `shown` (with its heading, when it is a group's first)
/// is inside it.
fn reveal(scroll: &ScrollState, rows: &[Row], shown: usize) {
  let Some((top, height)) = extent(rows, shown) else {
    return;
  };
  let seen = scroll.scroll_y();
  if top < seen {
    scroll.set_scroll_pending(0.0, top);
  } else if top + height > seen + LIST_HEIGHT {
    scroll.set_scroll_pending(0.0, top + height - LIST_HEIGHT);
  }
}

/// The picker's keys over the items shown now.
struct Keys {
  keys: Signal<Option<usize>>,
  scroll: ScrollState,
  rows: Vec<Row>,
  /// Where the keys are, where the chosen item is among the items shown, and how many there are.
  at: (Option<usize>, Option<usize>, usize),
  choose: view::Choose,
  on_close: Action,
}

impl Keys {
  /// Takes `event` when it is one of the picker's keys.
  fn press(&self, event: &KeyboardEvent) {
    if event.default_prevented() || event.composing {
      return;
    }
    let (at, chosen, count) = self.at;
    let took = match event.key.as_str() {
      "ArrowDown" | "ArrowUp" => {
        let by = if event.key == "ArrowDown" { 1 } else { -1 };
        let next = moved(at, chosen, count, by);
        if let Some(shown) = next {
          self.keys.set(Some(shown));
          reveal(&self.scroll, &self.rows, shown);
        }
        true
      }
      "Enter" => match at {
        Some(shown) => {
          (self.choose)(shown);
          true
        }
        None => false,
      },
      "Escape" => {
        (self.on_close)();
        true
      }
      _ => false,
    };
    if took {
      event.prevent_default();
    }
  }
}
