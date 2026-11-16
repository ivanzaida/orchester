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
  /// An Admin on tasks the server records as closed or blocked, one of each way: Done by the person, by mara, with the
  /// delivery of Webhooks v2 and by its run, Canceled by dana-k, Blocked by the person and by mara; KON-1 is Done with
  /// no record (decision D4, `Task - v2` T4–T6, T11a–f).
  ClosedAdmin,
  /// As [`Self::ClosedAdmin`], the person a member: the closed tasks read with no Reopen and say who does.
  ClosedMember,
  /// As [`Self::Member`], with the server refusing this version of Orchester (ORC-732, 07d `BDJKM`).
  VersionRefused,
  /// As [`Self::Member`], with KON-12's run interrupted at Implement, a team run under the claim this computer holds,
  /// which Continue run takes on with `orchester-fake-codex` (ORC-732, 01c `Tb55G` Running).
  TeamRunning,
  /// An Admin editing KON-12 (priority High): olha sets its priority to Urgent just before the person's first save,
  /// which then meets her change first (ORC-734, `Team authority · redesign` 05 `LmxUY`).
  EditConflict,
  /// As [`Self::Member`], on the team's resources of `picker-readiness-team`: the server's tasks are on Team delivery
  /// and Team quick fix, and Team delivery is not ready on this computer (ORC-840, `Task · Run · not ready here`
  /// R1 `Ku0qu`).
  NotReadyHere,
  /// As [`Self::NotReadyHere`], with a Claude Code connection, and KON-5 having a run on this computer that was
  /// cancelled (run 8) while mara's claim on it is stale: *Take over* starts a new run, which is refused as not ready
  /// here, and the header's line says when run 8 ended (ORC-840b, `Status line` `g9ufW`).
  NotReadyAfterRun,
  /// As [`Self::NotReadyHere`], with every task on Team quick fix, which is ready here, and the server a release ahead
  /// of this computer: a start builds its bundle in a schema this computer does not read, so a start fails for good,
  /// releases the claim and the task page says why (ORC-885).
  StartFailed,
  /// The Agents hub of `team-hub` and its variants as an Admin: the stand-in team server holds every resource of the
  /// team the seed cached, at the version it cached, so the window reads the team's configuration from it and shows the
  /// hub's normal states (`docs/design/team-configuration-live.md`).
  TeamHub,
  /// As [`Self::TeamHub`], for a member who is not an admin (`team-hub-member` and its variants).
  TeamHubMember,
  /// As [`Self::TeamHub`], the team server not answering since the join, so this computer never read the team's
  /// configuration: the hub says it cannot reach the team server, with *Try again* (`A25` `Zhcif`).
  TeamHubDown,
  /// An Admin on the Agents hub's team MCP servers, which the stand-in team server holds too (`Agents · MCP server ·
  /// admin · draft` A13 `WMtxm`, ORC-836): *Edit* a server, type, *Save* publishes its next version at once. The
  /// server also holds an MCP server named `figma-local` that this computer has not read, so *New MCP server* with
  /// that name is refused as taken (A15 `dLqHC`).
  TeamEdit,
  /// As [`Self::TeamEdit`], the server no longer taking this person's writes: *Save* is refused as no longer an admin,
  /// in place, and what was typed is kept.
  TeamEditDemoted,
  /// As [`Self::TeamEdit`], the server not answering once this computer read it: the hub shows what it read and says it
  /// cannot reach the team, *Edit* is not offered, and nothing waits to be sent.
  TeamEditOffline,
  /// As [`Self::TeamEdit`], the server rejecting the content of every version sent: *Save* says why and keeps what was
  /// typed (A17 `rJbPm`).
  TeamEditRejects,
  /// As [`Self::TeamEdit`], after a *Save* the server accepted: v2 is the team's (A13 `WMtxm`'s result). Made without a
  /// press.
  TeamEditPublished,
  /// As [`Self::TeamEdit`], after a *Save* the server refused as no longer an admin, and the next read made the person
  /// a member: the page is a member's. Made without a press.
  TeamEditRefusedAdmin,
  /// As [`Self::TeamEdit`], the server `pencil-design` needing a variable, `PENCIL_TOKEN`: the edit form has its
  /// *Variables it needs* row, and the panel the secret to fill in (A13 `WMtxm`).
  TeamEditVariables,
  /// As [`Self::StartFailed`], with the server not answering a Run's `work.start` for [`START_ANSWERS_AFTER`] after the
  /// window opens, and then answering as a server a release ahead: the start waits, the project's recovery carries it
  /// on, the start fails for good, and the task page, which said the run starts once they answer, says why it did not
  /// (ORC-885 follow-up).
  StartRecovered,
  /// As [`Self::StartFailed`], with KON-5's start already recorded as a server that does not answer leaves it, and the
  /// bundle never answered: the task page says the run is starting, in an information notice (ORC-885c).
  StartWaiting,
  /// As [`Self::StartFailed`], with KON-5's start recorded at the runner, which refuses it when recovery asks, so the
  /// task page says what the runner said without a press of Run (ORC-885c).
  StartRunnerRefused,
  /// As [`Self::StartFailed`], with KON-5's start recorded as an earlier window left it, failed for good and not
  /// dismissed: the task page tells it once (ORC-885c).
  StartPastFailure,
  /// An Admin on the Agents hub's team MCP servers, which the stand-in team server holds with the profile Builder that
  /// names `pencil-design` and two runs that pin it (`Agents · MCP server · admin · archive` A23 `Pnpbe`, ORC-838): the
  /// panel's … menu has *Archive*, which opens the confirmation listing what names the server and the runs that stop.
  /// Opening it is a press, which an MCP client may make; confirming it is a person's.
  TeamArchive,
  /// An Admin on KON-12 (run 8), which the team's archive of `pencil-design` stopped at Implement: the Summary says
  /// which resource and who archived it, and Builder, which still names it, cannot run (A22 `pUu78`, ORC-838). What
  /// the press of *Archive and stop N runs* leaves behind, made without a press. The person is a member, as A22 and
  /// M7 `Ml7HN` draw it: the hub's *Archived* lists `pencil-design` read-only, with no Restore.
  TeamArchived,
  /// [`Self::TeamArchived`] for an Admin (A36 `qvMZD`, A37 `AFbIb`): the hub's *Archived* offers *Restore
  /// pencil-design*, and Builder's *Ready here?* opens it there.
  TeamArchivedAdmin,
}

/// How long after the window opens the server of [`State::StartRecovered`] answers a `work.start`; recovery then
/// carries the waiting start on within a minute.
pub const START_ANSWERS_AFTER: std::time::Duration = std::time::Duration::from_secs(90);

impl State {
  pub const ALL: [Self; 49] = [
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
    Self::ClosedAdmin,
    Self::ClosedMember,
    Self::VersionRefused,
    Self::TeamRunning,
    Self::EditConflict,
    Self::NotReadyHere,
    Self::NotReadyAfterRun,
    Self::StartFailed,
    Self::TeamEdit,
    Self::TeamEditDemoted,
    Self::TeamEditOffline,
    Self::TeamEditRejects,
    Self::TeamEditPublished,
    Self::TeamEditRefusedAdmin,
    Self::TeamEditVariables,
    Self::TeamHub,
    Self::TeamHubMember,
    Self::TeamHubDown,
    Self::StartRecovered,
    Self::StartWaiting,
    Self::StartRunnerRefused,
    Self::StartPastFailure,
    Self::TeamArchive,
    Self::TeamArchived,
    Self::TeamArchivedAdmin,
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
      Self::ClosedAdmin => "team-closed-admin",
      Self::ClosedMember => "team-closed-member",
      Self::VersionRefused => "team-version-refused",
      Self::TeamRunning => "team-running",
      Self::EditConflict => "team-edit-conflict",
      Self::NotReadyHere => "team-not-ready",
      Self::NotReadyAfterRun => "team-not-ready-ended",
      Self::StartFailed => "team-start-failed",
      Self::TeamEdit => "team-edit",
      Self::TeamEditDemoted => "team-edit-demoted",
      Self::TeamEditOffline => "team-edit-offline",
      Self::TeamEditRejects => "team-edit-rejects",
      Self::TeamEditPublished => "team-edit-published",
      Self::TeamEditRefusedAdmin => "team-edit-refused-admin",
      Self::TeamEditVariables => "team-edit-variables",
      Self::TeamHub => "team-hub",
      Self::TeamHubMember => "team-hub-member",
      Self::TeamHubDown => "team-hub-down",
      Self::StartRecovered => "team-start-recovered",
      Self::StartWaiting => "team-start-waiting",
      Self::StartRunnerRefused => "team-start-runner-refused",
      Self::StartPastFailure => "team-start-past-failure",
      Self::TeamArchive => "team-archive",
      Self::TeamArchived => "team-archived",
      Self::TeamArchivedAdmin => "team-archived-admin",
    }
  }

  /// Whether tasks of Kontur have runs on this computer, which the server names by their ids here.
  pub const fn runs_here(self) -> bool {
    matches!(
      self,
      Self::OutageRunning
        | Self::MemberRunning
        | Self::ModuleOutage
        | Self::Claims
        | Self::TeamRunning
        | Self::NotReadyAfterRun
        | Self::TeamArchived
        | Self::TeamArchivedAdmin
    )
  }

  /// Whether the state is of the archive of a team resource, whose server holds the resources that name it.
  pub const fn archives(self) -> bool {
    matches!(self, Self::TeamArchive | Self::TeamArchived | Self::TeamArchivedAdmin)
  }

  /// Whether the state is of the hub's edit of the team's resources, whose server holds them.
  pub const fn edits(self) -> bool {
    matches!(
      self,
      Self::TeamEdit
        | Self::TeamEditDemoted
        | Self::TeamEditOffline
        | Self::TeamEditRejects
        | Self::TeamEditPublished
        | Self::TeamEditRefusedAdmin
        | Self::TeamEditVariables
    )
  }

  /// Whether the stand-in team server holds every team resource the seed cached, for the hub to read.
  pub const fn serves_hub(self) -> bool {
    matches!(self, Self::TeamHub | Self::TeamHubMember)
  }

  /// Whether the person set the program path of `pencil-design` on this computer before the state.
  pub const fn program_set(self) -> bool {
    matches!(self, Self::TeamEditRefusedAdmin)
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
