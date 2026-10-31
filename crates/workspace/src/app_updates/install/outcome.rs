//! What the start after a handover finds ([`super::handover`]): the version it runs is the one installed, so the update
//! is done; or the installer failed and started the old version again, with why in its result file and its log.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_api::releases::ReleaseVersion;

use super::handover::{InstallRecord, RESULT_FILE};

/// What became of the update the previous start handed over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AfterInstall {
  /// This start runs the installed version.
  Updated(JustUpdated),
  /// The installer did not install it; this start runs the version that handed over.
  Failed {
    version: ReleaseVersion,
    failure: InstallFailure,
  },
}

/// The update this start finished, for the status bar's *Updated to* and What's new.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JustUpdated {
  pub version: ReleaseVersion,
  /// Its release notes in Markdown.
  pub notes: String,
}

/// Why an update did not install. The old version keeps running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallFailure {
  /// What went wrong, in the words of the OS or the installer: runtime data, logged as it is.
  pub reason: String,
  /// The installer's exit code, when the app saw it exit.
  pub exit_code: Option<i32>,
  /// The installer's log, when it wrote one.
  pub log: Option<PathBuf>,
}

/// Why the installer stopped, when it wrote nothing itself.
const STOPPED_UNSAID: &str = "the installer stopped before it finished";

/// What became of the update handed over from `data_folder`, for a start running `running`; `None` when nothing was
/// handed over. The note and the result are removed, so it is found once. Blocking.
pub fn after_install(data_folder: &Path, running: &ReleaseVersion) -> Option<AfterInstall> {
  let record = InstallRecord::take(data_folder)?;
  let result = take_result(&data_folder.join(RESULT_FILE));
  if record.to == *running {
    return Some(AfterInstall::Updated(JustUpdated {
      version: record.to,
      notes: record.notes,
    }));
  }
  let log = record.log.is_file().then_some(record.log);
  let reason = result.unwrap_or_else(|| STOPPED_UNSAID.to_owned());
  tracing::warn!(version = %record.to, %reason, "the update did not install; the version before it runs");
  Some(AfterInstall::Failed {
    version: record.to,
    failure: InstallFailure {
      reason,
      exit_code: None,
      log,
    },
  })
}

/// The installer's result, trimmed; `None` without one.
fn take_result(path: &Path) -> Option<String> {
  let text = match fs::read_to_string(path) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
    Err(error) => {
      tracing::warn!(%error, "cannot read why the installer failed");
      return None;
    }
  };
  if let Err(error) = fs::remove_file(path) {
    tracing::warn!(%error, "cannot remove the installer's result");
  }
  let text = text.trim();
  (!text.is_empty()).then(|| text.to_owned())
}
