//! The line of a run Orchester tries to merge again by itself (09j `GhnHQ`, the *Retrying* row of `Cjprb`), in `info`:
//! *Retrying by itself · attempt N of 3 in Ns* and why the last try did not land. A component of its own with its own
//! interval, so its countdown renders one line a second and nothing else; the merge queue's retry, at no set time, does
//! not tick.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::PaletteColor,
  },
  core::Signal,
  node::Element,
};
use orchester_core::integration::attempt::MergeAsker;
use orchester_workspace::tasks::merge_attempts::{merges::NotMergedRun, read::Retrying};

use super::{row, words};

const TICK: Duration = Duration::from_secs(1);

/// The line of `run`, which Orchester tries again, in the row `id`.
pub(super) fn line(ctx: &mut Ctx, id: &str, run: &NotMergedRun, retrying: Retrying) -> Element {
  let attempt = &run.not_merged.attempt.attempt;
  ctx.mount_keyed::<Countdown>(
    &format!("{id}-retrying"),
    Next {
      id: id.to_owned(),
      last: attempt.asked_by(),
      retrying,
      cause: words::cause(ctx, attempt.cause()),
    },
  )
}

#[derive(Clone, Debug, PartialEq)]
struct Next {
  id: String,
  /// Who asked for the last try.
  last: MergeAsker,
  retrying: Retrying,
  cause: Arc<str>,
}

impl DevtoolsInspectable for Next {}

struct Countdown {
  tick: Signal<u64>,
  interval: Interval,
}

impl Component for Countdown {
  type Props = Next;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self { tick, interval }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Next {
      id,
      last,
      retrying,
      cause,
    } = ctx.props::<Next>().clone();
    // Read so every tick renders the line again.
    self.tick.get();
    if retrying.next.is_some() {
      self.interval.start();
    } else {
      self.interval.stop();
    }
    row::line(
      &id,
      &words::retrying_line(ctx, last, retrying, cause),
      PaletteColor::Info,
    )
  }
}
