//! Which team scenario a QA copy opens on. The seed knows the scenario and the QA app does not (`tools/qa-app.ps1`
//! tells it only whether to sign in), so the seed leaves the state's name in a marker beside the application database.
//! A marker that names nothing known is an error, not an app without its team. The seed includes this file too.

use std::{error::Error, fs, io::ErrorKind, path::Path};

/// The marker's file, beside the application database.
pub const FILE: &str = "team-qa";

/// What the QA copy does before its window shows, named as the seed's scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  /// *Join a team* with a code, the stand-in's address typed.
  Join,
  /// The code a person typed was wrong.
  WrongCode,
  /// The code a person typed has expired.
  ExpiredCode,
  /// Nothing answers at the address a person typed.
  Unreachable,
  /// A person's code joined the team.
  Joined,
  /// Joined, with Kontur linked and downloaded and Billing not on this computer yet.
  Offer,
  /// Joined, with Kontur linked and its tasks coming down.
  Downloading,
  /// Joined, with Kontur linked and the download of its tasks refused.
  DownloadFailed,
  /// Joined, with Kontur linked, its `origin` the project's, and 84 of its 212 tasks down, the rest held (`YN6MO`).
  DownloadProgress,
  /// As [`Self::DownloadProgress`], with the download stopped there by the server's refusal.
  DownloadStopped,
}

impl State {
  pub const ALL: [Self; 10] = [
    Self::Join,
    Self::WrongCode,
    Self::ExpiredCode,
    Self::Unreachable,
    Self::Joined,
    Self::Offer,
    Self::Downloading,
    Self::DownloadFailed,
    Self::DownloadProgress,
    Self::DownloadStopped,
  ];

  /// The seed's scenario of this state.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Join => "team-join",
      Self::WrongCode => "team-join-wrong-code",
      Self::ExpiredCode => "team-join-expired-code",
      Self::Unreachable => "team-join-unreachable",
      Self::Joined => "team-join-joined",
      Self::Offer => "team-offer",
      Self::Downloading => "team-downloading",
      Self::DownloadFailed => "team-download-failed",
      Self::DownloadProgress => "team-download-progress",
      Self::DownloadStopped => "team-download-stopped",
    }
  }

  /// Whether the QA copy presses in the window before it shows: *Join with the code* is a person's, which an MCP client
  /// cannot press.
  pub const fn drives(self) -> bool {
    matches!(
      self,
      Self::Join | Self::WrongCode | Self::ExpiredCode | Self::Unreachable | Self::Joined
    )
  }

  /// The state the marker beside the application database at `database` names; `None` without a marker.
  pub fn beside(database: &Path) -> Result<Option<Self>, Box<dyn Error>> {
    let marker = database.with_file_name(FILE);
    let name = match fs::read_to_string(&marker) {
      Ok(name) => name,
      Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(error.into()),
    };
    Self::ALL
      .into_iter()
      .find(|state| state.name() == name)
      .map(Some)
      .ok_or_else(|| format!("{} names no team state: {name}", marker.display()).into())
  }
}
