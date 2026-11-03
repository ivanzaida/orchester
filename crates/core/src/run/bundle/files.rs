//! The files an execution bundle names: each skill's reference files by path, and every file once by its hash, which
//! is what a bundle file read is asked for. The bytes themselves are read apart from the bundle.

use serde::{Deserialize, Serialize};

use super::canonical::file_sha256;
use crate::{blob::ContentHash, id::ResourceId, resource::SkillFilePath};

/// The most bytes one bundle file may have, and so the most a bundle file read answers with: 8 MiB. A run whose
/// skills have a larger file makes no bundle ([`super::BundleError::FileTooLarge`]).
pub const MAX_BUNDLE_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// `{sha256, size_bytes}`: one file's content.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {
  pub sha256: ContentHash,
  pub size_bytes: u64,
}

impl BundleFile {
  /// Whether `bytes` are this file's content: its size and its hash.
  pub fn matches(&self, bytes: &[u8]) -> bool {
    u64::try_from(bytes.len()).is_ok_and(|size| size == self.size_bytes) && file_sha256(bytes) == self.sha256
  }
}

/// `{path, sha256, size_bytes}`: a skill's reference file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillFileEntry {
  pub path: SkillFilePath,
  pub sha256: ContentHash,
  pub size_bytes: u64,
}

impl SkillFileEntry {
  pub fn file(&self) -> BundleFile {
    BundleFile {
      sha256: self.sha256.clone(),
      size_bytes: self.size_bytes,
    }
  }
}

/// `{resource_id, files}`: a pinned skill's reference files, in path order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillFiles {
  pub resource_id: ResourceId,
  pub files: Vec<SkillFileEntry>,
}
