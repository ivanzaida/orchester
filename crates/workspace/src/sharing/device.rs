//! How a server lists this computer's session (10.01 *Your sessions*): the computer's name and its
//! platform, sent as the sign-in's `device` (`BOHDAN-PC · Windows`) and read back from the session
//! list. A device another client named some other way is shown as it is.

/// Between the computer's name and the platform in a device text.
const SEPARATOR: &str = " · ";

/// A session's device as the session list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device {
  /// Orchester on `computer`, running on `platform`.
  Orchester { computer: String, platform: String },
  /// A device text this format does not describe.
  Other(String),
}

impl Device {
  /// Reads a session's device text.
  pub fn parse(text: &str) -> Self {
    match text.rsplit_once(SEPARATOR) {
      Some((computer, platform)) if !computer.is_empty() && !platform.is_empty() => Self::Orchester {
        computer: computer.to_owned(),
        platform: platform.to_owned(),
      },
      _ => Self::Other(text.to_owned()),
    }
  }
}

/// The device text of this computer: its name and platform.
pub fn this_computer() -> String {
  format!("{}{SEPARATOR}{}", platform::computer_name(), platform::NAME)
}

#[cfg(windows)]
mod platform {
  pub const NAME: &str = "Windows";

  /// The NetBIOS name Windows gives every process in `COMPUTERNAME`.
  pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows".to_owned())
  }
}

#[cfg(target_os = "macos")]
mod platform {
  pub const NAME: &str = "macOS";

  /// The host name without the `.local` of Bonjour, e.g. `bohdan-mbp`.
  pub fn computer_name() -> String {
    let name = rustix::system::uname();
    let name = name.nodename().to_string_lossy();
    name.trim_end_matches(".local").to_owned()
  }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
  pub const NAME: &str = "Linux";

  pub fn computer_name() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "Linux".to_owned())
  }
}
