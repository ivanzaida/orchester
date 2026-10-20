//! The newest plan-limit reading of each account, as the session's runners and reads hand them on ([`readings`],
//! what the status bar shows: this session's only), and with the last reading kept of each account where this
//! session has none newer ([`account_readings`], what an account's menus and details show of its use). Reading them
//! in a render renders again when one changes. [`shown`] marks the status bar's hint or the Usage page as shown, so
//! the accounts are read while it is.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  node::Element,
};
use orchester_core::connection::plan_limits::PlanReadings;
use orchester_workspace::plan_limits::{Readings, reads::Shown};

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

/// The last readings kept, one per account, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Stored(PlanReadings);

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// [`readings`], with each account's last kept reading where this session has none newer.
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
  for reading in stored.0.all() {
    if let Some(source) = reading.account_id().and_then(|account| stored.0.source_of(account)) {
      // An older kept reading than this session's is not taken.
      readings.observe_from(reading.clone(), source);
    }
  }
  readings
}

/// Marks what mounts it as shown while it is mounted: the accounts are read when it opens and every few minutes
/// while it stays (`orchester_workspace::plan_limits::reads`). It draws nothing. A view mounted without the session's
/// services (a test handing the status bar its state) has nothing to read.
pub fn shown(ctx: &mut Ctx, key: &str) -> Element {
  ctx.mount_keyed::<ShownMark>(key, ())
}

struct ShownMark {
  _shown: Option<Shown>,
}

impl Component for ShownMark {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      _shown: ctx
        .use_context::<Services>()
        .map(|services| services.plan_reads.shown()),
    }
  }

  fn render(&self, _ctx: &mut Ctx) -> impl Into<Element> {
    Element::new()
  }
}
