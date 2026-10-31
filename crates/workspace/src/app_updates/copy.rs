//! Which copies of the app update themselves: only one the release's installer installed on Windows, found by the
//! installer's uninstaller beside the program (Inno Setup's `unins000.exe` with its `unins000.dat`,
//! `tools/installer/orchester.iss`). A development build, a QA copy, `tools/install.ps1`'s copy and the portable zip
//! (its folder holds the zip's `README.txt`) are updated by hand; on macOS no copy updates itself yet.

use std::{fs, path::Path};

use orchester_api::releases::ReleasePlatform;

use crate::build::Build;

/// The file the portable zip puts beside the programs (`tools/installer/portable-readme.txt`).
const PORTABLE_README: &str = "README.txt";

/// What this copy of the app is, for its updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThisCopy {
  /// Installed by the release's installer on Windows: it downloads and installs its updates.
  Installer,
  /// It does not update itself, for this reason.
  NotThisCopy(CopyReason),
  /// Updates are not automatic on this platform yet (macOS).
  NotOnThisPlatform,
}

/// Why a copy does not update itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyReason {
  /// A build other than a release's (debug, `cargo run --release`, an example).
  Development,
  /// A QA copy: started with the app's MCP discovery folder.
  Qa,
  /// The copy `tools/install.ps1` built and copied.
  ScriptInstall,
  /// The release's portable zip.
  Portable,
  /// The program cannot tell its own folder.
  ProgramFolderUnknown,
}

impl ThisCopy {
  /// The copy a `build` is, started as a QA copy or not, on `platform`, from the program folder `folder`.
  pub fn of(build: Build, qa: bool, platform: ReleasePlatform, folder: Option<&Path>) -> Self {
    if qa {
      return Self::NotThisCopy(CopyReason::Qa);
    }
    if build == Build::Other {
      return Self::NotThisCopy(CopyReason::Development);
    }
    if platform == ReleasePlatform::Macos {
      return Self::NotOnThisPlatform;
    }
    let Some(folder) = folder else {
      return Self::NotThisCopy(CopyReason::ProgramFolderUnknown);
    };
    if has_uninstaller(folder) {
      Self::Installer
    } else if folder.join(PORTABLE_README).is_file() {
      Self::NotThisCopy(CopyReason::Portable)
    } else {
      Self::NotThisCopy(CopyReason::ScriptInstall)
    }
  }

  pub const fn updates_itself(self) -> bool {
    matches!(self, Self::Installer)
  }
}

/// Whether Inno Setup's uninstaller is in `folder`: `unins` and three digits, as `.exe` with its `.dat` beside it.
fn has_uninstaller(folder: &Path) -> bool {
  let entries = match fs::read_dir(folder) {
    Ok(entries) => entries,
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "cannot read the program's folder; no installer found");
      return false;
    }
  };
  entries.flatten().any(|entry| {
    let name = entry.file_name();
    let Some(stem) = name.to_str().and_then(|name| name.strip_suffix(".exe")) else {
      return false;
    };
    let numbered = stem
      .strip_prefix("unins")
      .is_some_and(|digits| digits.len() == 3 && digits.bytes().all(|byte| byte.is_ascii_digit()));
    numbered && folder.join(format!("{stem}.dat")).is_file()
  })
}

/// The platform this program was built for; `None` on one releases do not ship for.
pub const fn this_platform() -> Option<ReleasePlatform> {
  if cfg!(target_os = "windows") {
    Some(ReleasePlatform::Windows)
  } else if cfg!(target_os = "macos") {
    Some(ReleasePlatform::Macos)
  } else {
    None
  }
}
