//! Which copies update themselves, and what the others report.

use std::path::{Path, PathBuf};

use orchester_api::releases::{ReleaseArch, ReleasePlatform};
use orchester_workspace::{
  app_updates::{CopyReason, NewestRelease, ThisCopy, UpdateFailure, UpdateState},
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

/// The program folder `Contents/MacOS` of a bundle made at `bundle`.
fn bundle(bundle: &Path) -> PathBuf {
  let program = bundle.join("Contents").join("MacOS");
  std::fs::create_dir_all(&program).expect("the bundle");
  program
}

fn on_macos(build: Build, qa: bool, folder: &Path) -> ThisCopy {
  ThisCopy::of(build, qa, ReleasePlatform::Macos, Some(folder))
}

#[test]
fn app_updates_on_macos_a_bundle_the_person_can_replace_updates_itself() {
  let applications = tempfile::tempdir().expect("a folder");
  let program = bundle(&applications.path().join("Orchester.app"));

  let copy = on_macos(Build::Installed, false, &program);
  assert_eq!(copy, ThisCopy::Bundle);
  assert!(copy.updates_itself());
  assert_eq!(
    std::fs::read_dir(applications.path()).expect("the folder").count(),
    1,
    "the check that the folder can be written leaves nothing behind"
  );
  assert_eq!(
    on_macos(Build::Other, false, &program),
    ThisCopy::NotThisCopy(CopyReason::Development)
  );
  assert_eq!(
    on_macos(Build::Installed, true, &program),
    ThisCopy::NotThisCopy(CopyReason::Qa)
  );
}

#[test]
fn app_updates_on_macos_a_copy_that_cannot_be_replaced_in_place_says_why() {
  let root = tempfile::tempdir().expect("a folder");
  let loose = root.path().join("target").join("release");
  std::fs::create_dir_all(&loose).expect("a folder");
  assert_eq!(
    on_macos(Build::Installed, false, &loose),
    ThisCopy::NotThisCopy(CopyReason::NotInBundle)
  );
  let translocated = bundle(
    &root
      .path()
      .join("AppTranslocation")
      .join("A1B2")
      .join("d")
      .join("Orchester.app"),
  );
  assert_eq!(
    on_macos(Build::Installed, false, &translocated),
    ThisCopy::NotThisCopy(CopyReason::Translocated)
  );
  assert_eq!(
    ThisCopy::of(Build::Installed, false, ReleasePlatform::Macos, None),
    ThisCopy::NotThisCopy(CopyReason::ProgramFolderUnknown)
  );
}

#[cfg(unix)]
#[test]
fn app_updates_on_macos_a_bundle_in_a_folder_the_person_cannot_write_does_not_update_itself() {
  use std::os::unix::fs::PermissionsExt;
  let root = tempfile::tempdir().expect("a folder");
  let applications = root.path().join("Applications");
  let program = bundle(&applications.join("Orchester.app"));
  std::fs::set_permissions(&applications, std::fs::Permissions::from_mode(0o555)).expect("read only");

  let copy = on_macos(Build::Installed, false, &program);

  std::fs::set_permissions(&applications, std::fs::Permissions::from_mode(0o755)).expect("writable again");
  assert_eq!(copy, ThisCopy::NotThisCopy(CopyReason::BundleNotWritable));
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
async fn app_updates_a_bundle_on_macos_downloads_the_release_zip_and_not_another_file() {
  let bytes = installer_bytes();
  let mut zip = Offer::of(&bytes);
  zip.file = (
    "Orchester-0.4.0-macos-aarch64.zip",
    ReleasePlatform::Macos,
    ReleaseArch::Aarch64,
  );
  let stand = offering(zip, bytes.clone());
  let target = (ReleasePlatform::Macos, ReleaseArch::Aarch64);
  let copy = Copy::on(&stand.api, ThisCopy::Bundle, target);
  assert_eq!(copy.updates.state(), UpdateState::Checking);

  copy.updates.check_now().await;

  let UpdateState::Ready(ready) = copy.updates.state() else {
    panic!("ready: {:?}", copy.updates.state());
  };
  assert_eq!(ready.installer, copy.folder().join("Orchester-0.4.0-macos-aarch64.zip"));
  assert_eq!(std::fs::read(&ready.installer).expect("the zip"), bytes);

  let mut image = Offer::of(&bytes);
  image.file = (
    "Orchester-0.4.0-macos-aarch64.dmg",
    ReleasePlatform::Macos,
    ReleaseArch::Aarch64,
  );
  let stand = offering(image, bytes);
  let copy = Copy::on(&stand.api, ThisCopy::Bundle, target);
  copy.updates.check_now().await;
  assert!(
    matches!(
      copy.updates.state(),
      UpdateState::Failed {
        failure: UpdateFailure::NoInstaller,
        ..
      }
    ),
    "the disk image is not what a bundle updates from: {:?}",
    copy.updates.state()
  );
}

#[tokio::test]
async fn app_updates_not_on_this_platform_names_the_newest_release_and_keeps_it_when_the_cloud_is_away() {
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
