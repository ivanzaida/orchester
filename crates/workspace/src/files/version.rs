//! Which version of the project's files the code viewer reads: the `at` of its route
//! (`docs/design/code-viewer.md`, decision 2).

use std::{fmt, str::FromStr};

use orchester_core::{id::RunId, revision::Revision};
use thiserror::Error;

/// Where a file is read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Version {
  /// The files in the project's folder.
  Checkout,
  /// The files in a run's worktree, while it exists.
  Run(RunId),
  /// A commit, e.g. one side of a diff.
  Revision(Revision),
  /// The tip of the branch the project's checkout has: where runs bring their work.
  Base,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("`{0}` names no version of the project's files")]
pub struct VersionParseError(pub String);

const CHECKOUT: &str = "checkout";
const BASE: &str = "base";
const RUN: &str = "run:";
const REVISION: &str = "rev:";

impl FromStr for Version {
  type Err = VersionParseError;

  /// `checkout`, `base`, `run:<run id>` or `rev:<commit>`.
  fn from_str(value: &str) -> Result<Self, Self::Err> {
    let invalid = || VersionParseError(value.to_owned());
    match value {
      CHECKOUT => Ok(Self::Checkout),
      BASE => Ok(Self::Base),
      _ => {
        if let Some(run) = value.strip_prefix(RUN) {
          return run.parse().map(Self::Run).map_err(|_| invalid());
        }
        let revision = value.strip_prefix(REVISION).ok_or_else(invalid)?;
        Revision::new(revision).map(Self::Revision).map_err(|_| invalid())
      }
    }
  }
}

impl fmt::Display for Version {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Checkout => f.write_str(CHECKOUT),
      Self::Base => f.write_str(BASE),
      Self::Run(run) => write!(f, "{RUN}{run}"),
      Self::Revision(revision) => write!(f, "{REVISION}{revision}"),
    }
  }
}
