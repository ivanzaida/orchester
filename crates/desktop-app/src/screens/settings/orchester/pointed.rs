//! The invite row an `orchester://invite/…` link opened the column at (the route's `at`): the column scrolls so the
//! row is in view and rings it in a 2 `border_focus` stroke outside its edge for 2 s, as a decision card a navigation
//! points at is ringed (`The highlight` in `icGHG`). A row the cloud has not listed yet is scrolled to once it is.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::border::Border,
};
use orchester_core::id::InviteId;
use parking_lot::Mutex;

/// How long the ring stays.
const RING: Duration = Duration::from_secs(2);

/// The invite ringed, and whether its row is still to be scrolled to.
type Ringed = Arc<Mutex<Option<(InviteId, bool)>>>;

/// What the column keeps to ring the row a link opened it at. The render that scrolls to the row writes no signal;
/// the fade, a timer's handler, says when the ring goes.
pub(super) struct PointedInvite {
  row: ElementRef,
  /// The invite the route last opened at, so each opening rings once.
  followed: Mutex<Option<InviteId>>,
  ringed: Ringed,
  /// Moves on when the ring fades, so the row renders without it.
  faded: Signal<u64>,
  fade: Timeout,
}

impl PointedInvite {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let faded = ctx.signal(0);
    let ringed = Ringed::default();
    let (off, gone) = (ringed.clone(), faded.clone());
    let fade = ctx.create_timeout(RING, move || {
      off.lock().take();
      gone.update(|faded| *faded += 1);
    });
    Self {
      row: ElementRef::new(),
      followed: Mutex::new(None),
      ringed,
      faded,
      fade,
    }
  }

  /// Takes the route's `at`: a new invite there is ringed, and its row scrolled to once it shows.
  pub(super) fn follow(&self, at: Option<InviteId>) {
    let mut followed = self.followed.lock();
    if at != *followed
      && let Some(invite) = at
    {
      *self.ringed.lock() = Some((invite, true));
    }
    *followed = at;
  }

  /// `row`, the row of `invite`: scrolled into view on its first render after the link, and ringed until the ring
  /// fades, when it has the class `pointed`.
  pub(super) fn ring(&self, ctx: &mut Ctx, invite: InviteId, row: Column) -> Column {
    self.faded.get();
    let mut ringed = self.ringed.lock();
    let Some((_, scroll)) = ringed.as_mut().filter(|(pointed, _)| *pointed == invite) else {
      return row;
    };
    if std::mem::take(scroll) {
      ctx.scroll_into_view(&self.row);
      self.fade.restart();
    }
    row
      .ref_element(self.row.clone())
      .border(Border::outside(BorderSize::Md, PaletteColor::BorderFocus))
      .class("pointed")
  }
}
