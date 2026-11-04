//! The app release contract (`docs/cloud-api.md`, "App releases"): the signed message's bytes, the key ids and the
//! rollout hash against fixed vectors computed apart from this crate, the shapes' JSON, and the client's requests.
//!
//! `cargo test -p orchester-api --features client,release-signature --test releases releases`

mod admin;
mod client;
mod format;
mod shapes;
mod stable;
mod stand;
mod versions;

use orchester_api::releases::{FileDigest, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleaseVersion, SignedFile};

/// RFC 8032's first Ed25519 test key, as release key in these tests.
const SEED: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
const PUBLIC: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
/// SHA-256(`"orchester-release-key\0"` ‖ [`PUBLIC`]), first 8 bytes.
const KEY_ID: &str = "a594e17b5a19c648";
/// SHA-256 of `orchester release vector`.
const DIGEST: &str = "1055256c88ce265e44b7f378877ffe1c8e85c7670c4fa552f69f2e691a682490";
/// [`SEED`]'s signature of [`vector`]'s message.
const SIGNATURE: &str = "964d975ae3bc236c01e2496a3506d756c98e4c5ff549210838e4e8c476e3c44982bbd096c77bef09bd20bebf1b32652f1f40ef8bcd8a432d5bcac2cb82f01e04";

fn version(text: &str) -> ReleaseVersion {
  ReleaseVersion::parse(text).expect("a version")
}

fn file_name(text: &str) -> ReleaseFileName {
  ReleaseFileName::parse(text).expect("a file name")
}

/// The file the vectors sign.
fn vector() -> SignedFile {
  SignedFile {
    version: version("0.4.0"),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    name: file_name("Orchester-0.4.0-windows-x86_64.msi"),
    size: 48_213_504,
    sha256: FileDigest::parse(DIGEST).expect("a digest"),
  }
}
