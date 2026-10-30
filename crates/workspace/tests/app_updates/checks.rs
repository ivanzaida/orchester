//! The settings and the check: what is stored, what the cloud is asked, and the states of a check.

use std::time::Duration;

use orchester_core::app_update::{Channel, UpdateSettings};
use orchester_workspace::app_updates::{UpdateState, keep_checking};
use serde_json::json;

use super::{
  Copy, Offer, installer_bytes, is_check, nobody, now, offering,
  stand::{Answer, Stand},
};

#[test]
fn app_updates_settings_are_stable_and_automatic_until_changed_and_stored() {
  let copy = Copy::installed(&nobody());
  assert_eq!(copy.updates.settings().expect("settings"), UpdateSettings::default());
  assert_eq!(
    UpdateSettings::default(),
    UpdateSettings {
      channel: Channel::Stable,
      automatic: true
    }
  );

  copy.updates.set_channel(Channel::Beta).expect("channel");
  copy.updates.set_automatic(false).expect("automatic");

  let path = copy.workspace.database_path().expect("a file");
  let reopened = orchester_store::AppDb::open(&path).expect("reopened");
  assert_eq!(
    reopened.update_settings().expect("settings"),
    UpdateSettings {
      channel: Channel::Beta,
      automatic: false
    }
  );
}

#[tokio::test]
async fn app_updates_check_asks_anonymously_with_the_copy_and_its_channel() {
  let stand = Stand::start(|_| Answer::json(&json!({ "update": null })));
  let copy = Copy::installed(&stand.api);
  copy.updates.set_channel(Channel::Beta).expect("channel");
  let install = copy.workspace.installation_id().expect("an install id");

  copy.updates.check_now().await;

  let asked = stand.asked();
  assert_eq!(asked.len(), 1, "{asked:?}");
  assert_eq!(
    asked[0].line,
    format!("GET /v1/updates/check?version=0.3.0&platform=windows&arch=x86_64&channel=beta&install_id={install}")
  );
  assert_eq!(asked[0].header("authorization"), None, "no sign-in");
  assert_eq!(asked[0].header("cookie"), None);
}

#[tokio::test]
async fn app_updates_nothing_newer_is_up_to_date_and_kept_as_the_last_check() {
  let stand = Stand::start(|_| Answer::json(&json!({ "update": null })));
  let copy = Copy::installed(&stand.api);
  // An older download goes once nothing newer is offered.
  super::written(&copy.folder().join("Orchester-0.3.5-setup.exe.part"), b"older");
  let before = now();

  copy.updates.check_now().await;

  let UpdateState::UpToDate { checked_at } = copy.updates.state() else {
    panic!("up to date: {:?}", copy.updates.state());
  };
  assert!(checked_at >= before);
  let last = copy.updates.last_checked().expect("read").expect("kept");
  assert!(last >= before);
  assert_eq!(copy.downloads(), Vec::<String>::new());
}

#[tokio::test]
async fn app_updates_an_unreachable_cloud_says_when_it_last_answered() {
  let stand = Stand::start(|_| Answer::json(&json!({ "update": null })));
  let copy = Copy::installed(&stand.api);
  copy.updates.check_now().await;
  let answered = copy.updates.last_checked().expect("read");
  assert!(answered.is_some());

  let address = orchester_store::StoredCloudAddress {
    web: "https://orchester.cloud".to_owned(),
    api: Some(nobody()),
  };
  let path = copy.workspace.database_path().expect("a file");
  orchester_store::AppDb::open(&path)
    .expect("the database")
    .set_cloud_address(Some(&address))
    .expect("moved");
  copy.updates.check_now().await;

  assert_eq!(
    copy.updates.state(),
    UpdateState::CloudUnreachable { last_checked: answered }
  );
}

#[tokio::test]
async fn app_updates_a_cloud_never_reached_has_no_last_check() {
  let copy = Copy::installed(&nobody());
  assert_eq!(
    copy.updates.state(),
    UpdateState::Checking,
    "an installer's copy checks as it starts"
  );

  copy.updates.check_now().await;

  assert_eq!(
    copy.updates.state(),
    UpdateState::CloudUnreachable { last_checked: None }
  );
}

#[tokio::test]
async fn app_updates_keep_checking_asks_at_the_start_and_again_after_each_wait() {
  let stand = Stand::start(|_| Answer::json(&json!({ "update": null })));
  let copy = Copy::installed(&stand.api);

  let checking = tokio::spawn(keep_checking(copy.updates.clone(), Duration::from_millis(150)));
  tokio::time::sleep(Duration::from_millis(100)).await;
  let at_start = stand.asked().into_iter().filter(is_check).count();
  tokio::time::sleep(Duration::from_millis(400)).await;
  checking.abort();
  let later = stand.asked().into_iter().filter(is_check).count();

  assert_eq!(at_start, 1, "one check as it starts");
  assert!(later >= 3, "checked again after each wait: {later}");
  assert_eq!(
    orchester_workspace::app_updates::CHECK_EVERY,
    Duration::from_secs(6 * 60 * 60)
  );
}

#[tokio::test]
async fn app_updates_a_ready_update_waits_for_the_runs_at_work() {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes);
  let copy = Copy::installed(&stand.api);
  copy.updates.set_runs_at_work(vec!["ORC-110".to_owned()]);

  copy.updates.check_now().await;

  let UpdateState::WaitingForRuns { update, runs } = copy.updates.state() else {
    panic!("waiting for runs: {:?}", copy.updates.state());
  };
  assert_eq!((update.version.as_str(), runs), ("0.4.0", vec!["ORC-110".to_owned()]));

  copy
    .updates
    .set_runs_at_work(vec!["ORC-110".to_owned(), "ORC-346".to_owned()]);
  let UpdateState::WaitingForRuns { runs, .. } = copy.updates.state() else {
    panic!("still waiting");
  };
  assert_eq!(runs.len(), 2);

  copy.updates.set_runs_at_work(Vec::new());
  assert!(
    matches!(copy.updates.state(), UpdateState::Ready(_)),
    "{:?}",
    copy.updates.state()
  );
}

#[test]
fn app_updates_a_download_tells_its_percentage() {
  let downloading = UpdateState::Downloading {
    version: super::version("0.4.0"),
    received: 38,
    total: 91,
  };
  assert_eq!(downloading.percent(), Some(41));
  assert_eq!(UpdateState::Checking.percent(), None);
}
