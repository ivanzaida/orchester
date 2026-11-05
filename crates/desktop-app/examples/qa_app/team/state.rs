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
  /// Joined, with Kontur and Billing to pick and nothing linked; the seed leaves folders to link them to.
  SharedProjects,
  /// Joined, with no project the person is invited to.
  NoSharedProjects,
  /// Joined, with one project per way a clone from *Link a folder* ends, each cloned by a stand-in console (ORC-584).
  Clones,
  /// Joined as a member, with Kontur linked and its tasks read from the server (ORC-716, 01 `QdL7N`).
  Member,
  /// As [`Self::Member`], standing as an Admin of Kontur (ORC-716, 07 `BLTsP`).
  Admin,
  /// As [`Self::Member`], standing as the server's admin, no member of Kontur (ORC-716, 07 `BLTsP`).
  ServerAdmin,
  /// As [`Self::Member`], with the server's reads unavailable (ORC-716, 06 `GR3RS`).
  Outage,
  /// As [`Self::Outage`], with two of Kontur's tasks running on this computer (ORC-716, 06 `XTBXg`).
  OutageRunning,
  /// As [`Self::Member`], until the tasks were read and shown: then the server stops answering (ORC-716, 06 `GR3RS`).
  OutageLater,
  /// As [`Self::Member`], with KON-12 and KON-21 running on this computer, KON-12 claimed by the person on this
  /// computer and assigned to them (ORC-717, 01c `Tb55G`).
  MemberRunning,
  /// As [`Self::MemberRunning`], until a module's page read its module: then the server stops answering (ORC-717, 06d
  /// `B7Mbe`).
  ModuleOutage,
  /// A member, with a task in every claim state of `02h` `xUwYS` and rows 2–3 (ORC-732): KON-12 claimed here and
  /// running, KON-21 waiting for the person, KON-14 in review, KON-11 taken over by mara, KON-23 moved to the person's
  /// other computer, KON-24 the person's stale claim, KON-13 mara's, KON-10 the person's on another computer, KON-19
  /// under olha's module claim, KON-16 mara's stale claim (whose take over she renews first, 02f) and KON-5 nobody's.
  Claims,
  /// A member on the modules (ORC-732, 04d `PqVUn`): olha's stale claim on Webhooks v2, whose take over the server
  /// refuses while a task of it runs; Billing exports the person's on this computer, its delivery to ask for.
  ModuleWork,
  /// An Admin on the modules and Sharing (ORC-732, 04 `MvniR`, 04b–c, 04e, 07 `MnVch`, 05b): a branch change meets a
  /// taken branch, then running work, then someone else's change first; Automatic merge on, changed by someone else
  /// first; Billing exports the person's on this computer, its delivery to authorize.
  AdminWork,
  /// As [`Self::Member`], with the server refusing this version of Orchester (ORC-732, 07d `BDJKM`).
  VersionRefused,
  /// As [`Self::Member`], with KON-12's run interrupted at Implement, a team run under the claim this computer holds,
  /// which Continue run takes on with `orchester-fake-codex` (ORC-732, 01c `Tb55G` Running).
  TeamRunning,
  /// An Admin editing KON-12 (priority High): olha sets its priority to Urgent just before the person's first save,
  /// which then meets her change first (ORC-734, `Team authority · redesign` 05 `LmxUY`).
  EditConflict,
}

impl State {
  pub const ALL: [Self; 27] = [
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
    Self::SharedProjects,
    Self::NoSharedProjects,
    Self::Clones,
    Self::Member,
    Self::Admin,
    Self::ServerAdmin,
    Self::Outage,
    Self::OutageRunning,
    Self::OutageLater,
    Self::MemberRunning,
    Self::ModuleOutage,
    Self::Claims,
    Self::ModuleWork,
    Self::AdminWork,
    Self::VersionRefused,
    Self::TeamRunning,
    Self::EditConflict,
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
      Self::SharedProjects => "team-shared-projects",
      Self::NoSharedProjects => "team-shared-none",
      Self::Clones => "team-clone",
      Self::Member => "team-member",
      Self::Admin => "team-admin",
      Self::ServerAdmin => "team-server-admin",
      Self::Outage => "team-outage",
      Self::OutageRunning => "team-outage-running",
      Self::OutageLater => "team-outage-later",
      Self::MemberRunning => "team-member-running",
      Self::ModuleOutage => "team-module-outage",
      Self::Claims => "team-claims",
      Self::ModuleWork => "team-module-work",
      Self::AdminWork => "team-admin-work",
      Self::VersionRefused => "team-version-refused",
      Self::TeamRunning => "team-running",
      Self::EditConflict => "team-edit-conflict",
    }
  }

  /// Whether tasks of Kontur have runs on this computer, which the server names by their ids here.
  pub const fn runs_here(self) -> bool {
    matches!(
      self,
      Self::OutageRunning | Self::MemberRunning | Self::ModuleOutage | Self::Claims | Self::TeamRunning
    )
  }

  /// Whether the QA copy presses in the window before it shows: *Join with the code* and *Clone repository…* are a
  /// person's, which an MCP client cannot press.
  pub const fn drives(self) -> bool {
    matches!(
      self,
      Self::Join | Self::WrongCode | Self::ExpiredCode | Self::Unreachable | Self::Joined | Self::Clones
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
