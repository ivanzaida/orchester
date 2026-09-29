use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_exec::verifier_app::{PinError, VerifierPlatform, pin_verifier_app};

fn scratch() -> tempfile::TempDir {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/d6-pin-tests");
  fs::create_dir_all(&root).expect("create scratch root");
  tempfile::Builder::new()
    .prefix("case-")
    .tempdir_in(root)
    .expect("create scratch case")
}

fn fixture(root: &Path, platform: VerifierPlatform) -> (PathBuf, PathBuf) {
  let source = root.join("source");
  let run = root.join("run");
  fs::create_dir(&source).expect("create source");
  fs::create_dir(&run).expect("create run");
  let (desktop, runner) = platform.program_names();
  fs::write(source.join(desktop), b"desktop A").expect("write desktop");
  fs::write(source.join(runner), b"runner A").expect("write runner");
  (source, run)
}

#[test]
fn pin_verifier_app_keeps_both_programs_after_target_rebuild() {
  for platform in [VerifierPlatform::Windows, VerifierPlatform::Macos] {
    let scratch = scratch();
    let (source, run) = fixture(scratch.path(), platform);
    let app = pin_verifier_app(&source, &run, platform).expect("pin synthetic app");
    assert_eq!(app.desktop().parent(), app.runner().parent());
    assert!(app.desktop().starts_with(run.canonicalize().expect("absolute run")));
    assert_eq!(fs::read(app.desktop()).expect("pinned desktop"), b"desktop A");
    assert_eq!(fs::read(app.runner()).expect("pinned runner"), b"runner A");

    let (desktop, runner) = platform.program_names();
    fs::write(source.join(desktop), b"desktop B").expect("replace source desktop");
    fs::write(source.join(runner), b"runner B").expect("replace source runner");
    assert_eq!(fs::read(app.desktop()).expect("pinned desktop"), b"desktop A");
    assert_eq!(fs::read(app.runner()).expect("pinned runner"), b"runner A");

    let second = pin_verifier_app(&source, &run, platform).expect("pin rebuilt app");
    assert_ne!(app.desktop(), second.desktop());
    assert_eq!(fs::read(second.desktop()).expect("second desktop"), b"desktop B");
    assert_eq!(fs::read(second.runner()).expect("second runner"), b"runner B");
  }
}

#[test]
fn pin_verifier_app_never_returns_a_partial_pair() {
  let scratch = scratch();
  let (source, run) = fixture(scratch.path(), VerifierPlatform::Windows);
  fs::remove_file(source.join("orchester-runner.exe")).expect("remove runner");

  let error = pin_verifier_app(&source, &run, VerifierPlatform::Windows).expect_err("missing runner must fail");
  assert!(matches!(error, PinError::Io { .. }));
  assert_eq!(fs::read_dir(run).expect("run directory").count(), 0);
}

#[test]
fn pin_verifier_app_cli_returns_the_pinned_desktop_path() {
  let scratch = scratch();
  let platform = VerifierPlatform::host().expect("supported host");
  let (source, run) = fixture(scratch.path(), platform);
  let output = Command::new(env!("CARGO_BIN_EXE_pin-verifier-app"))
    .args([&source, &run])
    .output()
    .expect("start pin CLI");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let desktop = PathBuf::from(String::from_utf8(output.stdout).expect("UTF-8 output").trim());
  assert!(desktop.is_absolute());
  assert_eq!(fs::read(&desktop).expect("desktop"), b"desktop A");
  assert_eq!(
    fs::read(desktop.with_file_name(platform.program_names().1)).expect("runner"),
    b"runner A"
  );
}
