//! The app's updates (`orchester_workspace::app_updates`) against a stand-in Orchester Cloud, with release files signed
//! by a throwaway key: the settings, the anonymous check, the download with its resume and its checks, the states and
//! which copies update themselves.
//!
//! `cargo test -p orchester-workspace --test app_updates app_updates`

mod checks;
mod copies;
mod downloads;
mod stand;

use std::path::{Path, PathBuf};

use orchester_api::releases::{
  AcceptedReleaseKeys, FileDigest, FileSignature, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleasePublicKey,
  ReleaseVersion, SignedFile,
};
use orchester_core::time::Timestamp;
use orchester_store::{AppDb, StoredCloudAddress};
use orchester_workspace::{
  app_updates::{AppUpdates, ThisApp, ThisCopy, UPDATES_FOLDER},
  workspace::Workspace,
};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{Value, json};
use tempfile::TempDir;

use self::stand::{Answer, Asked, Stand};

/// The version the copy under test runs.
const RUNNING: &str = "0.3.0";
/// The version the stand-in offers.
const NEWER: &str = "0.4.0";
const INSTALLER: &str = "Orchester-0.4.0-setup.exe";
/// Where the offered installer downloads, as the check's answer names it.
const DOWNLOAD: &str = "https://api.orchester.cloud/v1/releases/0.4.0/files/Orchester-0.4.0-setup.exe";
/// The throwaway release key the app under test accepts.
const RELEASE_SEED: [u8; 32] = [7; 32];
/// A key the app does not accept.
const OTHER_SEED: [u8; 32] = [9; 32];

fn version(text: &str) -> ReleaseVersion {
  ReleaseVersion::parse(text).expect("a version")
}

fn pair(seed: [u8; 32]) -> Ed25519KeyPair {
  Ed25519KeyPair::from_seed_unchecked(&seed).expect("a key pair")
}

fn public(seed: [u8; 32]) -> ReleasePublicKey {
  let bytes: [u8; 32] = pair(seed).public_key().as_ref().try_into().expect("32 bytes");
  ReleasePublicKey::from_bytes(bytes)
}

fn keys() -> AcceptedReleaseKeys {
  AcceptedReleaseKeys::new(vec![public(RELEASE_SEED)])
}

/// The installer's bytes: a few chunks' worth.
fn installer_bytes() -> Vec<u8> {
  (0..300_000_u32).map(|index| (index % 251) as u8).collect()
}

/// How the stand-in describes the offered installer.
struct Offer {
  /// The bytes the size, the SHA-256 and the signature are of.
  bytes: Vec<u8>,
  /// The key that signs.
  seed: [u8; 32],
  /// Signs another name than the file's, so the signature does not verify.
  sign_other_name: bool,
  critical: bool,
}

impl Offer {
  fn of(bytes: &[u8]) -> Self {
    Self {
      bytes: bytes.to_vec(),
      seed: RELEASE_SEED,
      sign_other_name: false,
      critical: false,
    }
  }

  /// The update check's answer, offering [`NEWER`] with this file.
  fn answer(&self) -> Value {
    let sha256 = FileDigest::of(&self.bytes);
    let size = u64::try_from(self.bytes.len()).expect("a small file");
    let signed_name = if self.sign_other_name {
      "Orchester-0.4.0-other.exe"
    } else {
      INSTALLER
    };
    let signed = SignedFile {
      version: version(NEWER),
      platform: ReleasePlatform::Windows,
      arch: ReleaseArch::X86_64,
      name: ReleaseFileName::parse(signed_name).expect("a file name"),
      size,
      sha256,
    };
    let signature: [u8; 64] = pair(self.seed)
      .sign(&signed.message())
      .as_ref()
      .try_into()
      .expect("64 bytes");
    json!({ "update": {
      "version": NEWER,
      "channel": "stable",
      "notes": "Runs start faster.",
      "critical": self.critical,
      "published_at": 1_790_000_000_000_i64,
      "files": [{
        "name": INSTALLER,
        "platform": "windows",
        "arch": "x86_64",
        "url": DOWNLOAD,
        "size": size,
        "sha256": sha256.to_hex(),
        "key_id": public(self.seed).id().to_hex(),
        "signature": FileSignature::from_bytes(signature).to_hex(),
      }],
    }})
  }
}

fn is_check(asked: &Asked) -> bool {
  asked.line.starts_with("GET /v1/updates/check?")
}

/// A stand-in cloud that offers `offer` and serves `bytes` as its file.
fn offering(offer: Offer, bytes: Vec<u8>) -> Stand {
  Stand::start(move |asked| {
    if is_check(asked) {
      Answer::json(&offer.answer())
    } else {
      Answer::file(asked, &bytes)
    }
  })
}

/// A copy of the app with its own database, using the cloud at `api`.
struct Copy {
  dir: TempDir,
  workspace: Workspace,
  updates: AppUpdates,
}

impl Copy {
  fn installed(api: &str) -> Self {
    Self::new(api, ThisCopy::Installer)
  }

  fn new(api: &str, copy: ThisCopy) -> Self {
    let dir = tempfile::tempdir().expect("a folder");
    let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("a database");
    let address = StoredCloudAddress {
      web: "https://orchester.cloud".to_owned(),
      api: Some(api.to_owned()),
    };
    db.set_cloud_address(Some(&address)).expect("the cloud's address");
    let workspace = Workspace::new(db);
    let app = ThisApp {
      version: version(RUNNING),
      target: Some((ReleasePlatform::Windows, ReleaseArch::X86_64)),
      copy,
    };
    let updates = AppUpdates::new(workspace.clone(), app, keys(), dir.path());
    Self {
      dir,
      workspace,
      updates,
    }
  }

  fn folder(&self) -> PathBuf {
    self.dir.path().join(UPDATES_FOLDER)
  }

  /// The names of the files in the updates folder, sorted.
  fn downloads(&self) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(self.folder()) else {
      return Vec::new();
    };
    let mut names: Vec<String> = entries
      .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
      .collect();
    names.sort();
    names
  }
}

/// An API address nothing listens at.
fn nobody() -> String {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
  let address = listener.local_addr().expect("address");
  drop(listener);
  format!("http://{address}")
}

fn written(path: &Path, bytes: &[u8]) {
  std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
  std::fs::write(path, bytes).expect("written");
}

fn now() -> Timestamp {
  Timestamp::now()
}
