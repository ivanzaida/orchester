//! What the frame's notice says of the run control the runner last refused. It stays while another
//! request is under way, so it does not flicker away and back, and goes once a request is carried out or
//! the run it was about moved on: another run, or the same run in another state.

use lurq::{
  app::ctx::{Ctx, FutureState},
  core::Signal,
};
use orchester_core::{id::RunId, run::RunStatus};
use orchester_workspace::{messages::Message, tasks::detail::TaskDetail};

use super::controls::{Requested, RunControl};

/// Where the task's run stood when a request was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RunMark(Option<(RunId, RunStatus)>);

impl RunMark {
  fn of(detail: &TaskDetail) -> Self {
    Self(detail.run.as_ref().map(|run| (run.id(), run.run.status().clone())))
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
  control: RunControl,
  reason: Message,
  mark: RunMark,
}

impl lurq::app::component::DevtoolsInspectable for Refusal {}

/// The last refusal, and the request whose outcome it holds.
pub struct Refusals {
  last: Signal<Option<Refusal>>,
  /// The serial of the request whose outcome was taken in.
  taken: Signal<u64>,
}

impl Refusals {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      last: ctx.signal(None),
      taken: ctx.signal(0),
    }
  }

  /// The refusal the notice shows for `detail`, after taking in how `requested` ended (`state`).
  pub fn shown(
    &self,
    state: &FutureState<RunControl, Message>,
    requested: Option<Requested>,
    detail: &TaskDetail,
  ) -> Option<(RunControl, Message)> {
    let mark = RunMark::of(detail);
    if let Some(requested) =
      requested.filter(|requested| !state.is_pending() && requested.serial != self.taken.get_untracked())
    {
      self.taken.set(requested.serial);
      let refusal = state.error.clone().map(|reason| Refusal {
        control: requested.control,
        reason,
        mark: mark.clone(),
      });
      self.last.set(refusal);
    }
    self
      .last
      .get()
      .filter(|refusal| refusal.mark == mark)
      .map(|refusal| (refusal.control, refusal.reason))
  }
}
