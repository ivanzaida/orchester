//! The newest plan-limit reading of each account, as the session's runners hand them on (session-only, never read
//! back from storage); reading them in a render renders again when one changes.

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::connection::plan_limits::PlanReadings;
use orchester_workspace::plan_limits::Readings;

use super::Services;

pub fn readings(ctx: &mut Ctx) -> PlanReadings {
  let runners = Services::of(ctx).runners;
  let readings = ctx.stream(runners, |runners, emitter: StreamEmitter<Readings, ()>| async move {
    let mut updates = runners.plan_limits();
    loop {
      let readings = updates.borrow_and_update().clone();
      if !emitter.emit(Readings(readings)) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  readings.state().get().data.unwrap_or_default().0
}
