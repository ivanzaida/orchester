//! What the flow keeps while it is drawn for editing: which card the pointer or the keys are on, the card whose ⋯ menu
//! is open, the drag under way with where it would land, why the last move was refused, and where the cards stand on
//! the screen, which a drag is dropped by.

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{ElementRect, ElementRef, Signal},
};
use orchester_core::resource::{MoveError, PipelineEdit, StepKey};
use orchester_workspace::pipelines::flow::StepCard;
use parking_lot::Mutex;

/// A card the pointer, the keys or a menu is on, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Held(pub Option<StepKey>);
impl DevtoolsInspectable for Held {}

impl Held {
  pub(super) fn is(&self, key: &StepKey) -> bool {
    self.0.as_ref() == Some(key)
  }
}

/// A drag lifted from a card's handle.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Drag {
  pub key: StepKey,
  /// The card as it was drawn, which the floating copy draws again.
  pub card: StepCard,
  /// Where the pointer is over the flow, or none outside it.
  pub over: Option<Drop>,
  /// Where in the card the handle was held.
  pub grab: (f32, f32),
}

impl DevtoolsInspectable for Drag {}

/// Where a dragged card would land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Drop {
  /// Between two places of the flow: just before this step, or at the end.
  Before(Option<StepKey>),
  /// On a card: the two run in parallel.
  OnCard(StepKey),
}

/// The last move that was refused, with the step it was made on, none for a step added at the end.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Refused {
  pub key: Option<StepKey>,
  pub why: Why,
  pub lasts: Lasts,
}

/// Why a move was refused.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Why {
  /// The steps' rules do not take it.
  Rule(MoveError),
  /// Alt ↑ on a step nothing runs before.
  NothingBefore,
  /// Alt ↓ on a step nothing runs after.
  NothingAfter,
}

/// How long a refusal under its card is said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lasts {
  /// While the pointer or the keys are on the card: a move from its menu or the keys, made where the person looks.
  OnCard,
  /// Until the next change: a drop, let go over a place away from the card, which would otherwise leave nothing said
  /// once the pointer is elsewhere.
  NextChange,
}

impl DevtoolsInspectable for Refused {}

/// A stage of the flow as laid out: its cards, in flow order.
pub(super) struct Stage {
  pub cards: Vec<(StepKey, ElementRef)>,
}

#[derive(Clone)]
pub(super) struct Reorder {
  /// The card the pointer is on, and the card the keys are on.
  pub hovered: Signal<Held>,
  pub focused: Signal<Held>,
  /// The card whose menu is, or was last, opened, and whether it is open.
  pub menu_for: Signal<Held>,
  pub menu_open: Signal<bool>,
  pub drag: Signal<Option<Drag>>,
  /// Where the pointer is in the window while a card is lifted. Only the floating card reads it, so the flow is drawn
  /// again only when what the drag is over changes.
  pub pointer: Signal<(f32, f32)>,
  pub refused: Signal<Option<Refused>>,
  /// A drag that Esc set down: it ends without a drop.
  pub set_down: Arc<Mutex<bool>>,
  /// The card the keys move, which gets the focus back once the flow is drawn again.
  pub refocus: Arc<Mutex<Option<StepKey>>>,
  /// A save is under way or a question about it is open: the flow changes nothing, whatever a handler made before the
  /// lock says. Set by every draw.
  pub locked: Arc<Mutex<bool>>,
  /// The flow's own element, for where the floating card is drawn.
  pub frame: ElementRef,
  /// The edit the flow was last drawn from: a refusal that lasts until the next change goes once it differs.
  drawn: Arc<Mutex<Option<PipelineEdit>>>,
  stages: Arc<Mutex<Vec<Stage>>>,
  anchors: Arc<Mutex<Vec<(StepKey, ElementRef)>>>,
}

impl Reorder {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(Held::default()),
      focused: ctx.signal(Held::default()),
      menu_for: ctx.signal(Held::default()),
      menu_open: ctx.signal(false),
      drag: ctx.signal(None),
      pointer: ctx.signal((0.0, 0.0)),
      refused: ctx.signal(None),
      set_down: Arc::default(),
      refocus: Arc::default(),
      locked: Arc::default(),
      frame: ElementRef::default(),
      drawn: Arc::default(),
      stages: Arc::default(),
      anchors: Arc::default(),
    }
  }

  /// A refusal of a move from a card's menu or the keys says why under the card, while the pointer or the keys are on
  /// that card: once neither is, it goes. A refused drop, and one that is not about a card (a step added at the end),
  /// stay until the next change.
  pub(super) fn settle_refusal(&self) {
    let Some(Refused {
      key: Some(key),
      lasts: Lasts::OnCard,
      ..
    }) = self.refused.get_untracked()
    else {
      return;
    };
    if !self.hovered.get_untracked().is(&key) && !self.focused.get_untracked().is(&key) {
      self.refused.set(None);
    }
  }

  /// Notes that the flow is drawn from `current`: a refused drop is said until the edit changes in any way, a move,
  /// *Cancel* or a save, since a refused move changes nothing.
  pub(super) fn drawn_from(&self, current: &PipelineEdit) {
    let mut drawn = self.drawn.lock();
    if drawn.as_ref() == Some(current) {
      return;
    }
    let changed = drawn.is_some();
    *drawn = Some(current.clone());
    drop(drawn);
    let lasting = matches!(
      self.refused.get_untracked(),
      Some(Refused {
        lasts: Lasts::NextChange,
        ..
      })
    );
    if changed && lasting {
      self.refused.set(None);
    }
  }

  /// Forgets where the cards stood: a draw lays them out again.
  pub(super) fn clear_layout(&self) {
    self.stages.lock().clear();
    self.anchors.lock().clear();
  }

  /// Notes the stage whose cards are `keys`, and gives their elements, in flow order.
  pub(super) fn stage(&self, keys: impl IntoIterator<Item = StepKey>) -> Vec<(StepKey, ElementRef)> {
    let cards: Vec<(StepKey, ElementRef)> = keys.into_iter().map(|key| (key, ElementRef::default())).collect();
    self.stages.lock().push(Stage { cards: cards.clone() });
    cards
  }

  /// The element of a step's ⋯ button, made the first time it is asked for in a draw.
  pub(super) fn more_anchor(&self, key: &StepKey) -> ElementRef {
    let mut anchors = self.anchors.lock();
    if let Some((_, more)) = anchors.iter().find(|(held, _)| held == key) {
      return more.clone();
    }
    let more = ElementRef::default();
    anchors.push((key.clone(), more.clone()));
    more
  }

  /// The element of a step's card, if the draw laid it out.
  pub(super) fn card_element(&self, key: &StepKey) -> Option<ElementRef> {
    self
      .stages
      .lock()
      .iter()
      .flat_map(|stage| &stage.cards)
      .find(|(held, _)| held == key)
      .map(|(_, element)| element.clone())
  }

  /// Where the window point (`x`, `y`) would drop a card, from where the cards stand: above or below a card's middle
  /// band between two places, on its middle band onto the card, and outside the flow nowhere.
  pub(super) fn drop_at(&self, (x, y): (f32, f32)) -> Option<Drop> {
    let frame = self.frame.bounds();
    let outside = x < frame.x || x > frame.x + frame.width || y < frame.y || y > frame.y + frame.height;
    if !self.frame.is_attached() || outside {
      return None;
    }
    let stages = self.stages.lock();
    let laid: Vec<Vec<(&StepKey, ElementRect)>> = stages
      .iter()
      .map(|stage| {
        stage
          .cards
          .iter()
          .filter(|(_, element)| element.is_attached())
          .map(|(key, element)| (key, element.bounds()))
          .collect::<Vec<_>>()
      })
      .filter(|cards| !cards.is_empty())
      .collect();
    let first_of = |index: usize| {
      laid
        .get(index)
        .and_then(|cards| cards.first())
        .map(|(key, _)| (*key).clone())
    };
    for (index, cards) in laid.iter().enumerate() {
      let top = cards.iter().map(|(_, bounds)| bounds.y).fold(f32::MAX, f32::min);
      let bottom = cards
        .iter()
        .map(|(_, bounds)| bounds.y + bounds.height)
        .fold(f32::MIN, f32::max);
      if y < top {
        return Some(Drop::Before(first_of(index)));
      }
      if y > bottom {
        continue;
      }
      let band = cards.iter().map(|(_, bounds)| bounds.height).fold(f32::MAX, f32::min) * MIDDLE_EDGE;
      if y < top + band {
        return Some(Drop::Before(first_of(index)));
      }
      if y > bottom - band {
        return Some(Drop::Before(first_of(index + 1)));
      }
      return cards
        .iter()
        .find(|(_, bounds)| {
          x >= bounds.x && x <= bounds.x + bounds.width && y >= bounds.y && y <= bounds.y + bounds.height
        })
        .map(|(key, _)| Drop::OnCard((*key).clone()));
    }
    Some(Drop::Before(None))
  }
}

/// The share of a card's height at its top and at its bottom that is the place before or after it, not the card.
const MIDDLE_EDGE: f32 = 0.3;
