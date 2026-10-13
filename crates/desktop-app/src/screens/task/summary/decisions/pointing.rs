//! A decision card a navigation pointed at (`The highlight` in `icGHG`): the page scrolls so the whole card
//! is in view and rings it in a 2 `border_focus` stroke outside its edge for 2 s, then the card wears its own
//! stroke again (`Plan review` `iFzXa` in `JY0xW`, `Raise the limit` `Yg95b` in `krC4n`).

use std::time::Duration;

use lurq::{
  app::{
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::border::Border,
};

use crate::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
  },
  screens::task::TaskContext,
};

/// How long the ring stays.
const RING: Duration = Duration::from_secs(2);

/// What a decision component keeps to ring its card.
pub struct Pointing {
  card: ElementRef,
  ringed: Signal<bool>,
  fade: Timeout,
}

impl Pointing {
  pub fn new(ctx: &mut Ctx) -> Self {
    let ringed = ctx.signal(false);
    let off = ringed.clone();
    let fade = ctx.create_timeout(RING, move || off.set(false));
    Self {
      card: ElementRef::new(),
      ringed,
      fade,
    }
  }

  /// Takes the navigation's pointer when it points at `card` of this task: the card is brought into view
  /// and ringed. `true` on the render it arrives at.
  pub fn arrived(&self, ctx: &mut Ctx, props: &TaskContext, card: PointedCard) -> bool {
    let here = Pointed {
      project: props.project.open.project.id(),
      task: props.key.clone(),
      card,
    };
    if !Navigation::of(ctx).take_pointed(&here) {
      return false;
    }
    self.ringed.set(true);
    self.fade.restart();
    ctx.scroll_into_view(&self.card);
    true
  }

  /// `card`, found by the pointer, and ringed while it is: then it has the class `pointed`.
  pub fn ring(&self, card: Column) -> Column {
    let card = card.ref_element(self.card.clone());
    if !self.ringed.get() {
      return card;
    }
    card
      .border(Border::outside(BorderSize::Md, PaletteColor::BorderFocus))
      .class("pointed")
  }
}
