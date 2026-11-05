//! How the Orchestrator watches the project's runs (Project settings › Orchestrator, ORC-757): the order a /watch
//! without text takes and the minutes without progress after which a run is reported, or never. Run with
//! `cargo test -p orchester-core --test orchestrator_watch_settings watch_settings`.

use orchester_core::orchestrator::{NoProgressError, WatchSettings};

#[test]
fn watch_settings_default_to_no_order_and_twenty_minutes() {
  let watch = WatchSettings::default();
  assert_eq!(watch.default_order(), "");
  assert_eq!(watch.no_progress_minutes(), Some(20));
}

#[test]
fn watch_settings_keep_the_order_trimmed_and_minutes_from_one_to_a_day_or_off() {
  let watch = WatchSettings::new("  Watch ORC-12 until it merges.\n", Some(1)).expect("one minute");
  assert_eq!(watch.default_order(), "Watch ORC-12 until it merges.");
  assert_eq!(watch.no_progress_minutes(), Some(1));
  assert_eq!(
    WatchSettings::new("", Some(1440)).expect("a day").no_progress_minutes(),
    Some(1440)
  );
  assert_eq!(
    WatchSettings::new(" \n", None).expect("off"),
    WatchSettings::new("", None).expect("off"),
    "a blank order is no order"
  );
}

#[test]
fn watch_settings_refuse_minutes_outside_one_to_a_day() {
  assert_eq!(
    WatchSettings::new("order", Some(0)),
    Err(NoProgressError { minutes: 0 })
  );
  assert_eq!(
    WatchSettings::new("order", Some(1441)),
    Err(NoProgressError { minutes: 1441 })
  );
}
