//! One release file with its declaration: name, size, SHA-256, and the release key's signature over the signed message.

use std::{fs, ops::Range, path::Path};

use orchester_api::releases::{
  DeclaredFile, FileDigest, FileUpload, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleaseSigningKey,
  ReleaseVersion, SignedFile,
};

use crate::PublishError;

/// A release file's bytes and what CI declares for it.
pub struct ReleaseFile {
  declared: DeclaredFile,
  bytes: Vec<u8>,
}

impl ReleaseFile {
  /// The file at `path`, named as its file name, signed with `key` as a file of `version` for `platform` and `arch`.
  pub fn read(
    path: &Path,
    version: &ReleaseVersion,
    platform: ReleasePlatform,
    arch: ReleaseArch,
    key: &ReleaseSigningKey,
  ) -> Result<Self, PublishError> {
    let name = path
      .file_name()
      .and_then(|name| name.to_str())
      .and_then(|name| ReleaseFileName::parse(name).ok())
      .ok_or_else(|| PublishError::Name { path: path.to_owned() })?;
    let bytes = fs::read(path).map_err(|source| PublishError::Read {
      path: path.to_owned(),
      source,
    })?;
    Ok(Self::sign(name, bytes, version, platform, arch, key))
  }

  /// `bytes` as the file `name` of `version` for `platform` and `arch`, signed with `key`.
  pub fn sign(
    name: ReleaseFileName,
    bytes: Vec<u8>,
    version: &ReleaseVersion,
    platform: ReleasePlatform,
    arch: ReleaseArch,
    key: &ReleaseSigningKey,
  ) -> Self {
    let signed = SignedFile {
      version: version.clone(),
      platform,
      arch,
      name,
      size: bytes.len() as u64,
      sha256: FileDigest::of(&bytes),
    };
    let signature = key.sign(&signed);
    let declared = DeclaredFile {
      name: signed.name,
      platform,
      arch,
      size: signed.size,
      sha256: signed.sha256,
      key_id: key.id(),
      signature,
    };
    Self { declared, bytes }
  }

  pub fn declared(&self) -> &DeclaredFile {
    &self.declared
  }

  pub fn name(&self) -> &ReleaseFileName {
    &self.declared.name
  }

  /// The bytes of each part `upload` cuts the file into, after checking that it fits the file.
  pub(crate) fn parts(&self, upload: &FileUpload) -> Result<Vec<(u32, &[u8])>, PublishError> {
    let unexpected = |detail: String| PublishError::UnexpectedUpload {
      name: self.declared.name.clone(),
      detail,
    };
    if upload.name != self.declared.name || upload.size != self.declared.size {
      return Err(unexpected(format!(
        "it is {} of {} bytes, the file {} bytes",
        upload.name, upload.size, self.declared.size
      )));
    }
    if upload.part_size == 0 || upload.size.div_ceil(upload.part_size).max(1) != u64::from(upload.parts) {
      return Err(unexpected(format!(
        "{} parts of {} bytes do not cut {} bytes",
        upload.parts, upload.part_size, upload.size
      )));
    }
    upload
      .missing()
      .into_iter()
      .map(|part| {
        let range = part_range(part, upload.part_size, upload.size)
          .ok_or_else(|| unexpected(format!("part {part} lies outside the file")))?;
        Ok((part, &self.bytes[range]))
      })
      .collect()
  }
}

/// The bytes of part `part`: `part_size` from `part * part_size`, the last one the rest.
fn part_range(part: u32, part_size: u64, size: u64) -> Option<Range<usize>> {
  let start = u64::from(part).checked_mul(part_size)?;
  if start >= size {
    return None;
  }
  let end = start.saturating_add(part_size).min(size);
  Some(usize::try_from(start).ok()?..usize::try_from(end).ok()?)
}
