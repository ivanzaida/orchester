//! All projects (`docs/design/orchester-workspace.md`, "All projects"): the projects the application
//! database knows, each with where it stands — its folder, its data and its runner in this session — and
//! its runs, and removing one from the list. A session that starts without a project welcomes the person
//! instead of showing an empty list ([`Listing::Welcome`]); once it has listed a project, an empty list
//! stays a list.

use std::sync::atomic::Ordering;

use orchester_core::run::RunCounts;
use orchester_store::{RegisteredProject, StoreError};

use crate::{
  messages::{self, Message},
  runner::{Runners, state::RunnerState},
  workspace::{ListedData, ProjectData, ProjectEntry, Workspace},
};

/// What All projects shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
  /// No project is known, and none was listed in this session: a first launch.
  Welcome,
  /// The known projects, most recently opened first; empty once every project was removed.
  Projects(Vec<ProjectEntry>),
}

impl Workspace {
  /// The projects All projects lists, or that it welcomes the person. Blocks on SQLite and the file
  /// system.
  pub fn listing(&self) -> Result<Listing, StoreError> {
    let entries = self.entries()?;
    if !entries.is_empty() {
      self.listed.store(true, Ordering::Relaxed);
    }
    Ok(if entries.is_empty() && !self.listed.load(Ordering::Relaxed) {
      Listing::Welcome
    } else {
      Listing::Projects(entries)
    })
  }
}

/// Where a listed project stands: its folder and its data first, then its runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
  /// The folder is not where it was; the project cannot be opened.
  FolderMissing,
  /// The project's data cannot be read, and why; it cannot be opened.
  Unreadable(Message),
  /// It opens; its runner in this session.
  Opens(RunnerShown),
}

/// A project's runner as All projects names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerShown {
  /// No runner was started for it in this session.
  NotStarted,
  Starting,
  Online,
  Reconnecting,
  Stopping,
  Stopped,
  /// It failed and does not retry on its own.
  Offline,
}

impl RunnerShown {
  fn of(state: Option<&RunnerState>) -> Self {
    match state {
      None => Self::NotStarted,
      Some(RunnerState::Starting) => Self::Starting,
      Some(RunnerState::Online) => Self::Online,
      Some(RunnerState::Reconnecting { .. }) => Self::Reconnecting,
      Some(RunnerState::Stopping) => Self::Stopping,
      Some(RunnerState::Stopped) => Self::Stopped,
      Some(RunnerState::Failed { .. }) => Self::Offline,
    }
  }
}

/// A row of All projects: the project, where it stands, its runs and the agents at work in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedProject {
  pub project: RegisteredProject,
  pub standing: Standing,
  /// Its active runs and the runs that need the person; `None` until its data is read, and for a project
  /// that cannot be opened.
  pub counts: Option<RunCounts>,
  /// Agents at work in it, as its runner last reported.
  pub agents_working: usize,
}

impl ListedProject {
  /// `entry` with what its data said (`None` while it is read) and its runner's state and agents at work
  /// (`None` when no runner was started for it).
  pub fn of(entry: &ProjectEntry, data: Option<&ProjectData>, runner: Option<(&RunnerState, usize)>) -> Self {
    let shown = RunnerShown::of(runner.map(|(state, _)| state));
    let (standing, counts) = match data {
      _ if entry.folder_missing => (Standing::FolderMissing, None),
      Some(ProjectData::Unreadable(reason)) => (Standing::Unreadable(reason.clone()), None),
      // Active runs count only while the runner is online: without it no agent works.
      Some(ProjectData::Readable(Some(phases))) => (
        Standing::Opens(shown),
        Some(RunCounts::of(phases.iter().copied(), shown == RunnerShown::Online)),
      ),
      Some(ProjectData::Readable(None)) | None => (Standing::Opens(shown), None),
    };
    Self {
      project: entry.project.clone(),
      standing,
      counts,
      agents_working: runner.map_or(0, |(_, working)| working),
    }
  }

  /// Whether selecting the row opens the project.
  pub const fn opens(&self) -> bool {
    matches!(self.standing, Standing::Opens(_))
  }

  /// Why the project cannot be removed from the list now: agents are at work in it.
  pub fn removal_refused(&self) -> Option<Message> {
    (self.agents_working > 0)
      .then(|| Message::new("projects.removeRefused").with("name", self.project.name.to_string()))
  }
}

/// The rows of All projects for `entries`, with what `data` read of them and their runners in `runners`.
pub fn listed(entries: &[ProjectEntry], data: &ListedData, runners: &Runners) -> Vec<ListedProject> {
  entries
    .iter()
    .map(|entry| {
      let root = &entry.project.repo_root;
      let status = runners.status_of(root);
      let runner = status.as_ref().map(|status| {
        let working = status.work().map_or(0, |work| work.agents_working());
        (status.state_for(root), working)
      });
      ListedProject::of(
        entry,
        data.of(entry.project.id),
        runner.as_ref().map(|(state, working)| (state, *working)),
      )
    })
    .collect()
}

/// Forgets `project`: the application database no longer lists it, and its runner, if it has one, stops at
/// checkpoints. Its folder and its `.orchester` data stay. Refused while agents are at work in it, as its
/// runner reports now. Blocks on SQLite.
pub fn remove(workspace: &Workspace, runners: &Runners, project: &RegisteredProject) -> Result<(), Message> {
  let entry = ProjectEntry {
    project: project.clone(),
    folder_missing: false,
  };
  let now = listed(std::slice::from_ref(&entry), &ListedData::default(), runners);
  if let Some(refusal) = now.first().and_then(ListedProject::removal_refused) {
    return Err(refusal);
  }
  workspace.forget(project.id).map_err(|error| {
    tracing::error!(%error, "cannot remove a project from the list");
    messages::store_error(&error)
  })?;
  runners.forget(&project.repo_root);
  Ok(())
}
