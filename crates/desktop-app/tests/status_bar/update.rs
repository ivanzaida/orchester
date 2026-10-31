//! The update item (`Row 2 · status bar` `VFWHJ` of `Auto-updater · redesign` `acFkz`): downloading (S1 `t99e4v`),
//! ready (S2 `RccSH`), waiting for runs (S3 `UCZt3`), critical (S4 `y550R`), failed (S5 `D0bg7M`) and updated (S6
//! `wZaZb`), each in its color with its glyph and no card; a press opens Settings › Updates, or after an update its
//! release notes.
//!
//! `cargo test -p orchester-app --test status_bar app_updates_ui`

use orchester_app::routes::UpdatesShow;
use orchester_workspace::app_updates::UpdateFailure;

use super::*;

fn with(update: UpdateFacts) -> Bar {
  Bar::of(BarFacts { update, ..facts(None) })
}

fn version() -> String {
  "0.4.0".to_owned()
}

/// What the item says and where its press goes.
fn shown(update: UpdateFacts) -> (Vec<String>, Vec<BarAction>) {
  let mut bar = with(update);
  let words = bar.texts("status-update").expect("the item shows");
  bar.press("status-update");
  (words, bar.asked())
}

fn opens(show: UpdatesShow) -> Vec<BarAction> {
  vec![BarAction::Go(Route::Updates(show), OpenIn::ActiveTab)]
}

#[test]
fn app_updates_ui_nothing_to_say_shows_no_item() {
  let mut bar = with(UpdateFacts::Quiet);
  assert_eq!(bar.texts("status-update"), None);
}

#[test]
fn app_updates_ui_status_bar_says_each_state_and_opens_settings_updates() {
  let cases = [
    (
      UpdateFacts::Downloading {
        version: version(),
        percent: 42,
      },
      "Update 0.4.0 · downloading 42%",
      "downloading",
    ),
    (
      UpdateFacts::Ready { version: version() },
      "Update 0.4.0 ready · Restart",
      "ready",
    ),
    (
      UpdateFacts::Waiting {
        version: version(),
        runs: 3,
      },
      "Update 0.4.0 · after 3 runs end",
      "waiting",
    ),
    (
      UpdateFacts::Waiting {
        version: version(),
        runs: 1,
      },
      "Update 0.4.0 · after 1 run ends",
      "waiting",
    ),
    (
      UpdateFacts::Critical {
        version: "0.4.1".to_owned(),
      },
      "Critical update 0.4.1 · Restart",
      "critical",
    ),
    (
      UpdateFacts::Failed {
        version: version(),
        failure: UpdateFailure::BadSignature,
      },
      "Update 0.4.0 failed · signature didn't match",
      "failed",
    ),
    (
      UpdateFacts::NotInstalled { version: version() },
      "Update 0.4.0 didn't install",
      "not-installed",
    ),
    (
      UpdateFacts::Installing { version: version() },
      "Installing 0.4.0",
      "installing",
    ),
  ];
  for (facts, words, state) in cases {
    let (said, asked) = shown(facts);
    assert_eq!(said, [words.to_owned()], "{state}");
    assert_eq!(asked, opens(UpdatesShow::Section), "{state} opens Settings › Updates");
  }
}

#[test]
fn app_updates_ui_after_an_update_the_item_opens_whats_new() {
  let (said, asked) = shown(UpdateFacts::Updated { version: version() });
  assert_eq!(said, ["Updated to 0.4.0 · What's new".to_owned()]);
  assert_eq!(asked, opens(UpdatesShow::WhatsNew));
}

#[test]
fn app_updates_ui_no_item_has_a_card() {
  let mut bar = with(UpdateFacts::Failed {
    version: version(),
    failure: UpdateFailure::WrongDigest,
  });
  assert_eq!(bar.card("status-update", "status-update-card"), None);
}
