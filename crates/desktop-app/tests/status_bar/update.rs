//! The update item (`Row 2 · status bar` of `Auto-updater · redesign` `acFkz`): ready with *Restart* (S2 `RccSH`),
//! waiting for runs (S3 `UCZt3`), installing, *Updated to* (S6 `wZaZb`), and an update that did not install, with
//! why on its card; its press asks the window for *Restart to update* or opens the installer's log.
//!
//! `cargo test -p orchester-app --test status_bar update_install`

use super::*;

fn with(update: UpdateFacts) -> Bar {
  Bar::of(BarFacts { update, ..facts(None) })
}

#[test]
fn update_install_nothing_to_say_shows_no_item() {
  let mut bar = with(UpdateFacts::Quiet);
  assert_eq!(bar.texts("status-update"), None);
}

#[test]
fn update_install_a_ready_update_offers_restart_to_update() {
  let mut bar = with(UpdateFacts::Ready {
    version: "0.4.0".to_owned(),
  });

  assert_eq!(
    bar.texts("status-update"),
    Some(vec!["Update 0.4.0 ready · Restart".to_owned()])
  );
  bar.press("status-update");
  assert_eq!(bar.asked(), [BarAction::RestartToUpdate]);
}

#[test]
fn update_install_waiting_for_runs_names_how_many_and_asks_again() {
  let mut bar = with(UpdateFacts::Waiting {
    version: "0.4.0".to_owned(),
    runs: 3,
  });
  assert_eq!(
    bar.texts("status-update"),
    Some(vec!["Update 0.4.0 · after 3 runs end".to_owned()])
  );
  bar.press("status-update");
  assert_eq!(bar.asked(), [BarAction::RestartToUpdate]);

  let mut one = with(UpdateFacts::Waiting {
    version: "0.4.0".to_owned(),
    runs: 1,
  });
  assert_eq!(
    one.texts("status-update"),
    Some(vec!["Update 0.4.0 · after 1 run ends".to_owned()])
  );
}

#[test]
fn update_install_installing_and_updated_say_so_and_do_nothing() {
  let mut installing = with(UpdateFacts::Installing {
    version: "0.4.0".to_owned(),
  });
  assert_eq!(
    installing.texts("status-update"),
    Some(vec!["Installing 0.4.0".to_owned()])
  );
  installing.press("status-update");
  assert_eq!(installing.asked(), []);

  let mut updated = with(UpdateFacts::Updated {
    version: "0.4.0".to_owned(),
  });
  assert_eq!(
    updated.texts("status-update"),
    Some(vec!["Updated to 0.4.0".to_owned()])
  );
}

#[test]
fn update_install_a_failed_install_says_why_and_opens_its_log() {
  let log = PathBuf::from("C:/Orchester/logs/update-0.4.0.log");
  let mut bar = with(UpdateFacts::NotInstalled {
    version: "0.4.0".to_owned(),
    reason: "installing the files failed and the changes were rolled back".to_owned(),
    log: Some(log.clone()),
  });

  assert_eq!(
    bar.texts("status-update-text"),
    Some(vec!["Update 0.4.0 didn't install".to_owned()])
  );
  let card = bar.card("status-update", "status-update-card").expect("the card");
  assert!(
    card.contains(&"installing the files failed and the changes were rolled back".to_owned()),
    "{card:?}"
  );
  assert!(
    card.contains(&"Press to open the installer's log.".to_owned()),
    "{card:?}"
  );
  bar.press("status-update");
  assert_eq!(bar.asked(), [BarAction::OpenFile(log)]);
}
