//! Project settings' tabs: its own, named in its query, and Connections and Secrets, which keep their paths.

use orchester_app::routes::{
  ConnectionsView, NewConnection, ProjectPage, ProjectSettingsTab, Route, RouteError, Screen,
};

use super::{CONNECTION, PROJECT, at, connection};

/// Connections, Secrets and their pages are tabs of Project settings: they keep their own paths, and the sidebar
/// shows Project settings as the screen they belong to.
#[test]
fn connections_and_secrets_routes_are_project_settings_tabs() {
  let root = format!("/project/{PROJECT}/connections");
  for (page, path) in [
    (ProjectPage::Connections(ConnectionsView::default()), root.clone()),
    (
      ProjectPage::Connections(ConnectionsView::of(connection())),
      format!("{root}?selected={CONNECTION}"),
    ),
    (
      ProjectPage::NewConnection(NewConnection::AgentProvider),
      format!("{root}/new"),
    ),
    (
      ProjectPage::NewConnection(NewConnection::McpServer),
      format!("{root}/new?kind=mcp_server"),
    ),
    (
      ProjectPage::EditConnection(connection()),
      format!("{root}/{CONNECTION}/edit"),
    ),
    (
      ProjectPage::Secrets(Default::default()),
      format!("/project/{PROJECT}/secrets"),
    ),
    (
      ProjectPage::ProjectRules,
      format!("/project/{PROJECT}/settings?tab=runs&view=rules"),
    ),
  ] {
    let route = at(page);
    assert_eq!(route.path(), path);
    assert_eq!(Route::parse(&path), Ok(route.clone()), "{path}");
    assert_eq!(route.nav_screen(), Screen::ProjectSettings, "{route:?}");
  }
  assert_eq!(
    Route::parse(&format!("{root}/not-an-id/edit")),
    Err(RouteError::NoScreen)
  );
  assert!(matches!(
    Route::parse(&format!("{root}?selected=nope")),
    Err(RouteError::InvalidParam { .. })
  ));
}

/// Project settings' own tabs are named in its query; the names of the tabs Runs and Context took in still open them.
#[test]
fn project_settings_tabs_have_their_documented_paths() {
  let root = format!("/project/{PROJECT}/settings");
  for (tab, path) in [
    (ProjectSettingsTab::General, root.clone()),
    (ProjectSettingsTab::Runs, format!("{root}?tab=runs")),
    (ProjectSettingsTab::Orchestrator, format!("{root}?tab=orchestrator")),
    (ProjectSettingsTab::Context, format!("{root}?tab=context")),
    (ProjectSettingsTab::Sharing, format!("{root}?tab=sharing")),
  ] {
    let route = at(ProjectPage::Settings(tab));
    assert_eq!(route.path(), path);
    assert_eq!(Route::parse(&path), Ok(route));
  }
  for (old, tab) in [
    ("limits", ProjectSettingsTab::Runs),
    ("sources", ProjectSettingsTab::Context),
    ("code_index", ProjectSettingsTab::Context),
  ] {
    assert_eq!(
      Route::parse(&format!("{root}?tab={old}")),
      Ok(at(ProjectPage::Settings(tab))),
      "{old}"
    );
  }
}

/// Environment (`S06`), the Secrets tab it was, keeps the secrets' path; the settings' query naming it, or Secrets, or
/// the environment Runs once held, opens it there.
#[test]
fn project_settings_environment_opens_the_secrets_route() {
  let root = format!("/project/{PROJECT}/settings");
  for name in ["environment", "secrets"] {
    assert_eq!(
      Route::parse(&format!("{root}?tab={name}")),
      Ok(at(ProjectPage::Secrets(Default::default()))),
      "{name}"
    );
  }
  assert!(matches!(
    Route::parse(&format!("{root}?tab=nowhere")),
    Err(RouteError::InvalidParam { .. })
  ));
}

/// The Memory page's Settings tab (`S15` `BJQn0`) is gone: its path opens Context, where the memory settings are now.
#[test]
fn memory_settings_tab_route_opens_project_settings_context() {
  let memory = format!("/project/{PROJECT}/memory");
  assert_eq!(
    Route::parse(&format!("{memory}?tab=settings")),
    Ok(at(ProjectPage::Settings(ProjectSettingsTab::Context)))
  );
  assert!(matches!(
    Route::parse(&format!("{memory}?tab=nowhere")),
    Err(RouteError::InvalidParam { .. })
  ));
}
