//! The OS notification that this copy's update is ready (`Auto-updater · redesign` `acFkz`), among the app's other
//! notifications: once per version, also across restarts, as the application database's window state keeps the
//! version told ([`TOLD`]). A critical release says so. Its click opens Settings › Updates.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, StreamEmitter},
};
use orchester_workspace::{
  app_updates::{AppUpdates, UpdateState},
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use super::os::{self, Delivery, Shown};
use crate::{
  routes::{Route, UpdatesShow},
  services::Services,
};

/// Where the window state keeps the version the person was told is ready.
pub const TOLD: &str = "orchester-app.updates.told";

/// A ready update the person was not told of yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadyToTell {
  pub version: String,
  pub critical: bool,
}

/// What to tell of `state`: an update downloaded and checked, unless `told` names its version already.
pub fn to_tell(state: &UpdateState, told: Option<&str>) -> Option<ReadyToTell> {
  let update = state.ready()?;
  let version = update.version.to_string();
  (told != Some(version.as_str())).then_some(ReadyToTell {
    version,
    critical: update.critical,
  })
}

/// The version told, once read from the window state.
#[derive(Debug, Default)]
pub(super) struct Told {
  read: bool,
  version: Option<String>,
}

/// The version told as stored; none when the window state cannot be read, which is logged.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored(Option<String>);

impl DevtoolsInspectable for Stored {}

/// Tells the person of a ready update they were not told of, through the system as `delivery` allows.
pub(super) fn tell(ctx: &mut Ctx, delivery: Delivery, told: &Mutex<Told>) {
  let services = Services::of(ctx);
  let state = followed(ctx, &services.updates);
  let workspace = services.workspace.clone();
  let stored = ctx
    .future((), move |()| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.ui_state(TOLD))
          .await?
          .map(Stored)
          .map_err(|error| -> Message {
            tracing::warn!(%error, "which update the person was told of cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get();
  let mut told = told.lock();
  if !told.read {
    if stored.is_pending() {
      return;
    }
    told.read = true;
    told.version = stored.data.and_then(|stored| stored.0);
  }
  let Some(ready) = to_tell(&state, told.version.as_deref()) else {
    return;
  };
  told.version = Some(ready.version.clone());
  if delivery == Delivery::Silent {
    tracing::debug!(
      version = ready.version,
      "an update is ready; this build shows no notification"
    );
    return;
  }
  let (title, body) = if ready.critical {
    ("shell.update.criticalTitle", "shell.update.criticalBody")
  } else {
    ("shell.update.readyTitle", "shell.update.readyBody")
  };
  let shown = Shown {
    title: ctx.t_args(title, [("version", ready.version.clone())]).to_string(),
    body: ctx.t(body).to_string(),
    route: Route::Updates(UpdatesShow::Section).path(),
    sound: false,
  };
  if let Err(error) = os::show(&shown) {
    tracing::warn!(%error, "the system did not take the notification of a ready update");
    return;
  }
  let workspace = services.workspace;
  let version = ready.version;
  services.runtime.spawn(async move {
    let kept = background::run(move || workspace.set_ui_state(TOLD, &version)).await;
    if let Ok(Err(error)) = kept {
      tracing::warn!(%error, "which update the person was told of was not kept; a restart tells it again");
    }
  });
}

/// The update's state, whenever it changes.
fn followed(ctx: &mut Ctx, updates: &AppUpdates) -> UpdateState {
  ctx
    .stream(
      updates.clone(),
      |updates, emitter: StreamEmitter<UpdateState, ()>| async move {
        let mut changes = updates.subscribe();
        loop {
          let state = changes.borrow_and_update().clone();
          if !emitter.emit(state) || changes.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| updates.state())
}
