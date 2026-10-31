//! *Updates* in Settings › General, live (`crate::updates` draws it): where this copy's update stands as
//! `AppUpdates` follows it, the channel and *Update automatically* as stored, read again after each save and each
//! check, and what its presses do. A setting saves as it is chosen, off the UI thread, and the header's save state
//! follows it; the choice shows while it saves. *Check now* and *Resume* run on the app's runtime; *Restart to update*
//! installs at once or asks about the runs at work (`shell::update`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use orchester_workspace::{
  app_updates::{AppUpdates, UpdateState},
  background,
  messages::{self, Message},
};

use crate::{
  routes::UpdatesShow,
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  shell::update::Restart,
  ui::message::message_text,
  updates::{SectionProps, UpdateAction, UpdateDispatch, UpdatesFacts, UpdatesSection},
};

const ID: &str = "settings-updates";

/// A setting the person changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
  Channel(Channel),
  Automatic(bool),
}

impl DevtoolsInspectable for Change {}

impl Change {
  fn applied(self, settings: UpdateSettings) -> UpdateSettings {
    match self {
      Self::Channel(channel) => UpdateSettings { channel, ..settings },
      Self::Automatic(automatic) => UpdateSettings { automatic, ..settings },
    }
  }
}

type SaveAction = FutureAction<Change, (), Message>;

/// The settings and when the cloud last answered, as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
  settings: UpdateSettings,
  last_checked: Option<Timestamp>,
}

impl DevtoolsInspectable for Stored {}

/// The update's state, and how often what is stored may have changed: a save, or a state of another kind, such as a
/// check's end. A download's progress is not one: it would read the database with every chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Followed {
  state: UpdateState,
  stored_changes: u64,
}

impl DevtoolsInspectable for Followed {}

/// What the route asks *Updates* to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown(pub Option<UpdatesShow>);

impl DevtoolsInspectable for Shown {}

pub struct UpdatesGroup {
  /// The change being saved, shown until it is.
  chosen: Signal<Option<Change>>,
  /// A page or a log that could not be opened.
  unopened: Signal<Option<String>>,
}

impl Component for UpdatesGroup {
  type Props = Shown;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(None),
      unopened: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Shown(show) = *ctx.props::<Shown>();
    let services = Services::of(ctx);
    // Hooks in the same order on every render: the action, the stream, then the read.
    let save = save_action(ctx, &services.updates);
    let saving = save.state().get();
    let followed = follow(ctx, &services.updates);
    let stored = stored(ctx, &services, followed.stored_changes);
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&saving));
    }
    let chosen = self.chosen.get().filter(|_| saving.is_pending());
    let settings = stored
      .data
      .as_ref()
      .map(|stored| chosen.map_or(stored.settings, |change| change.applied(stored.settings)));
    let facts = UpdatesFacts {
      running: services.updates.this_app().version.to_string(),
      state: followed.state,
      just_updated: services.updates.just_updated().cloned(),
      settings,
      last_checked: stored.data.as_ref().map(|stored| stored.last_checked),
      problem: self.problem(ctx, (&stored, &saving)),
      show,
    };
    let dispatch = self.dispatch(ctx, &services, save);
    ctx.mount_keyed::<UpdatesSection>("settings-updates-section", SectionProps { facts, dispatch })
  }
}

impl UpdatesGroup {
  /// What the section's presses do in this window.
  fn dispatch(&self, ctx: &mut Ctx, services: &Services, save: SaveAction) -> UpdateDispatch {
    let restart = Restart::of(ctx);
    let (updates, runtime, links) = (
      services.updates.clone(),
      services.runtime.clone(),
      services.links.clone(),
    );
    let (chosen, unopened) = (self.chosen.clone(), self.unopened.clone());
    Arc::new(move |action| match action {
      UpdateAction::CheckNow => {
        let updates = updates.clone();
        runtime.spawn(async move { updates.check_now().await });
      }
      UpdateAction::Pause => updates.pause(),
      UpdateAction::Resume => {
        let updates = updates.clone();
        runtime.spawn(async move { updates.resume().await });
      }
      UpdateAction::Restart => restart.press(),
      UpdateAction::Channel(channel) => {
        chosen.set(Some(Change::Channel(channel)));
        save.run(Change::Channel(channel));
      }
      UpdateAction::Automatic(automatic) => {
        chosen.set(Some(Change::Automatic(automatic)));
        save.run(Change::Automatic(automatic));
      }
      UpdateAction::Open(url) => match links.open(&url) {
        Ok(()) => unopened.set(None),
        Err(error) => {
          tracing::warn!(%error, url, "a page of the update could not be opened");
          unopened.set(Some(url));
        }
      },
      UpdateAction::OpenLog(log) => match orchester_exec::external::open_path(&log) {
        Ok(()) => unopened.set(None),
        Err(error) => {
          tracing::warn!(%error, log = %log.display(), "the installer's log could not be opened");
          unopened.set(Some(log.display().to_string()));
        }
      },
    })
  }

  /// Why the settings can't be read or a change was not saved, or what could not be opened.
  fn problem(
    &self,
    ctx: &Ctx,
    (stored, saving): (&FutureState<Stored, Message>, &FutureState<(), Message>),
  ) -> Option<String> {
    let (key, reason) = match (&stored.error, &saving.error) {
      (Some(reason), _) => ("settings.general.updates.unreadable", reason),
      (None, Some(reason)) if !saving.is_pending() => ("settings.general.updates.notSaved", reason),
      _ => {
        let url = self.unopened.get()?;
        return Some(
          ctx
            .t_args("settings.general.updates.notOpened", [("url", url)])
            .to_string(),
        );
      }
    };
    let reason = message_text(ctx, reason);
    Some(ctx.t_args(key, [("reason", &*reason)]).to_string())
  }
}

/// Saves the person's change.
fn save_action(ctx: &mut Ctx, updates: &AppUpdates) -> SaveAction {
  let updates = updates.clone();
  ctx.future_action(move |change: Change| {
    let updates = updates.clone();
    async move {
      background::run(move || match change {
        Change::Channel(channel) => updates.set_channel(channel),
        Change::Automatic(automatic) => updates.set_automatic(automatic),
      })
      .await?
      .map_err(|error| {
        tracing::error!(%error, "an update setting was not saved");
        messages::store_error(&error)
      })
    }
  })
}

/// The update's state whenever it changes, counting the saves and the changes of its kind.
fn follow(ctx: &mut Ctx, updates: &AppUpdates) -> Followed {
  ctx
    .stream(
      updates.clone(),
      |updates, emitter: StreamEmitter<Followed, ()>| async move {
        let (mut states, mut saved) = (updates.subscribe(), updates.settings_saved());
        let mut stored_changes = 0;
        let mut last_kind = kind(&states.borrow());
        loop {
          let state = states.borrow_and_update().clone();
          if last_kind != kind(&state) {
            last_kind = kind(&state);
            stored_changes += 1;
          }
          if !emitter.emit(Followed {
            state,
            stored_changes: stored_changes + *saved.borrow_and_update(),
          }) {
            return;
          }
          tokio::select! {
            changed = states.changed() => if changed.is_err() { return },
            changed = saved.changed() => if changed.is_err() { return },
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| Followed {
      state: updates.state(),
      stored_changes: 0,
    })
}

/// Which kind of state it is, whatever it carries.
fn kind(state: &UpdateState) -> std::mem::Discriminant<UpdateState> {
  std::mem::discriminant(state)
}

/// The settings and the last check as stored, read when the group shows and again after `changes`.
fn stored(ctx: &mut Ctx, services: &Services, changes: u64) -> FutureState<Stored, Message> {
  let (updates, hold) = (services.updates.clone(), services.hold.clone());
  ctx
    .future(changes, move |_| {
      let (updates, hold) = (updates.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || {
          Ok::<_, orchester_store::StoreError>(Stored {
            settings: updates.settings()?,
            last_checked: updates.last_checked()?,
          })
        })
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the update settings cannot be read");
          messages::store_error(&error)
        })
      }
    })
    .state()
    .get()
}

/// Counts the update settings saved, so what shows them reads them again.
pub(super) fn settings_saved(ctx: &mut Ctx, updates: &AppUpdates) -> u64 {
  ctx
    .stream(updates.clone(), |updates, emitter: StreamEmitter<u64, ()>| async move {
      let mut saved = updates.settings_saved();
      loop {
        let count = *saved.borrow_and_update();
        if !emitter.emit(count) || saved.changed().await.is_err() {
          return;
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}
