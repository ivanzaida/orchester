//! The Chromium-based browser Orchester's agents browse with (`docs/design/browser.md`, "The browser process"): which
//! one is installed ([`find`]), the command that starts it headless on a project's own profile folder ([`headless`]),
//! and the debugging address it writes into that folder once it listens ([`DevToolsActivePort`]). Orchester ships no
//! browser: none found is an explicit [`BrowserUnavailable`] with its reason, never a silent gap.
//!
//! - Windows (`windows`): Microsoft Edge, then Google Chrome, from the registry's `App Paths` and the usual install
//!   folders.
//! - macOS ([`macos`]): Google Chrome, Microsoft Edge, Brave, Chromium in `/Applications` and `~/Applications`.
//! - Other platforms: unavailable, saying so.

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::command::Command;
use thiserror::Error;

#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use self::unsupported as platform;
#[cfg(target_os = "windows")]
use self::windows as platform;

/// The code every browser tool answers with while no browser can be used.
pub const BROWSER_UNAVAILABLE: &str = "browser_unavailable";
/// The file Chromium writes into its profile folder once its debugging port listens.
pub const DEVTOOLS_ACTIVE_PORT: &str = "DevToolsActivePort";

/// A Chromium-based browser's program, found installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserProgram {
  name: &'static str,
  path: PathBuf,
}

impl BrowserProgram {
  /// The browser's product name, such as `Google Chrome`.
  pub const fn name(&self) -> &'static str {
    self.name
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

/// Why no browser can be used, said to the person and answered to every browser tool as [`BROWSER_UNAVAILABLE`].
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{BROWSER_UNAVAILABLE}: {reason}")]
pub struct BrowserUnavailable {
  reason: String,
}

impl BrowserUnavailable {
  pub fn new(reason: impl Into<String>) -> Self {
    Self { reason: reason.into() }
  }

  pub fn reason(&self) -> &str {
    &self.reason
  }
}

/// The browser installed on this computer, in the platform's order of preference. Blocks: on Windows it reads the
/// registry through `reg.exe`.
pub fn find() -> Result<BrowserProgram, BrowserUnavailable> {
  platform::find()
}

/// The first of `candidates`, each a browser's name and where its program would be, whose program is a file;
/// unavailable when none is, naming the browsers looked for and `looked`, where.
pub fn first_installed(
  candidates: impl IntoIterator<Item = (&'static str, PathBuf)>,
  looked: &str,
) -> Result<BrowserProgram, BrowserUnavailable> {
  let mut names: Vec<&str> = Vec::new();
  for (name, path) in candidates {
    if path.is_file() {
      return Ok(BrowserProgram { name, path });
    }
    if !names.contains(&name) {
      names.push(name);
    }
  }
  Err(BrowserUnavailable::new(format!(
    "no Chromium-based browser is installed: looked for {} in {looked}",
    names.join(", ")
  )))
}

/// The command that starts `program` headless on `profile`, the project's own profile folder, with its debugging port
/// on a free port of the loopback interface, which it writes into [`DEVTOOLS_ACTIVE_PORT`]; it opens on a blank page.
/// Refused for a folder whose path is not Unicode, which the browser's arguments could not name exactly.
pub fn headless(program: &BrowserProgram, profile: &Path) -> Result<Command, BrowserUnavailable> {
  let folder = profile.to_str().ok_or_else(|| {
    BrowserUnavailable::new(format!(
      "the browser profile folder {} is not a Unicode path",
      profile.display()
    ))
  })?;
  Ok(Command::new(
    program.path(),
    [
      "--headless=new".to_owned(),
      format!("--user-data-dir={folder}"),
      "--remote-debugging-port=0".to_owned(),
      "--no-first-run".to_owned(),
      "--no-default-browser-check".to_owned(),
      "--disable-extensions".to_owned(),
      "--disable-sync".to_owned(),
      "--mute-audio".to_owned(),
      "about:blank".to_owned(),
    ],
  ))
}

/// Where a started browser listens for its DevTools Protocol connection, as it wrote it into its profile folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevToolsActivePort {
  port: u16,
  path: String,
}

impl DevToolsActivePort {
  /// The file's `text`: the port on its first line, the browser's WebSocket path (`/devtools/browser/<id>`) on its
  /// second. `None` for anything else, such as a file still being written.
  pub fn parse(text: &str) -> Option<Self> {
    let mut lines = text.lines();
    let port = lines.next()?.trim().parse::<u16>().ok().filter(|port| *port != 0)?;
    let path = lines.next()?.trim();
    if !path.starts_with("/devtools/browser/") || path.contains(char::is_whitespace) {
      return None;
    }
    Some(Self {
      port,
      path: path.to_owned(),
    })
  }

  /// What the browser on `profile` wrote; `None` while the file is missing or not complete.
  pub fn read(profile: &Path) -> io::Result<Option<Self>> {
    match fs::read_to_string(profile.join(DEVTOOLS_ACTIVE_PORT)) {
      Ok(text) => Ok(Self::parse(&text)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(error),
    }
  }

  /// Removes what an earlier browser on `profile` wrote, so a start reads only its own browser's address.
  pub fn remove(profile: &Path) -> io::Result<()> {
    match fs::remove_file(profile.join(DEVTOOLS_ACTIVE_PORT)) {
      Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
      _ => Ok(()),
    }
  }

  pub const fn port(&self) -> u16 {
    self.port
  }

  /// The browser's WebSocket address on the loopback interface.
  pub fn websocket_url(&self) -> String {
    format!("ws://127.0.0.1:{}{}", self.port, self.path)
  }
}

/// The default value in the answer of `reg query <key> /ve`: the text after its type (`REG_SZ`, `REG_EXPAND_SZ`) on
/// the value's line, whatever the language of the value's name, without surrounding quotes. `None` for an answer
/// without one.
pub fn registry_default_value(answer: &str) -> Option<PathBuf> {
  answer.lines().find_map(|line| {
    let (_, value) = line.split_once("REG_EXPAND_SZ").or_else(|| line.split_once("REG_SZ"))?;
    let value = value.trim().trim_matches('"');
    (!value.is_empty()).then(|| PathBuf::from(value))
  })
}
