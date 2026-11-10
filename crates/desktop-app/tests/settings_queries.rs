//! The settings queries' invalidation map (`orchester_app::queries::changes`): which queries each recorded change, the
//! accounts', the update settings' and the runs' touch, so a change from an MCP tool or another screen reads again
//! exactly what shows it. The window follows the map in `tests/project_settings/queries.rs` and
//! `tests/settings_general.rs`.

use orchester_app::queries::changes::{
  ACCOUNTS, APP_OUTSIDE, AppRead, OUTSIDE, ProjectRead, RUNS, TEAM, UPDATES, app_touched, touched,
};
use orchester_workspace::tasks::outside::{AppOutside, Outside, OutsideChanges};

/// The Agents and Pipelines hubs' reads, which a change no settings tab shows may touch.
const HUB: [ProjectRead; 6] = [
  ProjectRead::TeamHub,
  ProjectRead::LocalHub,
  ProjectRead::PipelineHub,
  ProjectRead::HubInstructions,
  ProjectRead::HubSkills,
  ProjectRead::ProfileForm,
];

#[test]
fn settings_queries_limits_auto_mode_and_autopilot_read_the_project_settings_again() {
  // `update_project_limits`, `set_allow_autopilot` and the approval settings' tools record these kinds; a profile's
  // approval mode is named in Runs › Approvals.
  for kind in [Outside::Limits, Outside::AutoMode, Outside::Runs] {
    assert_eq!(touched(kind), &[ProjectRead::Settings], "{kind:?}");
  }
  assert!(touched(Outside::Profiles).contains(&ProjectRead::Settings));
}

#[test]
fn settings_queries_project_settings_read_what_the_environment_and_context_show() {
  // `set_project_environment` and `set_repository_agent_files` record `ProjectSettings`.
  let reads = touched(Outside::ProjectSettings);
  for read in [
    ProjectRead::Settings,
    ProjectRead::Variables,
    ProjectRead::Memory,
    ProjectRead::Sources,
  ] {
    assert!(reads.contains(&read), "{read:?} in {reads:?}");
  }
}

#[test]
fn settings_queries_code_index_reads_the_setting() {
  // The builds are live state Context › Code index reads again itself; `tests/reads/screens.rs` counts those reads.
  assert_eq!(touched(Outside::CodeIndex), &[ProjectRead::Settings]);
}

#[test]
fn settings_queries_connections_read_the_list_the_github_link_the_reviewer_and_the_secrets() {
  let reads = touched(Outside::Connections);
  for read in [
    ProjectRead::Settings,
    ProjectRead::Repository,
    ProjectRead::Connections,
    ProjectRead::Secrets,
  ] {
    assert!(reads.contains(&read), "{read:?} in {reads:?}");
  }
}

#[test]
fn settings_queries_orchestrator_switches_and_estimates_read_only_their_own() {
  assert_eq!(touched(Outside::Orchestrator), &[ProjectRead::Orchestrator]);
  assert_eq!(touched(Outside::Estimates), &[ProjectRead::Estimates]);
}

#[test]
fn settings_queries_changes_no_settings_tab_shows_read_no_setting() {
  for kind in [
    Outside::Tasks,
    Outside::Pipelines,
    Outside::Tools,
    Outside::Instructions,
    Outside::Skills,
  ] {
    let reads = touched(kind);
    assert!(reads.iter().all(|read| HUB.contains(read)), "{kind:?}: {reads:?}");
  }
  assert!(touched(Outside::Tasks).is_empty());
}

#[test]
fn settings_queries_the_hub_reads_again_what_it_followed() {
  // The kinds each of the Agents hub's reads followed as a future's dependencies.
  let expected: [(ProjectRead, &[Outside]); 6] = [
    (
      ProjectRead::TeamHub,
      &[
        Outside::Connections,
        Outside::Tools,
        Outside::Profiles,
        Outside::Instructions,
        Outside::Skills,
      ],
    ),
    (
      ProjectRead::LocalHub,
      &[
        Outside::Connections,
        Outside::Tools,
        Outside::Profiles,
        Outside::Instructions,
        Outside::Skills,
        Outside::Pipelines,
      ],
    ),
    (
      ProjectRead::PipelineHub,
      &[
        Outside::Connections,
        Outside::Tools,
        Outside::Profiles,
        Outside::Pipelines,
      ],
    ),
    (
      ProjectRead::HubInstructions,
      &[Outside::Instructions, Outside::Profiles, Outside::Pipelines],
    ),
    (
      ProjectRead::HubSkills,
      &[Outside::Skills, Outside::Profiles, Outside::Pipelines],
    ),
    (
      ProjectRead::ProfileForm,
      &[
        Outside::Connections,
        Outside::Profiles,
        Outside::Pipelines,
        Outside::ProjectSettings,
      ],
    ),
  ];
  for (read, kinds) in expected {
    for kind in OUTSIDE {
      assert_eq!(
        touched(kind).contains(&read),
        kinds.contains(&kind),
        "{read:?} on {kind:?}"
      );
    }
  }
  // A local hub shows what the runs decide; a team's hub follows its configuration, which the server's events change.
  assert!(RUNS.contains(&ProjectRead::LocalHub) && !RUNS.contains(&ProjectRead::TeamHub));
  assert!(
    RUNS.contains(&ProjectRead::PipelineHub),
    "a pipeline's readiness follows the runs too"
  );
  assert_eq!(TEAM, &[ProjectRead::TeamHub]);
}

#[test]
fn settings_queries_every_kind_is_followed_once() {
  for (index, kind) in OUTSIDE.iter().enumerate() {
    assert!(!OUTSIDE[index + 1..].contains(kind), "{kind:?} twice");
  }
  for (index, kind) in APP_OUTSIDE.iter().enumerate() {
    assert!(!APP_OUTSIDE[index + 1..].contains(kind), "{kind:?} twice");
  }
  // Every kind that reads a query is followed: a kind left out of the list would never invalidate.
  for kind in [
    Outside::Limits,
    Outside::AutoMode,
    Outside::Runs,
    Outside::CodeIndex,
    Outside::ProjectSettings,
    Outside::Connections,
    Outside::Orchestrator,
    Outside::Estimates,
  ] {
    assert!(OUTSIDE.contains(&kind), "{kind:?} followed");
  }
}

#[test]
fn settings_queries_each_application_setting_reads_its_own_query() {
  let expected = [
    (AppOutside::UsageThreshold, AppRead::UsageThreshold),
    (AppOutside::ErrorReports, AppRead::ErrorReports),
    (AppOutside::DailyReport, AppRead::DailyReport),
    (AppOutside::Browser, AppRead::Browser),
    (AppOutside::Automation, AppRead::Automation),
  ];
  for (kind, read) in expected {
    assert_eq!(app_touched(kind), &[read], "{kind:?}");
  }
}

#[test]
fn settings_queries_accounts_updates_and_runs_read_what_they_change() {
  assert_eq!(ACCOUNTS, &[AppRead::Accounts]);
  assert_eq!(UPDATES, &[AppRead::Updates]);
  // A run's request answered *for every run* adds a rule to the project's settings; a run's start builds its index.
  assert_eq!(
    RUNS,
    &[
      ProjectRead::Settings,
      ProjectRead::IndexBuilds,
      ProjectRead::LocalHub,
      ProjectRead::PipelineHub
    ]
  );
}

#[test]
fn settings_queries_application_changes_are_counted_apart_from_projects() {
  let changes = OutsideChanges::default();
  let first = changes.record_app(AppOutside::UsageThreshold);
  assert_eq!(first.of(AppOutside::UsageThreshold), 1);
  assert_eq!(first.of(AppOutside::Browser), 0);
  let second = changes.record_app(AppOutside::UsageThreshold);
  assert_eq!(second.of(AppOutside::UsageThreshold), 2);
  assert_eq!(*changes.subscribe_app().borrow(), second);
  assert!(changes.subscribe().borrow().is_empty(), "no project changed");
}

#[test]
fn settings_queries_orchestrator_and_estimates_are_counted_per_project() {
  let changes = OutsideChanges::default();
  let project = orchester_core::id::ProjectId::generate();
  let counts = changes.record(project, Outside::Orchestrator);
  assert_eq!(counts.of(Outside::Orchestrator), 1);
  let counts = changes.record(project, Outside::Estimates);
  assert_eq!(
    (counts.of(Outside::Orchestrator), counts.of(Outside::Estimates)),
    (1, 1)
  );
  assert_eq!(changes.counts(project), counts);
}
