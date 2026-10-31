//! The download of an offered installer: kept only once verified, resumed after an interruption, and deleted and
//! reported when it fails a check.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use orchester_workspace::app_updates::{UpdateFailure, UpdateState};

use super::{
  Copy, INSTALLER, OTHER_SEED, Offer, installer_bytes, is_check, offering,
  stand::{Answer, Stand},
  version,
};

#[tokio::test]
async fn app_updates_a_verified_download_is_ready_with_its_notes() {
  let bytes = installer_bytes();
  let mut offer = Offer::of(&bytes);
  offer.critical = true;
  let stand = offering(offer, bytes.clone());
  let copy = Copy::installed(&stand.api);

  copy.updates.check_now().await;

  let UpdateState::Ready(ready) = copy.updates.state() else {
    panic!("ready: {:?}", copy.updates.state());
  };
  assert_eq!(ready.version, version("0.4.0"));
  assert_eq!(ready.notes, "Runs start faster.");
  assert!(ready.critical);
  assert_eq!(ready.installer, copy.folder().join(INSTALLER));
  assert_eq!(std::fs::read(&ready.installer).expect("the installer"), bytes);
  assert_eq!(copy.downloads(), vec![INSTALLER.to_owned()]);
  let lines: Vec<String> = stand.asked().into_iter().map(|asked| asked.line).collect();
  assert_eq!(
    lines[1],
    format!("GET /v1/releases/0.4.0/files/{INSTALLER}"),
    "{lines:?}"
  );
  assert_eq!(stand.asked()[1].header("authorization"), None);
}

#[tokio::test]
async fn app_updates_a_ready_download_is_not_downloaded_again() {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes);
  let copy = Copy::installed(&stand.api);
  copy.updates.check_now().await;
  copy.updates.check_now().await;

  assert!(matches!(copy.updates.state(), UpdateState::Ready(_)));
  let downloads = stand.asked().into_iter().filter(|asked| !is_check(asked)).count();
  assert_eq!(downloads, 1);
}

#[tokio::test]
async fn app_updates_an_interrupted_download_resumes_with_a_range() {
  let bytes = installer_bytes();
  let half = bytes.len() / 2;
  let offer = Offer::of(&bytes);
  let served = bytes.clone();
  let broke_off = Arc::new(AtomicBool::new(false));
  let stand = Stand::start(move |asked| {
    if is_check(asked) {
      return Answer::json(&offer.answer());
    }
    let mut answer = Answer::file(asked, &served);
    if !broke_off.swap(true, Ordering::SeqCst) {
      answer.cut = Some(half);
    }
    answer
  });
  let copy = Copy::installed(&stand.api);

  copy.updates.check_now().await;
  assert!(
    matches!(
      copy.updates.state(),
      UpdateState::CloudUnreachable { last_checked: Some(_) }
    ),
    "{:?}",
    copy.updates.state()
  );
  assert_eq!(
    copy.downloads(),
    vec![format!("{INSTALLER}.part")],
    "what arrived is kept"
  );
  let kept = std::fs::metadata(copy.folder().join(format!("{INSTALLER}.part")))
    .expect("the part")
    .len();
  assert!(kept > 0 && kept < bytes.len() as u64, "{kept}");

  copy.updates.check_now().await;

  assert!(
    matches!(copy.updates.state(), UpdateState::Ready(_)),
    "{:?}",
    copy.updates.state()
  );
  let downloads: Vec<_> = stand.asked().into_iter().filter(|asked| !is_check(asked)).collect();
  assert_eq!(downloads.len(), 2);
  assert_eq!(downloads[0].header("range"), None);
  assert_eq!(downloads[1].header("range"), Some(format!("bytes={kept}-").as_str()));
  assert_eq!(
    std::fs::read(copy.folder().join(INSTALLER)).expect("the installer"),
    bytes
  );
}

/// *Pause* keeps what arrived and stops the download; a check meanwhile downloads nothing; *Resume* goes on from the
/// byte it stopped at.
#[tokio::test]
async fn app_updates_a_paused_download_resumes_where_it_stopped() {
  let bytes = installer_bytes();
  let half = bytes.len() / 2;
  let offer = Offer::of(&bytes);
  let served = bytes.clone();
  let stalled = Arc::new(AtomicBool::new(false));
  let stand = Stand::start(move |asked| {
    if is_check(asked) {
      return Answer::json(&offer.answer());
    }
    let mut answer = Answer::file(asked, &served);
    if !stalled.swap(true, Ordering::SeqCst) {
      answer.stall = Some(half);
    }
    answer
  });
  let copy = Copy::installed(&stand.api);
  let updates = copy.updates.clone();
  let check = tokio::spawn(async move { updates.check_now().await });
  let mut changes = copy.updates.subscribe();
  let downloading =
    changes.wait_for(|state| matches!(state, UpdateState::Downloading { received, .. } if *received > 0));
  tokio::time::timeout(std::time::Duration::from_secs(20), downloading)
    .await
    .expect("the download to start")
    .map(drop)
    .expect("the state to be followed");

  copy.updates.pause();
  check.await.expect("the check ends");

  let UpdateState::Paused {
    version: paused,
    received,
    total,
  } = copy.updates.state()
  else {
    panic!("paused: {:?}", copy.updates.state());
  };
  assert_eq!(paused, version("0.4.0"));
  assert!(received > 0 && received < total, "{received} of {total}");
  assert_eq!(
    copy.downloads(),
    vec![format!("{INSTALLER}.part")],
    "what arrived is kept"
  );
  let kept = std::fs::metadata(copy.folder().join(format!("{INSTALLER}.part")))
    .expect("the part")
    .len();

  copy.updates.check_now().await;
  assert!(
    matches!(copy.updates.state(), UpdateState::Paused { .. }),
    "a check while paused downloads nothing: {:?}",
    copy.updates.state()
  );

  copy.updates.resume().await;

  assert!(
    matches!(copy.updates.state(), UpdateState::Ready(_)),
    "{:?}",
    copy.updates.state()
  );
  let downloads: Vec<_> = stand.asked().into_iter().filter(|asked| !is_check(asked)).collect();
  assert_eq!(downloads.len(), 2);
  assert_eq!(downloads[1].header("range"), Some(format!("bytes={kept}-").as_str()));
  assert_eq!(
    std::fs::read(copy.folder().join(INSTALLER)).expect("the installer"),
    bytes
  );
}

/// *Pause* pressed while the state still reads a download that no fetch listens to any more (here the check that ran it
/// was dropped) pauses nothing: the next check still checks and downloads.
#[tokio::test]
async fn app_updates_a_pause_no_download_took_up_does_not_stop_later_checks() {
  let bytes = installer_bytes();
  let half = bytes.len() / 2;
  let offer = Offer::of(&bytes);
  let served = bytes.clone();
  let stalled = Arc::new(AtomicBool::new(false));
  let stand = Stand::start(move |asked| {
    if is_check(asked) {
      return Answer::json(&offer.answer());
    }
    let mut answer = Answer::file(asked, &served);
    if !stalled.swap(true, Ordering::SeqCst) {
      answer.stall = Some(half);
    }
    answer
  });
  let copy = Copy::installed(&stand.api);
  let updates = copy.updates.clone();
  let check = tokio::spawn(async move { updates.check_now().await });
  let mut changes = copy.updates.subscribe();
  let downloading =
    changes.wait_for(|state| matches!(state, UpdateState::Downloading { received, .. } if *received > 0));
  tokio::time::timeout(std::time::Duration::from_secs(20), downloading)
    .await
    .expect("the download to start")
    .map(drop)
    .expect("the state to be followed");
  check.abort();
  let _ = check.await;
  assert!(
    matches!(copy.updates.state(), UpdateState::Downloading { .. }),
    "{:?}",
    copy.updates.state()
  );

  copy.updates.pause();
  copy.updates.check_now().await;

  assert!(
    matches!(copy.updates.state(), UpdateState::Ready(_)),
    "a later check still checks: {:?}",
    copy.updates.state()
  );
}

/// Checks once against a cloud that offers `offer` but serves `served`, and returns the copy.
async fn failing(offer: Offer, served: Vec<u8>) -> Copy {
  let stand = offering(offer, served);
  let copy = Copy::installed(&stand.api);
  copy.updates.check_now().await;
  copy
}

fn failed(copy: &Copy, failure: UpdateFailure) {
  assert_eq!(
    copy.updates.state(),
    UpdateState::Failed {
      version: version("0.4.0"),
      failure
    }
  );
  assert_eq!(copy.downloads(), Vec::<String>::new(), "nothing unverified is kept");
}

#[tokio::test]
async fn app_updates_a_download_with_another_digest_is_deleted_and_reported() {
  let bytes = installer_bytes();
  let mut other = bytes.clone();
  other[1000] ^= 0xff;
  let copy = failing(Offer::of(&bytes), other).await;
  failed(&copy, UpdateFailure::WrongDigest);
}

#[tokio::test]
async fn app_updates_a_download_signed_with_an_unknown_key_is_deleted_and_reported() {
  let bytes = installer_bytes();
  let mut offer = Offer::of(&bytes);
  offer.seed = OTHER_SEED;
  let copy = failing(offer, bytes).await;
  failed(&copy, UpdateFailure::UnknownKey);
}

#[tokio::test]
async fn app_updates_a_download_whose_signature_does_not_verify_is_deleted_and_reported() {
  let bytes = installer_bytes();
  let mut offer = Offer::of(&bytes);
  offer.sign_other_name = true;
  let copy = failing(offer, bytes).await;
  failed(&copy, UpdateFailure::BadSignature);
}

#[tokio::test]
async fn app_updates_a_download_of_another_size_is_deleted_and_reported() {
  let bytes = installer_bytes();
  let longer = [bytes.as_slice(), b"more"].concat();
  let copy = failing(Offer::of(&bytes), longer).await;
  failed(&copy, UpdateFailure::WrongSize);
}

#[tokio::test]
async fn app_updates_a_kept_installer_that_no_longer_verifies_is_downloaded_again() {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes.clone());
  let copy = Copy::installed(&stand.api);
  super::written(&copy.folder().join(INSTALLER), b"changed since");

  copy.updates.check_now().await;

  assert!(matches!(copy.updates.state(), UpdateState::Ready(_)));
  assert_eq!(
    std::fs::read(copy.folder().join(INSTALLER)).expect("the installer"),
    bytes
  );
}
