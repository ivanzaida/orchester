//! CI's upload of a release (`docs/cloud-api.md`, "Uploading a release", token scope `releases:write`): it creates the
//! release, declares each file with its signature, sends the file in parts, and finalizes. Finalizing checks every file
//! against its declaration and signature and answers which were accepted.

use serde::{Deserialize, Serialize};

use super::{
  CiRun, FileDigest, FileSignature, ReleaseArch, ReleaseFileName, ReleaseKeyId, ReleasePlatform, ReleaseVersion,
  SignedFile,
};

/// `POST /releases`: a new release, a draft until it is finalized.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewRelease {
  pub version: ReleaseVersion,
  /// Release notes in Markdown, at most 64 KiB.
  pub notes: String,
  #[serde(default)]
  pub critical: bool,
  /// The CI run that uploads it. Left out when `None`, because a cloud before this field refuses it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ci_run: Option<CiRun>,
}

/// `POST /releases/{version}/files`: one file of a draft and what CI signed for it. Declaring the same file again
/// answers its upload as it stands, so an interrupted upload resumes; declaring another file under the same name is
/// refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredFile {
  pub name: ReleaseFileName,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub size: u64,
  pub sha256: FileDigest,
  pub key_id: ReleaseKeyId,
  pub signature: FileSignature,
}

impl DeclaredFile {
  /// What the declaration says was signed, as a file of the release `version`.
  pub fn signed(&self, version: &ReleaseVersion) -> SignedFile {
    SignedFile {
      version: version.clone(),
      platform: self.platform,
      arch: self.arch,
      name: self.name.clone(),
      size: self.size,
      sha256: self.sha256,
    }
  }
}

/// A file's upload as it stands: `201` of a first declaration, `200` of a repeated one and of each part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileUpload {
  pub name: ReleaseFileName,
  pub size: u64,
  /// Bytes in every part but the last, which holds the rest: part `i` is bytes `i * part_size ..`.
  pub part_size: u64,
  /// How many parts the file has: `size / part_size`, rounded up (at least one).
  pub parts: u32,
  /// The parts the cloud has, in ascending order; the rest are still to send.
  pub received: Vec<u32>,
}

impl FileUpload {
  /// The parts still to send, in ascending order.
  pub fn missing(&self) -> Vec<u32> {
    (0..self.parts)
      .filter(|part| self.received.binary_search(part).is_err())
      .collect()
  }
}

/// `200` of `POST /releases/{version}/finalize`: the verdict on each declared file. The release is published (on Beta)
/// only when every file was accepted; otherwise it stays a draft, the rejected files' parts are dropped, and CI
/// declares and sends them again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verification {
  pub version: ReleaseVersion,
  pub files: Vec<FileVerdict>,
}

impl Verification {
  /// Whether every file was accepted, so the release is published.
  pub fn accepted(&self) -> bool {
    self.files.iter().all(|file| file.result == FileResult::Accepted)
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileVerdict {
  pub name: ReleaseFileName,
  #[serde(flatten)]
  pub result: FileResult,
}

/// What finalizing found for one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum FileResult {
  Accepted,
  Rejected { reason: Rejection },
}

/// Why finalizing rejected a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
  /// Not every part arrived.
  Incomplete,
  /// The bytes' SHA-256 is not the declared one.
  Sha256Mismatch,
  /// The signature names a key the cloud does not accept.
  UnknownKey,
  /// The named key does not verify the signature over the declared file's message.
  SignatureMismatch,
}
