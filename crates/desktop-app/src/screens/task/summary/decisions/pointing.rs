//! A decision card a navigation pointed at (`The highlight` in `icGHG`): the page scrolls so the whole card
//! is in view and rings it in a 2 `border_focus` stroke outside its edge for 2 s, then the card wears its own
//! stroke again (`Plan review` `iFzXa` in `JY0xW`, `Raise the limit` `Yg95b` in `krC4n`).

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

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

/// What a decision component keeps to ring its card. The render the pointer arrives at rings the card without
/// writing a signal; the fade, a timer's handler, says when the ring goes.
pub struct Pointing {
  card: ElementRef,
  ringed: Arc<AtomicBool>,
  /// Moves on when the ring fades, so the card renders without it.
  faded: Signal<u64>,
  fade: Timeout,
}

impl Pointing {
  pub fn new(ctx: &mut Ctx) -> Self {
    let ringed = Arc::new(AtomicBool::new(false));
    let faded = ctx.signal(0);
    let (off, gone) = (ringed.clone(), faded.clone());
    let fade = ctx.create_timeout(RING, move || {
      off.store(false, Ordering::Relaxed);
      gone.update(|faded| *faded += 1);
    });
    Self {
      card: ElementRef::new(),
      ringed,
      faded,
      fade,
    }
  }

  /// Takes the navigation's pointer when it points at `card` of this task: the card is brought into view
  /// and ringed in this render. `true` on the render it arrives at.
  pub fn arrived(&self, ctx: &mut Ctx, props: &TaskContext, card: PointedCard) -> bool {
    let here = Pointed {
      project: props.project.open.project.id(),
      task: props.key.clone(),
      card,
    };
    if !Navigation::of(ctx).take_pointed(&here) {
      return false;
    }
    self.ringed.store(true, Ordering::Relaxed);
    self.fade.restart();
    ctx.scroll_into_view(&self.card);
    true
  }

  /// `card`, found by the pointer, and ringed while it is: then it has the class `pointed`.
  pub fn ring(&self, card: Column) -> Column {
    let card = card.ref_element(self.card.clone());
    self.faded.get();
    if !self.ringed.load(Ordering::Relaxed) {
      return card;
    }
    card
      .border(Border::outside(BorderSize::Md, PaletteColor::BorderFocus))
      .class("pointed")
  }
}
