//! Which queries a change touches: each kind of a project's change that `OutsideChanges` counts, each application
//! setting's, and the other signals the window follows (the accounts', the update settings', a project's runs', a team
//! project's configuration). The map is plain data ([`touched`], [`app_touched`]), so it is tested apart from the
//! window; [`invalidate`] and [`invalidate_app`] turn it into lurq's invalidations.

use std::path::Path;

use lurq::query::QueryClient;
use orchester_workspace::tasks::outside::{AppOutside, Outside};

use super::{Session, app, hub, project};
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
  /// A team project's Agents hub: the team's resources and what this computer filled in.
  TeamHub,
  /// A local project's Agents hub: its resources, what uses them and how they stand.
  LocalHub,
  /// What a local instruction's panel edits over: the instructions, the profiles and pipelines they name.
  HubInstructions,
  /// What a local skill's panel edits over: the skills, the profiles and pipelines their scopes name.
  HubSkills,
  /// What the form that makes a profile chooses from: the agent connections, the profiles, the pipelines' steps and the
  /// repository's agent files.
  ProfileForm,
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
}

/// What a project's runs starting, ending or changing state touches: the project's settings, whose rules *Approve for
/// every run* adds as a run's request is answered, and the code index's builds, which the runner makes as a run starts.
/// The project's loader follows the runs' state as it is published, so a move no tab rendered still reads them again.
/// A local project's hub shows what the runs decide too: which profiles and servers they use, and so readiness.
pub const RUNS: &[ProjectRead] = &[ProjectRead::Settings, ProjectRead::IndexBuilds, ProjectRead::LocalHub];
/// What a change of a team project's configuration touches: its hub. The configuration is read again whenever the
/// project's cache took one of the team server's events, so this is how a resource published, archived or restored on
/// the server, a member's role and the standings of what was sent from here reach the hub.
pub const TEAM: &[ProjectRead] = &[ProjectRead::TeamHub];
/// What a change of the person's accounts touches.
pub const ACCOUNTS: &[AppRead] = &[AppRead::Accounts];
/// What an update setting saved, or the update's state turning to another kind (a check's end), touches.
pub const UPDATES: &[AppRead] = &[AppRead::Updates];

/// The queries a project's change of `kind` touches.
pub const fn touched(kind: Outside) -> &'static [ProjectRead] {
  match kind {
    // A profile's approval mode is what Runs › Approvals names as set to Autopilot; the code index's setting is one of
    // the project's settings (its builds are its tab's live state).
    Outside::AutoMode | Outside::Limits | Outside::Runs | Outside::CodeIndex => &[ProjectRead::Settings],
    // The hubs list the profiles; an instruction's and a skill's scope name them.
    Outside::Profiles => &[
      ProjectRead::Settings,
      ProjectRead::TeamHub,
      ProjectRead::LocalHub,
      ProjectRead::HubInstructions,
      ProjectRead::HubSkills,
      ProjectRead::ProfileForm,
    ],
    // The repository's agent files are what a new profile's permissions are shown beside.
    Outside::ProjectSettings => &[
      ProjectRead::Settings,
      ProjectRead::Variables,
      ProjectRead::Memory,
      ProjectRead::Sources,
      ProjectRead::ProfileForm,
    ],
    // A connection's change may change the reviewer's account, the GitHub link and what the secrets are used by. A
    // program path, a secret value and *On here* filled in for the team's resources count as one too.
    Outside::Connections => &[
      ProjectRead::Settings,
      ProjectRead::Repository,
      ProjectRead::Connections,
      ProjectRead::Secrets,
      ProjectRead::TeamHub,
      ProjectRead::LocalHub,
      ProjectRead::ProfileForm,
    ],
    Outside::Orchestrator => &[ProjectRead::Orchestrator],
    Outside::Estimates => &[ProjectRead::Estimates],
    // A pipeline's steps use profiles and name an instruction's and a skill's scope.
    Outside::Pipelines => &[
      ProjectRead::LocalHub,
      ProjectRead::HubInstructions,
      ProjectRead::HubSkills,
      ProjectRead::ProfileForm,
    ],
    Outside::Tools => &[ProjectRead::TeamHub, ProjectRead::LocalHub],
    Outside::Instructions => &[
      ProjectRead::TeamHub,
      ProjectRead::LocalHub,
      ProjectRead::HubInstructions,
    ],
    Outside::Skills => &[ProjectRead::TeamHub, ProjectRead::LocalHub, ProjectRead::HubSkills],
    Outside::Tasks => &[],
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
pub const APP_OUTSIDE: [AppOutside; 5] = [
  AppOutside::UsageThreshold,
  AppOutside::ErrorReports,
  AppOutside::DailyReport,
  AppOutside::Browser,
  AppOutside::Automation,
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
    ProjectRead::TeamHub => client.invalidate(hub::team_hub(session, root)),
    ProjectRead::LocalHub => client.invalidate(hub::local_hub(session, root)),
    ProjectRead::HubInstructions => client.invalidate(hub::instructions(session, root)),
    ProjectRead::HubSkills => client.invalidate(hub::skills(session, root)),
    ProjectRead::ProfileForm => client.invalidate(hub::profile_form(session, root)),
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
  }
}
