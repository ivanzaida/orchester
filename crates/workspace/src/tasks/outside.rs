//! Changes made outside the screens, by automation through Orchester's MCP server, counted per project
//! and per kind of data: only the changed project's feed reads its data again, and a screen reads again
//! only what the change touched, in place. Application settings, which no project owns, are counted apart
//! ([`AppOutside`]).

use std::{collections::HashMap, sync::Arc};

use orchester_core::id::ProjectId;
use tokio::sync::watch;

/// What a change made outside the screens touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outside {
  /// A task was saved, queued or returned to Draft, or a run's work changed what the task list shows.
  Tasks,
  /// A run's own settings changed, such as its auto mode.
  Runs,
  /// The project's auto mode settings changed.
  AutoMode,
  /// Whether the project uses the code index changed.
  CodeIndex,
  /// The project's limits changed.
  Limits,
  /// Project environment or repository agent files changed.
  ProjectSettings,
  /// A pipeline was added or published.
  Pipelines,
  /// A connection or an MCP server was added, changed, checked or removed.
  Connections,
  /// A command tool was added, saved or published.
  Tools,
  /// An agent profile draft or published version changed.
  Profiles,
  /// A project instruction draft or published version changed.
  Instructions,
  /// A skill draft or published version changed.
  Skills,
  /// A switch of what the project's Orchestrator may do itself changed.
  Orchestrator,
  /// Whether Orchester estimates the project's story points changed.
  Estimates,
}

/// How often each kind of a project's data changed outside the screens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutsideCounts {
  pub tasks: u64,
  pub runs: u64,
  pub auto_mode: u64,
  pub code_index: u64,
  pub limits: u64,
  pub project_settings: u64,
  pub pipelines: u64,
  pub connections: u64,
  pub tools: u64,
  pub profiles: u64,
  pub instructions: u64,
  pub skills: u64,
  pub orchestrator: u64,
  pub estimates: u64,
}

impl OutsideCounts {
  /// How often `change` was made.
  pub const fn of(self, change: Outside) -> u64 {
    match change {
      Outside::Tasks => self.tasks,
      Outside::Runs => self.runs,
      Outside::AutoMode => self.auto_mode,
      Outside::CodeIndex => self.code_index,
      Outside::Limits => self.limits,
      Outside::ProjectSettings => self.project_settings,
      Outside::Pipelines => self.pipelines,
      Outside::Connections => self.connections,
      Outside::Tools => self.tools,
      Outside::Profiles => self.profiles,
      Outside::Instructions => self.instructions,
      Outside::Skills => self.skills,
      Outside::Orchestrator => self.orchestrator,
      Outside::Estimates => self.estimates,
    }
  }

  fn count(&mut self, change: Outside) {
    let count = match change {
      Outside::Tasks => &mut self.tasks,
      Outside::Runs => &mut self.runs,
      Outside::AutoMode => &mut self.auto_mode,
      Outside::CodeIndex => &mut self.code_index,
      Outside::Limits => &mut self.limits,
      Outside::ProjectSettings => &mut self.project_settings,
      Outside::Pipelines => &mut self.pipelines,
      Outside::Connections => &mut self.connections,
      Outside::Tools => &mut self.tools,
      Outside::Profiles => &mut self.profiles,
      Outside::Instructions => &mut self.instructions,
      Outside::Skills => &mut self.skills,
      Outside::Orchestrator => &mut self.orchestrator,
      Outside::Estimates => &mut self.estimates,
    };
    *count += 1;
  }
}

/// An application setting that changed, which no project owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppOutside {
  /// The share of a plan window at which Usage warns.
  UsageThreshold,
  /// Whether error reports are sent.
  ErrorReports,
  /// When the Orchestrator's daily report comes, or that it is off.
  DailyReport,
  /// Agents' browser program or the Browser window's switch.
  Browser,
  /// What automation may do, or whether agents may control the window.
  Automation,
}

/// How often each application setting changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppOutsideCounts {
  pub usage_threshold: u64,
  pub error_reports: u64,
  pub daily_report: u64,
  pub browser: u64,
  pub automation: u64,
}

impl AppOutsideCounts {
  /// How often `change` was made.
  pub const fn of(self, change: AppOutside) -> u64 {
    match change {
      AppOutside::UsageThreshold => self.usage_threshold,
      AppOutside::ErrorReports => self.error_reports,
      AppOutside::DailyReport => self.daily_report,
      AppOutside::Browser => self.browser,
      AppOutside::Automation => self.automation,
    }
  }

  const fn count(&mut self, change: AppOutside) {
    let count = match change {
      AppOutside::UsageThreshold => &mut self.usage_threshold,
      AppOutside::ErrorReports => &mut self.error_reports,
      AppOutside::DailyReport => &mut self.daily_report,
      AppOutside::Browser => &mut self.browser,
      AppOutside::Automation => &mut self.automation,
    };
    *count += 1;
  }
}

/// Counts the changes made outside the screens, by project, and the application settings' changes.
#[derive(Clone)]
pub struct OutsideChanges {
  projects: Arc<watch::Sender<HashMap<ProjectId, OutsideCounts>>>,
  app: Arc<watch::Sender<AppOutsideCounts>>,
}

impl Default for OutsideChanges {
  fn default() -> Self {
    Self {
      projects: Arc::new(watch::Sender::new(HashMap::new())),
      app: Arc::new(watch::Sender::new(AppOutsideCounts::default())),
    }
  }
}

impl PartialEq for OutsideChanges {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.projects, &other.projects)
  }
}

impl OutsideChanges {
  /// `change` was made to `project` outside the screens. Returns how often each kind of `project`'s data changed
  /// with this change, before any later one: a screen that made the change takes it as seen.
  pub fn record(&self, project: ProjectId, change: Outside) -> OutsideCounts {
    let mut recorded = OutsideCounts::default();
    self.projects.send_modify(|counts| {
      let counts = counts.entry(project).or_default();
      counts.count(change);
      recorded = *counts;
    });
    recorded
  }

  /// How often each kind of `project`'s data changed so far.
  pub fn counts(&self, project: ProjectId) -> OutsideCounts {
    self.projects.borrow().get(&project).copied().unwrap_or_default()
  }

  pub fn subscribe(&self) -> watch::Receiver<HashMap<ProjectId, OutsideCounts>> {
    self.projects.subscribe()
  }

  /// The application setting `change` names changed. Returns how often each one changed with this change.
  pub fn record_app(&self, change: AppOutside) -> AppOutsideCounts {
    let mut recorded = AppOutsideCounts::default();
    self.app.send_modify(|counts| {
      counts.count(change);
      recorded = *counts;
    });
    recorded
  }

  pub fn subscribe_app(&self) -> watch::Receiver<AppOutsideCounts> {
    self.app.subscribe()
  }
}
