//! Settings › Browser of the computer a runner runs on (protocol 47, `docs/architecture/remote-runners.md`, the
//! computer's resources): the desktop reads them from the application database and passes them to the runner when it
//! attaches ([`crate::Command::Attach`]) and again whenever they change ([`crate::Command::SetBrowser`]), so the
//! runner never opens `orchester.db` itself. The next start of the project's browser uses what was passed last.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The browser program agents' pages open in and where the projects' browser profiles are, as the desktop read them;
/// each part with the reason it could not be read instead, which the browser then answers with when it cannot start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerBrowser {
  /// The program the person chose in Settings › Browser; `None` for the one installed.
  pub program: Result<Option<PathBuf>, String>,
  /// The folder holding one browser profile per project, named by the project's id: `<application data>/browser/`
  /// beside `orchester.db`, where the cookies and sign-ins are.
  pub profiles: Result<PathBuf, String>,
}

impl ComputerBrowser {
  /// The one installed, with the projects' profiles in `profiles`.
  pub const fn installed(profiles: PathBuf) -> Self {
    Self {
      program: Ok(None),
      profiles: Ok(profiles),
    }
  }
}
