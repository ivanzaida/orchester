//! The tool's `publish` command against the stand-in cloud: the Windows and macOS files of one release.

use std::{
  path::{Path, PathBuf},
  process::{Command, Output},
};

use orchester_api::releases::{AcceptedReleaseKeys, DeclaredFile, ReleaseArch, ReleasePlatform, ReleaseSigningKey};
use serde_json::{Value, json};

use super::{SEED, stand::Stand, version};

const TOKEN: &str = "orct_release_0123456789";
const INSTALLER: &str = "Orchester-0.5.0-setup.exe";
const DISK_IMAGE: &str = "Orchester-0.5.0-macos-aarch64.dmg";

fn publish(api: &str, arguments: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_orchester-release"))
    .args(["publish", "--api", api, "--version", "0.5.0"])
    .args(arguments)
    .env("ORCHESTER_RELEASE_SIGNING_KEY", SEED)
    .env("ORCHESTER_CLOUD_RELEASE_TOKEN", TOKEN)
    .output()
    .expect("the tool runs")
}

fn written(folder: &Path, name: &str) -> PathBuf {
  let path = folder.join(name);
  std::fs::write(&path, name).expect("written");
  path
}

fn upload(name: &str, received: &[u32]) -> Value {
  json!({ "name": name, "size": name.len(), "part_size": name.len(), "parts": 1, "received": received })
}

#[test]
fn release_signing_publishes_both_platforms_in_one_release() {
  let summary = json!({ "release": {
    "version": "0.5.0", "stage": "draft", "critical": false, "created_at": 1, "published_at": null, "installs": 0,
    "files_deleted": false,
  }});
  let accepted = json!({ "version": "0.5.0", "files": [
    { "name": INSTALLER, "result": "accepted" },
    { "name": DISK_IMAGE, "result": "accepted" },
  ]});
  let stand = Stand::start(vec![
    ("201 Created", summary),
    ("201 Created", upload(INSTALLER, &[])),
    ("200 OK", upload(INSTALLER, &[0])),
    ("201 Created", upload(DISK_IMAGE, &[])),
    ("200 OK", upload(DISK_IMAGE, &[0])),
    ("200 OK", accepted),
  ]);
  let folder = tempfile::tempdir().expect("a folder");
  let (installer, disk_image) = (written(folder.path(), INSTALLER), written(folder.path(), DISK_IMAGE));
  let notes = written(folder.path(), "notes.md");
  let output = publish(
    &stand.address,
    &[
      "--notes-file",
      notes.to_str().expect("a path"),
      "--platform",
      "windows",
      "--arch",
      "x86_64",
      installer.to_str().expect("a path"),
      "--platform",
      "macos",
      "--arch",
      "aarch64",
      disk_image.to_str().expect("a path"),
    ],
  );
  let (stdout, stderr) = (
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr),
  );
  assert!(output.status.success(), "{stdout}{stderr}");
  for shown in [&stdout, &stderr] {
    assert!(!shown.contains(SEED) && !shown.contains(TOKEN));
  }

  let asked = stand.asked();
  let lines: Vec<&str> = asked.iter().map(|asked| asked.line.as_str()).collect();
  assert_eq!(
    lines,
    [
      "POST /v1/releases".to_owned(),
      "POST /v1/releases/0.5.0/files".to_owned(),
      format!("PUT /v1/releases/0.5.0/files/{INSTALLER}/parts/0"),
      "POST /v1/releases/0.5.0/files".to_owned(),
      format!("PUT /v1/releases/0.5.0/files/{DISK_IMAGE}/parts/0"),
      "POST /v1/releases/0.5.0/finalize".to_owned(),
    ],
    "one draft and one finalize, after the last file"
  );
  let accepted_keys = AcceptedReleaseKeys::new(vec![ReleaseSigningKey::parse(SEED).expect("the key").public_key()]);
  for (request, name, platform, arch) in [
    (&asked[1], INSTALLER, ReleasePlatform::Windows, ReleaseArch::X86_64),
    (&asked[3], DISK_IMAGE, ReleasePlatform::Macos, ReleaseArch::Aarch64),
  ] {
    let declared: DeclaredFile = serde_json::from_slice(&request.body).expect("a declaration");
    assert_eq!(
      (declared.name.as_str(), declared.platform, declared.arch),
      (name, platform, arch)
    );
    assert_eq!(
      accepted_keys.verify(
        &declared.signed(&version("0.5.0")),
        declared.key_id,
        &declared.signature
      ),
      Ok(()),
      "signed for its own platform and architecture"
    );
  }
}

#[test]
fn release_signing_publish_refuses_a_file_without_its_platform_and_architecture() {
  let stand = Stand::start(Vec::new());
  for arguments in [
    &["--notes-file", "n", "a.zip"][..],
    &["--notes-file", "n", "--platform", "windows", "a.zip"],
    // The architecture named for Windows does not carry over to macOS.
    &[
      "--notes-file",
      "n",
      "--platform",
      "windows",
      "--arch",
      "x86_64",
      "a.zip",
      "--platform",
      "macos",
      "b.zip",
    ],
  ] {
    let output = publish(&stand.address, arguments);
    assert!(!output.status.success());
    let shown = String::from_utf8_lossy(&output.stderr);
    assert!(shown.contains("before the file"), "{shown}");
  }
  assert!(stand.asked().is_empty());
}
