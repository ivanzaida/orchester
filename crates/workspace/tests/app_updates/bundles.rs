//! macOS: the helper that replaces the app's bundle once the app exited (`install/bundle/replace.sh`), with stand-in
//! bundles whose program is a shell script that notes it was opened. They are background only: no window or Dock icon
//! shows. The new bundle's `codesign` and Team ID checks need a Developer ID and are not run here.
//!
//! `cargo test -p orchester-workspace --test app_updates update_install_bundle`

use std::{
  fs,
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::written;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/app_updates/install/bundle/replace.sh");

/// A background-only bundle at `bundle` whose program writes `label` to `opened`; without a program when `label` is
/// `None`, so it does not open.
fn stand_in(bundle: &Path, label: Option<&str>, opened: &Path) {
  let made = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("after 1970")
    .as_nanos();
  let id = format!("dev.orchester.test.update.{}.{made}", std::process::id());
  let plist = format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
     <key>CFBundleIdentifier</key><string>{id}</string>\
     <key>CFBundleExecutable</key><string>run</string>\
     <key>CFBundlePackageType</key><string>APPL</string>\
     <key>LSBackgroundOnly</key><true/></dict></plist>\n"
  );
  written(&bundle.join("Contents").join("Info.plist"), plist.as_bytes());
  let Some(label) = label else {
    return;
  };
  let program = bundle.join("Contents").join("MacOS").join("run");
  written(
    &program,
    format!("#!/bin/sh\necho {label} > '{}'\n", opened.display()).as_bytes(),
  );
  fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("executable");
}

struct Replaced {
  app: PathBuf,
  unpacked: PathBuf,
  result: PathBuf,
  log: PathBuf,
  opened: PathBuf,
}

/// Runs the helper for an app process that exits after a second, replacing the bundle `old` with `new`.
fn replace(root: &Path, old: Option<&str>, new: Option<&str>) -> Replaced {
  let opened = root.join("opened.txt");
  let app = root.join("Applications").join("Orchester.app");
  let unpacked = root.join("Applications").join(".Orchester.app.partial");
  stand_in(&app, old, &opened);
  stand_in(&unpacked.join("Orchester.app"), new, &opened);
  let (result, log) = (root.join("result.txt"), root.join("logs").join("update.log"));
  fs::create_dir_all(root.join("logs")).expect("the logs");
  let mut running = Command::new("sleep").arg("1").spawn().expect("a stand-in app process");
  let mut helper = Command::new("/bin/sh")
    .arg(SCRIPT)
    .arg(running.id().to_string())
    .arg(&app)
    .arg(unpacked.join("Orchester.app"))
    .arg(&unpacked)
    .arg(&result)
    .arg(&log)
    .stdin(Stdio::null())
    .spawn()
    .expect("the helper");
  running.wait().expect("the app exited");
  helper.wait().expect("the helper ended");
  Replaced {
    app,
    unpacked,
    result,
    log,
    opened,
  }
}

/// What the bundle opened last wrote, once it did.
fn opened(path: &Path) -> String {
  let since = Instant::now();
  while since.elapsed() < Duration::from_secs(20) {
    if let Ok(text) = fs::read_to_string(path) {
      return text.trim().to_owned();
    }
    std::thread::sleep(Duration::from_millis(100));
  }
  panic!("no bundle opened");
}

fn program(app: &Path) -> String {
  fs::read_to_string(app.join("Contents").join("MacOS").join("run")).expect("the program")
}

#[test]
fn update_install_bundle_replaces_the_app_after_it_exited_and_opens_the_new_one() {
  let root = tempfile::tempdir().expect("a folder");

  let done = replace(root.path(), Some("old"), Some("new"));

  assert_eq!(opened(&done.opened), "new");
  assert!(program(&done.app).contains("echo new"));
  let aside = done.app.with_file_name(".Orchester.app.old");
  assert!(!aside.exists(), "the old bundle is removed");
  assert!(!done.unpacked.exists());
  assert!(!done.result.exists(), "nothing failed");
  let log = fs::read_to_string(&done.log).expect("the log");
  assert!(log.contains("replaced"), "{log}");
}

#[test]
fn update_install_bundle_that_does_not_open_puts_the_old_app_back_and_says_why() {
  let root = tempfile::tempdir().expect("a folder");

  let done = replace(root.path(), Some("old"), None);

  assert_eq!(opened(&done.opened), "old", "the old app opens again");
  assert!(program(&done.app).contains("echo old"));
  assert!(!done.unpacked.exists());
  let result = fs::read_to_string(&done.result).expect("why");
  assert_eq!(result.trim(), "the new app does not open");
}

/// Runs `program` with `arguments` and fails the test when it fails.
fn ran(program: &str, arguments: &[&std::ffi::OsStr]) {
  let status = Command::new(program).args(arguments).status().expect(program);
  assert!(status.success(), "{program} {arguments:?}: {status}");
}

/// The release's zip of a stand-in bundle, signed ad hoc when `signed`, as `ditto` packs it; and the installed
/// bundle's program folder, signed ad hoc too.
fn release_zip(root: &Path, signed: bool) -> (PathBuf, PathBuf) {
  let opened = root.join("opened.txt");
  let new = root.join("build").join("Orchester.app");
  stand_in(&new, Some("new"), &opened);
  if signed {
    ran("/usr/bin/codesign", &["--sign".as_ref(), "-".as_ref(), new.as_os_str()]);
  }
  let zip = root.join("updates").join("Orchester-0.4.0-macos-aarch64.zip");
  fs::create_dir_all(root.join("updates")).expect("the updates");
  ran(
    "/usr/bin/ditto",
    &[
      "-c".as_ref(),
      "-k".as_ref(),
      "--keepParent".as_ref(),
      new.as_os_str(),
      zip.as_os_str(),
    ],
  );
  let installed = root.join("Applications").join("Orchester.app");
  stand_in(&installed, Some("old"), &opened);
  ran(
    "/usr/bin/codesign",
    &["--sign".as_ref(), "-".as_ref(), installed.as_os_str()],
  );
  (zip, installed.join("Contents").join("MacOS"))
}

fn start(root: &Path, zip: PathBuf, program_folder: PathBuf) -> std::io::Result<std::process::Child> {
  orchester_workspace::app_updates::install::Handover {
    installer: zip,
    program_folder,
    pid: std::process::id(),
    log: root.join("update.log"),
    result: root.join("result.txt"),
  }
  .start()
}

#[test]
fn update_install_bundle_without_a_valid_signature_is_refused_and_nothing_is_left_unpacked() {
  let root = tempfile::tempdir().expect("a folder");
  let (zip, program_folder) = release_zip(root.path(), false);

  let error = start(root.path(), zip, program_folder).expect_err("refused");

  assert!(
    error
      .to_string()
      .starts_with("the new app's code signature does not verify"),
    "{error}"
  );
  assert!(!root.path().join("Applications").join(".Orchester.app.partial").exists());
}

#[test]
fn update_install_bundle_of_a_copy_signed_ad_hoc_is_refused_for_its_missing_team_id() {
  let root = tempfile::tempdir().expect("a folder");
  let (zip, program_folder) = release_zip(root.path(), true);

  let error = start(root.path(), zip, program_folder).expect_err("refused");

  assert_eq!(error.to_string(), "the running app has no Team ID");
  assert!(!root.path().join("Applications").join(".Orchester.app.partial").exists());
  assert!(program(&root.path().join("Applications").join("Orchester.app")).contains("echo old"));
}
