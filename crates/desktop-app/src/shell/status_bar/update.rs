//! The status bar's update item (`Row 2 · status bar` `VFWHJ` of `Auto-updater · redesign` `acFkz`): it shows only
//! while the update has news, never with a card. Downloading, quiet in `text_muted` with `download` (S1 `t99e4v`), or
//! paused there; ready, *Update 0.4.0 ready · Restart* in `info` with `circle-arrow-up` (S2 `RccSH`); waiting for
//! the runs at work, *Update 0.4.0 · after 3 runs end* with `hourglass` (S3 `UCZt3`); a critical release, *Critical
//! update 0.4.1 · Restart* in `warning` with `shield-alert` (S4 `y550R`); a download that failed its check, *Update
//! 0.4.0 failed · signature didn't match* in `danger` with `circle-x` (S5 `D0bg7M`), or an install that did not
//! finish; installing; and for the first minutes after the update, *Updated to 0.4.0 · What's new* in `success` with
//! `circle-check` (S6 `wZaZb`). A press opens Settings › Updates, where the update's row says the rest and offers
//! what to do; after an update it opens its release notes there.

use std::time::{Duration, Instant};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, StreamEmitter},
    theme::PaletteColor,
  },
  node::Element,
};
use orchester_workspace::app_updates::{
  AppUpdates, UpdateFailure, UpdateState,
  install::{InstallStep, JustUpdated},
};

use super::{
  facts::{Dispatch, go},
  item::{glyph, label, pressed},
};
use crate::{
  routes::{Route, UpdatesShow},
  ui::icon::Icon,
};

/// How long *Updated to* shows after the update's first start.
const UPDATED_FOR: Duration = Duration::from_secs(10 * 60);

/// What the item shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateFacts {
  /// Nothing the person needs to know: no item.
  #[default]
  Quiet,
  /// `version` downloads, `percent` of it so far.
  Downloading { version: String, percent: u8 },
  /// The person paused the download of `version` at `percent`.
  Paused { version: String, percent: u8 },
  /// A verified update is ready; no run is at work.
  Ready { version: String },
  /// It waits for `runs` runs at work to end.
  Waiting { version: String, runs: usize },
  /// A ready release fixes a security problem.
  Critical { version: String },
  /// The runners stop, the databases are copied aside and the installer takes over.
  Installing { version: String },
  /// This start finished the update to `version`.
  Updated { version: String },
  /// The download of `version` failed its check, or could not be made at all.
  Failed { version: String, failure: UpdateFailure },
  /// `version` was ready but did not install.
  NotInstalled { version: String },
}

impl DevtoolsInspectable for UpdateFacts {}

impl UpdateFacts {
  /// The item for `state`, with `updated` while *Updated to* shows.
  pub fn of(state: &UpdateState, updated: Option<&JustUpdated>) -> Self {
    if let Some(update) = state.to_install().filter(|update| update.critical) {
      return Self::Critical {
        version: update.version.to_string(),
      };
    }
    match state {
      UpdateState::Downloading { version, .. } => Self::Downloading {
        version: version.to_string(),
        percent: state.percent().unwrap_or_default(),
      },
      UpdateState::Paused { version, .. } => Self::Paused {
        version: version.to_string(),
        percent: state.percent().unwrap_or_default(),
      },
      UpdateState::Ready(update) => Self::Ready {
        version: update.version.to_string(),
      },
      UpdateState::WaitingForRuns { update, runs }
      | UpdateState::Installing {
        update,
        step: InstallStep::WaitingForRuns(runs),
      } => Self::Waiting {
        version: update.version.to_string(),
        runs: runs.len(),
      },
      UpdateState::Installing { update, .. } => Self::Installing {
        version: update.version.to_string(),
      },
      UpdateState::Failed { version, failure } => Self::Failed {
        version: version.to_string(),
        failure: *failure,
      },
      UpdateState::InstallFailed { version, .. } => Self::NotInstalled {
        version: version.to_string(),
      },
      _ => updated.map_or(Self::Quiet, |updated| Self::Updated {
        version: updated.version.to_string(),
      }),
    }
  }
}

/// The item's facts, whenever the update's state changes, with *Updated to* for its first minutes.
pub fn update_facts(ctx: &mut Ctx, updates: &AppUpdates) -> UpdateFacts {
  ctx
    .stream(
      updates.clone(),
      |updates, emitter: StreamEmitter<UpdateFacts, ()>| async move {
        let until = Instant::now() + UPDATED_FOR;
        let mut changes = updates.subscribe();
        loop {
          let fresh = Instant::now() < until;
          let state = changes.borrow_and_update().clone();
          let facts = UpdateFacts::of(&state, updates.just_updated().filter(|_| fresh).as_ref());
          if !emitter.emit(facts) {
            return;
          }
          tokio::select! {
            changed = changes.changed() => if changed.is_err() { return },
            () = tokio::time::sleep_until(until.into()), if fresh => {}
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The item, or nothing while the update has nothing to say. A press opens Settings › Updates; after an update, its
/// release notes there.
pub fn item_of(ctx: &mut Ctx, facts: &UpdateFacts, dispatch: &Dispatch) -> Option<Element> {
  let (state, glyph_of, color, words) = shown(ctx, facts)?;
  let show = if matches!(facts, UpdateFacts::Updated { .. }) {
    UpdatesShow::WhatsNew
  } else {
    UpdatesShow::Section
  };
  let item = pressed("status-update", go(dispatch, Route::Updates(show)))
    .describe("state", state)
    .describe("a11y_name", words.clone())
    .child(glyph(glyph_of, color.clone()))
    .child(label(&words, color).id("status-update-text"));
  Some(item.into())
}

/// The item's state name, glyph, color and words.
fn shown(ctx: &Ctx, facts: &UpdateFacts) -> Option<(&'static str, Icon, PaletteColor, String)> {
  let words = |key: &str, version: &str| ctx.t_args(key, [("version", version.to_owned())]).to_string();
  let percent = |key: &str, version: &str, percent: u8| {
    ctx
      .t_args(key, [("version", version.to_owned()), ("percent", percent.to_string())])
      .to_string()
  };
  Some(match facts {
    UpdateFacts::Quiet => return None,
    UpdateFacts::Downloading { version, percent: done } => (
      "downloading",
      Icon::Download,
      PaletteColor::TextMuted,
      percent("shell.status.update.downloading", version, *done),
    ),
    UpdateFacts::Paused { version, percent: done } => (
      "paused",
      Icon::Pause,
      PaletteColor::TextMuted,
      percent("shell.status.update.paused", version, *done),
    ),
    UpdateFacts::Ready { version } => (
      "ready",
      Icon::CircleArrowUp,
      PaletteColor::Info,
      words("shell.status.update.ready", version),
    ),
    UpdateFacts::Waiting { version, runs } => {
      let key = if *runs == 1 {
        "shell.status.update.waiting.one"
      } else {
        "shell.status.update.waiting.other"
      };
      let words = ctx.t_args(key, [("version", version.clone()), ("count", runs.to_string())]);
      ("waiting", Icon::Hourglass, PaletteColor::Info, words.to_string())
    }
    UpdateFacts::Critical { version } => (
      "critical",
      Icon::ShieldAlert,
      PaletteColor::Warning,
      words("shell.status.update.critical", version),
    ),
    UpdateFacts::Installing { version } => (
      "installing",
      Icon::LoaderCircle,
      PaletteColor::Info,
      words("shell.status.update.installing", version),
    ),
    UpdateFacts::Updated { version } => (
      "updated",
      Icon::CircleCheck,
      PaletteColor::Success,
      words("shell.status.update.updated", version),
    ),
    UpdateFacts::Failed { version, failure } => {
      let why = ctx.t(why_key(*failure));
      let words = ctx.t_args(
        "shell.status.update.failed",
        [("version", version.clone()), ("why", why.to_string())],
      );
      ("failed", Icon::CircleX, PaletteColor::Danger, words.to_string())
    }
    UpdateFacts::NotInstalled { version } => (
      "not-installed",
      Icon::CircleX,
      PaletteColor::Danger,
      words("shell.status.update.notInstalled", version),
    ),
  })
}

/// Why a download failed, in the few words the bar has room for.
const fn why_key(failure: UpdateFailure) -> &'static str {
  match failure {
    UpdateFailure::NoInstaller => "shell.status.update.why.noInstaller",
    UpdateFailure::WrongSize => "shell.status.update.why.wrongSize",
    UpdateFailure::WrongDigest => "shell.status.update.why.wrongDigest",
    UpdateFailure::UnknownKey => "shell.status.update.why.unknownKey",
    UpdateFailure::BadSignature => "shell.status.update.why.badSignature",
    UpdateFailure::NotSaved => "shell.status.update.why.notSaved",
  }
}
