//! The newest plan-limit reading of each account, as the session's runners hand them on ([`readings`], what the
//! status bar shows: this session's only), and with the last reading a project stored of each account where this
//! session has none newer ([`account_readings`], what an account's menus and details show of its use). Reading them
//! in a render renders again when one changes.

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

/// The last readings projects stored, one per account, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Stored(Vec<orchester_core::connection::plan_limits::LimitReading>);

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// [`readings`], with each account's last stored reading where this session has none newer.
pub fn account_readings(ctx: &mut Ctx) -> PlanReadings {
  let mut readings = readings(ctx);
  let workspace = Services::of(ctx).workspace;
  let stored = ctx
    .future((), move |()| {
      let workspace = workspace.clone();
      async move {
        orchester_workspace::background::run(move || workspace.last_plan_readings())
          .await?
          .map(Stored)
          .map_err(|error| error.message())
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default();
  for reading in stored.0 {
    // An older stored reading than this session's is not taken.
    readings.observe(reading);
  }
  readings
}
