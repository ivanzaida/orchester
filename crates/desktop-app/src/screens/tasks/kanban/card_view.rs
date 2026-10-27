//! A card as the board mounts it: a click opens its task, by a browser's rules as a row's press does; Shift+click
//! selects it; a drag lifts it only once the pointer moved [`LIFT`] with the button held, so a click never moves a
//! card (`02 Dragging` `Yyeme`). While the pointer is on it, its age gives way to *More actions* (`07 Card actions and
//! keys` `pXzH8`). A card of a read-only list (not a member any more, or kept *Not shared*) neither lifts nor is
//! selected, and a *Not shared* one has no menu.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{DragEvent, MouseButton, MouseButtonMask, MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Button, Column},
  core::{ElementRef, Signal},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::project::TaskKey;

use super::{
  actions::{BoardState, Drag, Focused, MenuPage, OpenMenu, Target},
  card::{CardLook, CardModel, content},
  display::Display,
  drop::Dropping,
  menu::card_menu,
};
use crate::{
  navigation::{Navigation, press::LinkPress},
  routes::{ProjectPage, Route, TaskView},
  theme::HELD_CARD_OPACITY,
  ui::{icon::Icon, icon_button::IconButton},
};

/// How far the pointer moves with the button held before a card lifts.
const LIFT: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CardProps {
  pub card: CardModel,
  pub display: Display,
  pub look: CardLook,
  /// Where the card is.
  pub place: Target,
  /// A drag lifts it.
  pub lifts: bool,
  /// It has *More actions*.
  pub menu: bool,
}

impl DevtoolsInspectable for CardProps {}

pub struct CardView {
  hovered: Signal<bool>,
  open: Signal<bool>,
  anchor: ElementRef,
  frame: ElementRef,
}

impl Component for CardView {
  type Props = CardProps;

  fn create(ctx: &mut Ctx) -> Self {
    let open = ctx.signal(false);
    // The menu closed by a choice, a press outside or Esc: the board no longer holds it open.
    let (menu, task) = (BoardState::of(ctx).menu, ctx.props::<CardProps>().card.row.id);
    ctx.watch(&open, move |open: &bool| {
      if !*open && menu.get_untracked().0.is_some_and(|(held, _)| held == task) {
        menu.set(OpenMenu(None));
      }
    });
    Self {
      hovered: ctx.signal(false),
      open,
      anchor: ElementRef::default(),
      frame: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CardProps>().clone();
    let state = BoardState::of(ctx);
    let key = props.card.row.key.clone();
    let task = props.card.row.id;
    let held = props.menu && state.menu.get().0.is_some_and(|(held, _)| held == task);
    if held != self.open.get_untracked() {
      self.open.set(held);
    }
    let acting = props.menu && (self.hovered.get() || held);
    let more = acting.then(|| {
      let menu = state.menu.clone();
      let button = IconButton::new(
        format!("task-more-{key}"),
        Icon::Ellipsis,
        ctx.t_args("task_close.menu.label", [("task", &*key)]),
      )
      .compact()
      .element(self.anchor.clone())
      .on_click(move || {
        menu.set(OpenMenu((!held).then_some((task, MenuPage::Commands))));
      });
      if held { button.active() } else { button }.render(ctx)
    });
    let look = CardLook { acting, ..props.look };
    let parts = content(ctx, &props.card, (&props.display, look), more);
    let frame = frame(&props, look, parts).ref_element(self.frame.clone());
    let menu = held.then(|| card_menu(ctx, &props.card, (&self.open, &self.anchor)));
    let card = self.pressable(ctx, &props, frame);
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(card)
      .with_children(menu)
  }
}

impl CardView {
  /// The card that opens its task, selects on Shift+click unless it is read-only, and lifts on a drag.
  fn pressable(&self, ctx: &mut Ctx, props: &CardProps, frame: Column) -> Element {
    let state = BoardState::of(ctx);
    let dropping = Dropping::of(ctx);
    let key = props.card.row.key.clone();
    let task: TaskKey = key.parse().expect("a task's key reads back as a task key");
    let page = if props.card.row.not_shared {
      ProjectPage::KeptTask(task, TaskView::default())
    } else {
      ProjectPage::Task(task, TaskView::default())
    };
    let open = Navigation::of(ctx).to(Route::Project(props.card.project, page));
    let (middle, selected, focused, id) = (
      open.clone(),
      state.selected.clone(),
      state.focused.clone(),
      props.card.row.id,
    );
    let selects = !dropping.locked(&props.card);
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let button = Button::empty()
      .id(format!("task-card-{key}"))
      .width(Dimension::Pct(100.0))
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", props.card.row.title.clone())
      .describe("state", if props.look.selected { "selected" } else { "enabled" })
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_mouse_down(|event: MouseEvent| {
        if event.button == MouseButton::Middle {
          event.prevent_default();
        }
      })
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        let press = LinkPress::of(&event);
        if press.shift && !press.command {
          if !selects {
            return;
          }
          focused.set(Focused(Some(id)));
          selected.update(|selected| {
            if !selected.0.remove(&id) {
              selected.0.insert(id);
            }
          });
          return;
        }
        if let Some(at) = press.open_in() {
          open(at);
        }
      })
      .on_mouse_click(MouseButton::Middle, move |event: MouseEvent| {
        event.stop_propagation();
        if let Some(at) = LinkPress::of(&event).open_in() {
          middle(at);
        }
      })
      .child(frame);
    if !props.lifts {
      return button.into();
    }
    let (card, place, frame_ref) = (props.card.clone(), props.place.clone(), self.frame.clone());
    let started = state.clone();
    button
      .start_drag_buttons(MouseButtonMask::LEFT)
      .on_drag_start(move |_: DragEvent| *started.set_down.lock() = false)
      .on_drag_move(move |event: DragEvent| moved(&state, (&card, &place, &frame_ref), &event))
      .on_drag_end(move |_: DragEvent| ended(&dropping))
      .into()
  }
}

/// The card's frame: `surface_panel` in a `border` stroke; `accent_muted` in an `accent` stroke when selected; an
/// outline where a lifted card was; faded where a drop waits for the person; a `border_focus` stroke for the card the
/// keys reach.
fn frame(props: &CardProps, look: CardLook, parts: Column) -> Column {
  let (fill, stroke) = match () {
    () if look.lifted => (None, PaletteColor::Border),
    () if look.selected => (Some(PaletteColor::AccentMuted), PaletteColor::Accent),
    () if look.waiting => (Some(PaletteColor::SurfacePanel), PaletteColor::BorderStrong),
    () if look.focused => (Some(PaletteColor::SurfacePanel), PaletteColor::BorderFocus),
    () if props.card.highlighted => (Some(PaletteColor::InfoMuted), PaletteColor::Border),
    () => (Some(PaletteColor::SurfacePanel), PaletteColor::Border),
  };
  let frame = Column::new()
    .id(format!("task-card-frame-{}", props.card.row.key))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, stroke))
    .hovered_style(Style::new().border_inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(parts.opacity(if look.lifted { 0.0 } else { 1.0 }));
  let frame = match fill {
    Some(fill) => frame.background(fill),
    None => frame,
  };
  if look.waiting {
    frame.opacity(HELD_CARD_OPACITY)
  } else {
    frame
  }
}

/// The card lifted for the person's drag, under the pointer: `surface_raised` in a `border_strong` stroke, lifted by
/// the `md` shadow (`KON-27` `yYfRU`).
pub fn lifted_frame(parts: Column) -> Column {
  Column::new()
    .id("tasks-board-lifted")
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .box_shadow(lurq::app::theme::ShadowStyle::Md)
    .child(parts)
}

/// The pointer moved with the card's button held: once it went [`LIFT`], the card lifts; then the board follows where
/// it is over.
fn moved(state: &BoardState, (card, place, frame): (&CardModel, &Target, &ElementRef), event: &DragEvent) {
  if *state.set_down.lock() {
    return;
  }
  let lifted = state.drag.get_untracked();
  if lifted.is_none() && event.total_delta_x.hypot(event.total_delta_y) < LIFT {
    return;
  }
  state.pointer.set((event.x, event.y));
  let over = state.target_at((event.x, event.y));
  match lifted {
    None => {
      let bounds = frame.bounds();
      state.refused.set(None);
      state.drag.set(Some(Drag {
        card: card.clone(),
        from: place.clone(),
        grab: (event.start_x - bounds.x, event.start_y - bounds.y),
        size: (bounds.width, bounds.height),
        over,
      }));
    }
    Some(drag) if drag.over != over => state.drag.set(Some(Drag { over, ..drag })),
    Some(_) => {}
  }
}

/// The button was let go: a lifted card drops where it is over, unless Esc set it down.
fn ended(dropping: &Dropping) {
  let state = &dropping.state;
  let set_down = std::mem::take(&mut *state.set_down.lock());
  let Some(drag) = state.drag.get_untracked() else {
    return;
  };
  state.drag.set(None);
  if set_down {
    return;
  }
  if let Some(target) = drag.over.filter(|target| *target != drag.from) {
    dropping.drop_card(&drag.card, &target);
  }
}
