//! Which copies update themselves, and what the others report.

use orchester_api::releases::ReleasePlatform;
use orchester_workspace::{
  app_updates::{CopyReason, NewestRelease, ThisCopy, UpdateState},
  build::Build,
};
use serde_json::json;

use super::{
  Copy, DOWNLOAD, Offer, installer_bytes, is_check, nobody, offering,
  stand::{Answer, Stand},
  version,
};

/// The copy a program of `build` in a folder holding `files` is, on Windows.
fn copy_with(build: Build, qa: bool, files: &[&str]) -> ThisCopy {
  let folder = tempfile::tempdir().expect("a folder");
  for file in ["orchester.exe", "orchester-runner.exe"].iter().chain(files) {
    std::fs::write(folder.path().join(file), b"").expect("a file");
  }
  ThisCopy::of(build, qa, ReleasePlatform::Windows, Some(folder.path()))
}

#[test]
fn app_updates_only_the_installers_copy_on_windows_updates_itself() {
  let installed = copy_with(Build::Installed, false, &["unins000.exe", "unins000.dat"]);
  assert_eq!(installed, ThisCopy::Installer);
  assert!(installed.updates_itself());

  assert_eq!(
    copy_with(Build::Installed, false, &["README.txt"]),
    ThisCopy::NotThisCopy(CopyReason::Portable)
  );
  assert_eq!(
    copy_with(Build::Installed, false, &[]),
    ThisCopy::NotThisCopy(CopyReason::ScriptInstall)
  );
  // An uninstaller without its data is not the installer's.
  assert_eq!(
    copy_with(Build::Installed, false, &["unins000.exe"]),
    ThisCopy::NotThisCopy(CopyReason::ScriptInstall)
  );
  assert_eq!(
    copy_with(Build::Other, false, &["unins000.exe", "unins000.dat"]),
    ThisCopy::NotThisCopy(CopyReason::Development)
  );
  assert_eq!(
    copy_with(Build::Installed, true, &["unins000.exe", "unins000.dat"]),
    ThisCopy::NotThisCopy(CopyReason::Qa)
  );
  assert_eq!(
    ThisCopy::of(Build::Installed, false, ReleasePlatform::Windows, None),
    ThisCopy::NotThisCopy(CopyReason::ProgramFolderUnknown)
  );
}

#[test]
fn app_updates_no_copy_on_macos_updates_itself_yet() {
  let folder = tempfile::tempdir().expect("a folder");
  assert_eq!(
    ThisCopy::of(Build::Installed, false, ReleasePlatform::Macos, Some(folder.path())),
    ThisCopy::NotOnThisPlatform
  );
  assert_eq!(
    ThisCopy::of(Build::Other, true, ReleasePlatform::Macos, Some(folder.path())),
    ThisCopy::NotThisCopy(CopyReason::Qa)
  );
}

#[tokio::test]
async fn app_updates_a_copy_updated_by_hand_names_the_newest_release_and_downloads_nothing() {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes);
  let copy = Copy::new(&stand.api, ThisCopy::NotThisCopy(CopyReason::Portable));
  assert_eq!(
    copy.updates.state(),
    UpdateState::NotThisCopy {
      reason: CopyReason::Portable,
      newest: None
    }
  );

  copy.updates.check_now().await;

  let newest = NewestRelease {
    version: version("0.4.0"),
    download: DOWNLOAD.to_owned(),
  };
  assert_eq!(
    copy.updates.state(),
    UpdateState::NotThisCopy {
      reason: CopyReason::Portable,
      newest: Some(newest)
    }
  );
  assert!(stand.asked().iter().all(is_check), "nothing downloads");
  assert_eq!(copy.downloads(), Vec::<String>::new());
}

#[tokio::test]
async fn app_updates_macos_names_the_newest_release_and_keeps_it_when_the_cloud_is_away() {
  let bytes = installer_bytes();
  let stand = offering(Offer::of(&bytes), bytes);
  let copy = Copy::new(&stand.api, ThisCopy::NotOnThisPlatform);
  copy.updates.check_now().await;
  let UpdateState::NotOnThisPlatform { newest: Some(newest) } = copy.updates.state() else {
    panic!("the newest release: {:?}", copy.updates.state());
  };
  assert_eq!(newest.version, version("0.4.0"));

  let path = copy.workspace.database_path().expect("a file");
  let away = orchester_store::StoredCloudAddress {
    web: "https://orchester.cloud".to_owned(),
    api: Some(nobody()),
  };
  orchester_store::AppDb::open(&path)
    .expect("the database")
    .set_cloud_address(Some(&away))
    .expect("moved");
  copy.updates.check_now().await;

  assert_eq!(
    copy.updates.state(),
    UpdateState::NotOnThisPlatform { newest: Some(newest) }
  );
}

#[tokio::test]
async fn app_updates_a_development_copy_with_nothing_newer_names_no_release() {
  let stand = Stand::start(|_| Answer::json(&json!({ "update": null })));
  let copy = Copy::new(&stand.api, ThisCopy::NotThisCopy(CopyReason::Development));
  copy.updates.check_now().await;
  assert_eq!(
    copy.updates.state(),
    UpdateState::NotThisCopy {
      reason: CopyReason::Development,
      newest: None
    }
  );
}
