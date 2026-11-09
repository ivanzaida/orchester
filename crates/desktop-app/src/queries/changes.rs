//! Which queries a change touches: each kind of a project's change that `OutsideChanges` counts, each application
//! setting's, and the other signals the window follows (the accounts', the update settings', a project's runs'). The
//! map is plain data ([`touched`], [`app_touched`]), so it is tested apart from the window; [`invalidate`] and
//! [`invalidate_app`] turn it into lurq's invalidations.

use std::path::Path;

use lurq::query::QueryClient;
use orchester_workspace::tasks::outside::{AppOutside, Outside};

use super::{Session, app, project};
use crate::screens::settings::Consent;

/// A project's query, by what it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectRead {
  /// `project_settings::load_in`: limits, agent files, the code index setting, auto mode, Autopilot, the reviewer, the
  /// approval rules and base notices.
  Settings,
  /// The stored GitHub link and the account it expects.
  Repository,
  Estimates,
  /// The Orchestrator's switches, every one, and how it watches the project's runs.
  Orchestrator,
  Memory,
  Variables,
  Connections,
  /// The secrets, whichever one their page shows.
  Secrets,
  Sources,
  /// The code index's builds, which a run's start builds and its end may remove.
  IndexBuilds,
}

/// An application query, by what it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppRead {
  UsageThreshold,
  ErrorReports,
  Updates,
  DailyReport,
  Browser,
  /// What automation may do and whether agents control the window, every consent.
  Automation,
  Accounts,
  /// The computer's *Services at once*, which Settings › General and every project's Runs › Limits show.
  ServiceSlots,
}

/// What a project's runs starting, ending or changing state touches: the project's settings, whose rules *Approve for
/// every run* adds as a run's request is answered, and the code index's builds, which the runner makes as a run starts.
/// The project's loader follows the runs' state as it is published, so a move no tab rendered still reads them again.
pub const RUNS: &[ProjectRead] = &[ProjectRead::Settings, ProjectRead::IndexBuilds];
/// What a change of the person's accounts touches.
pub const ACCOUNTS: &[AppRead] = &[AppRead::Accounts];
/// What an update setting saved, or the update's state turning to another kind (a check's end), touches.
pub const UPDATES: &[AppRead] = &[AppRead::Updates];

/// The queries a project's change of `kind` touches.
pub const fn touched(kind: Outside) -> &'static [ProjectRead] {
  match kind {
    // A profile's approval mode is what Runs › Approvals names as set to Autopilot; the code index's setting is one of
    // the project's settings (its builds are its tab's live state).
    Outside::AutoMode | Outside::Limits | Outside::Runs | Outside::Profiles | Outside::CodeIndex => {
      &[ProjectRead::Settings]
    }
    Outside::ProjectSettings => &[
      ProjectRead::Settings,
      ProjectRead::Variables,
      ProjectRead::Memory,
      ProjectRead::Sources,
    ],
    // A connection's change may change the reviewer's account, the GitHub link and what the secrets are used by.
    Outside::Connections => &[
      ProjectRead::Settings,
      ProjectRead::Repository,
      ProjectRead::Connections,
      ProjectRead::Secrets,
    ],
    Outside::Orchestrator => &[ProjectRead::Orchestrator],
    Outside::Estimates => &[ProjectRead::Estimates],
    Outside::Tasks | Outside::Pipelines | Outside::Tools | Outside::Instructions | Outside::Skills => &[],
  }
}

/// The queries a change of the application setting `kind` touches.
pub const fn app_touched(kind: AppOutside) -> &'static [AppRead] {
  match kind {
    AppOutside::UsageThreshold => &[AppRead::UsageThreshold],
    AppOutside::ErrorReports => &[AppRead::ErrorReports],
    AppOutside::DailyReport => &[AppRead::DailyReport],
    AppOutside::Browser => &[AppRead::Browser],
    AppOutside::Automation => &[AppRead::Automation],
    AppOutside::ServiceSlots => &[AppRead::ServiceSlots],
  }
}

/// Every kind of a project's change, for following them all.
pub const OUTSIDE: [Outside; 14] = [
  Outside::Tasks,
  Outside::Runs,
  Outside::AutoMode,
  Outside::CodeIndex,
  Outside::Limits,
  Outside::ProjectSettings,
  Outside::Pipelines,
  Outside::Connections,
  Outside::Tools,
  Outside::Profiles,
  Outside::Instructions,
  Outside::Skills,
  Outside::Orchestrator,
  Outside::Estimates,
];

/// Every application setting's change, for following them all.
pub const APP_OUTSIDE: [AppOutside; 6] = [
  AppOutside::UsageThreshold,
  AppOutside::ErrorReports,
  AppOutside::DailyReport,
  AppOutside::Browser,
  AppOutside::Automation,
  AppOutside::ServiceSlots,
];

/// Invalidates the project at `root`'s query `read`: an entry a tab or the project's loader shows reads again behind
/// its last value.
pub fn invalidate(client: &QueryClient, session: &Session, root: &Path, read: ProjectRead) {
  let (session, root) = (session.clone(), root.to_path_buf());
  match read {
    ProjectRead::Settings => client.invalidate(project::project_settings(session, root)),
    ProjectRead::Repository => client.invalidate(project::repository(session, root)),
    ProjectRead::Estimates => client.invalidate(project::estimates(session, root)),
    ProjectRead::Orchestrator => {
      client.invalidate(project::orchestrator_drives(session.clone(), root.clone()));
      client.invalidate(project::orchestrator_watch(session, root));
    }
    ProjectRead::Memory => client.invalidate(project::memory_settings(session, root)),
    ProjectRead::Variables => client.invalidate(project::variables(session, root)),
    ProjectRead::Connections => client.invalidate(project::connections(session, root)),
    // Keyed by the secret a page shows too, so every project's are read again: only shown ones read.
    ProjectRead::Secrets => client.invalidate(project::secrets::all()),
    ProjectRead::Sources => client.invalidate(project::project_sources(session, root)),
    ProjectRead::IndexBuilds => client.invalidate(project::index_builds(session, root)),
  }
}

/// Invalidates the application query `read`.
pub fn invalidate_app(client: &QueryClient, session: &Session, read: AppRead) {
  let session = session.clone();
  match read {
    AppRead::UsageThreshold => client.invalidate(app::usage_threshold(session)),
    AppRead::ErrorReports => client.invalidate(app::error_reports(session)),
    AppRead::Updates => client.invalidate(app::update_settings(session)),
    AppRead::DailyReport => client.invalidate(app::daily_report(session)),
    AppRead::Browser => client.invalidate(app::browser_settings(session)),
    AppRead::Automation => {
      for consent in Consent::ALL {
        client.invalidate(app::automation_consent(session.clone(), consent));
      }
    }
    AppRead::Accounts => client.invalidate(app::agent_accounts(session)),
    AppRead::ServiceSlots => client.invalidate(app::service_slots(session)),
  }
}
