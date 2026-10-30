//! Why publishing a release failed. No variant holds the signing key or the release token.

use std::{io, path::PathBuf};

use orchester_api::{
  client::ClientError,
  releases::{FileResult, FileVerdict, ReleaseFileName},
};

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
  #[error("cannot read {}: {source}", path.display())]
  Read { path: PathBuf, source: io::Error },
  /// The file's name is not one the release contract takes.
  #[error("{} is not a release file name (ASCII letters, digits, `.`, `_`, `+`, `-`)", path.display())]
  Name { path: PathBuf },
  /// A call to the cloud failed; `step` says which.
  #[error("{step}: {source}")]
  Cloud { step: String, source: ClientError },
  /// The cloud's answer to a declaration does not fit the file declared, so its parts cannot be cut.
  #[error("the cloud's upload of {name} does not fit the file: {detail}")]
  UnexpectedUpload { name: ReleaseFileName, detail: String },
  /// After the last part the cloud still misses some.
  #[error("the cloud still misses parts {missing:?} of {name}")]
  PartsMissing { name: ReleaseFileName, missing: Vec<u32> },
  /// Finalizing did not accept every file; the release stays a draft.
  #[error("the cloud did not accept the release: {}", describe(.0))]
  Rejected(Vec<FileVerdict>),
}

impl PublishError {
  pub(crate) fn cloud(step: impl Into<String>) -> impl FnOnce(ClientError) -> Self {
    let step = step.into();
    move |source| Self::Cloud { step, source }
  }
}

fn describe(verdicts: &[FileVerdict]) -> String {
  verdicts
    .iter()
    .map(|verdict| match verdict.result {
      FileResult::Accepted => format!("{} accepted", verdict.name),
      FileResult::Rejected { reason } => format!("{} rejected ({reason:?})", verdict.name),
    })
    .collect::<Vec<_>>()
    .join(", ")
}
