//! All projects (`docs/design/orchester-workspace.md`, "All projects"): the projects the application
//! database knows, each with where it stands — its folder, its data and its runner in this session — and
//! its runs, and removing one from the list. A session that starts without a project welcomes the person
//! instead of showing an empty list ([`Listing::Welcome`]); once it has listed a project, an empty list
//! stays a list.

use std::sync::atomic::Ordering;

use orchester_core::run::TaskPhase;
use orchester_store::{RegisteredProject, StoreError};

use crate::{
  background,
  messages::{self, Message},
  runner::{ForgetRefusal, Runners, state::RunnerState},
  tasks::data::ProjectTasks,
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
  /// The projects All projects lists, or that it welcomes the person: not while an account's projects wait for its
  /// sign-in, which the list says. Blocks on SQLite and the file system.
  pub fn listing(&self) -> Result<Listing, StoreError> {
    let entries = self.entries()?;
    if !entries.is_empty() {
      self.listed.store(true, Ordering::Relaxed);
    }
    Ok(
      if entries.is_empty() && !self.listed.load(Ordering::Relaxed) && self.waiting_projects()?.is_empty() {
        Listing::Welcome
      } else {
        Listing::Projects(entries)
      },
    )
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

/// What All projects says about a project's runs without opening it: how many have an agent at work, and how
/// many things wait for the person, as every *Needs you* counts them ([`ProjectTasks::needs_you`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunCounts {
  /// Runs with an agent at work.
  pub active: usize,
  pub needs_you: usize,
}

impl RunCounts {
  /// Counts `tasks` as they stand with the runner online or not: without it no agent works, so a run recorded
  /// as running is interrupted rather than active ([`ProjectTasks::without_runner`]).
  pub fn of(tasks: &ProjectTasks, runner_online: bool) -> Self {
    let offline;
    let tasks = if runner_online {
      tasks
    } else {
      offline = tasks.without_runner();
      &offline
    };
    Self {
      active: tasks
        .phases
        .values()
        .filter(|latest| latest.phase() == Some(TaskPhase::Running))
        .count(),
      needs_you: tasks.needs_you(),
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
  /// Agents at work in it, as its runner last reported; `None` while its runner is starting, reconnecting
  /// or stopping, or has not reported yet, so whether agents work is not known.
  pub agents_working: Option<usize>,
}

impl ListedProject {
  /// `entry` with what its data said (`None` while it is read) and its runner's state and agents at work
  /// (`None` when no runner was started for it).
  pub fn of(entry: &ProjectEntry, data: Option<&ProjectData>, runner: Option<(&RunnerState, Option<usize>)>) -> Self {
    let shown = RunnerShown::of(runner.map(|(state, _)| state));
    let (standing, counts) = match data {
      _ if entry.folder_missing => (Standing::FolderMissing, None),
      Some(ProjectData::Unreadable(reason)) => (Standing::Unreadable(reason.clone()), None),
      // Active runs count only while the runner is online: without it no agent works.
      Some(ProjectData::Readable(Some(tasks))) => (
        Standing::Opens(shown),
        Some(RunCounts::of(tasks, shown == RunnerShown::Online)),
      ),
      Some(ProjectData::Readable(None)) | None => (Standing::Opens(shown), None),
    };
    Self {
      project: entry.project.clone(),
      standing,
      counts,
      agents_working: runner.map_or(Some(0), |(_, working)| working),
    }
  }

  /// Whether selecting the row opens the project.
  pub const fn opens(&self) -> bool {
    matches!(self.standing, Standing::Opens(_))
  }

  /// Why the project cannot be removed from the list now: agents are at work in it, or whether they are is
  /// not known yet.
  pub fn removal_refused(&self) -> Option<Message> {
    let key = match self.agents_working {
      Some(0) => return None,
      Some(_) => "projects.removeRefused",
      None => "projects.removeUnknown",
    };
    Some(Message::new(key).with("name", self.project.name.to_string()))
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
        let state = status.state_for(root);
        let working = match state {
          RunnerState::Stopped | RunnerState::Failed { .. } => Some(0),
          RunnerState::Online => status.work().map(|work| work.agents_working()),
          RunnerState::Starting | RunnerState::Reconnecting { .. } | RunnerState::Stopping => None,
        };
        (state, working)
      });
      ListedProject::of(
        entry,
        data.of(entry.project.id),
        runner.as_ref().map(|(state, working)| (state, *working)),
      )
    })
    .collect()
}

/// Forgets `project`: its runner, if it has one, stops once it answers that it has no work, then the
/// application database no longer lists it. Its folder and its `.orchester` data stay. Refused while agents
/// are at work in it, or while its runner cannot tell yet.
pub async fn remove(workspace: &Workspace, runners: &Runners, project: &RegisteredProject) -> Result<(), Message> {
  let name = project.name.to_string();
  runners
    .forget_if_idle(&project.repo_root)
    .await
    .map_err(|refusal| match refusal {
      ForgetRefusal::Busy => Message::new("projects.removeRefused").with("name", name),
      ForgetRefusal::Unknown => Message::new("projects.removeUnknown").with("name", name),
      ForgetRefusal::NoAnswer => Message::new("projects.removeNoAnswer").with("name", name),
    })?;
  let (workspace, id) = (workspace.clone(), project.id);
  background::run(move || workspace.forget(id)).await?.map_err(|error| {
    tracing::error!(%error, "cannot remove a project from the list");
    messages::store_error(&error)
  })
}
