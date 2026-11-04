//! navigate's Project settings: its tabs, and Connections and Environment, which are tabs of it with routes of their
//! own.

use serde_json::{Map, Value};

use super::{ToolError, string};
use crate::routes::{ConnectionsView, NewConnection, ProjectPage, ProjectSettingsTab, SecretsView, ServicesPage};

/// Project settings' tabs as navigate names them: its own, then the two with routes of their own.
pub(super) const SETTINGS_TABS: [&str; 8] = [
  "general",
  "runs",
  "services",
  "orchestrator",
  "context",
  "connections",
  "environment",
  "sharing",
];

/// Project settings › Connections, or the page of the connection `selected` names.
pub(super) fn connections_view(args: &Map<String, Value>) -> Result<ConnectionsView, ToolError> {
  if args.contains_key("selected") {
    return Ok(ConnectionsView::of(super::navigate::connection(args, "selected")?));
  }
  Ok(ConnectionsView::default())
}

/// Project settings on the tab `tab` names: one of its own, or Connections or Environment (`secrets` too, the tab it
/// was), whose routes are their own; with tab runs, `view` rules or storage opens that sub-page, and with tab services
/// a page under it ([`services_page`]).
pub(super) fn project_settings_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let view = string(args, "view")?;
  let page = match string(args, "tab")? {
    Some("connections") => ProjectPage::Connections(ConnectionsView::default()),
    Some(name) if ProjectSettingsTab::is_environment(name) => ProjectPage::Secrets(SecretsView::default()),
    _ => ProjectPage::Settings(super::navigate::tab(
      args,
      ProjectSettingsTab::from_name,
      &SETTINGS_TABS,
    )?),
  };
  match (page, view) {
    (page, None) => Ok(page),
    (ProjectPage::Settings(ProjectSettingsTab::Runs), Some("rules")) => Ok(ProjectPage::ProjectRules),
    (ProjectPage::Settings(ProjectSettingsTab::Runs), Some("storage")) => Ok(ProjectPage::Storage),
    (ProjectPage::Settings(ProjectSettingsTab::Services), Some(view)) => services_page(args, view),
    _ => Err(ToolError::InvalidParam {
      param: "view",
      reason: "rules and storage are pages of the runs tab, new-service, new-stack, new-suite, import, flaky, \
               service, stack and suite pages of the services tab"
        .to_owned(),
    }),
  }
}

/// The page under Services `view` names; a service's, a stack's or a suite's with its resource `id`.
fn services_page(args: &Map<String, Value>, view: &str) -> Result<ProjectPage, ToolError> {
  if let Some(page) = ServicesPage::named(view) {
    return Ok(ProjectPage::Services(page));
  }
  let item: fn(_) -> ServicesPage = match view {
    "service" => ServicesPage::Service,
    "stack" => ServicesPage::Stack,
    "suite" => ServicesPage::Suite,
    other => {
      return Err(ToolError::InvalidParam {
        param: "view",
        reason: format!(
          "`{other}` is not one of {}, service, stack or suite",
          ServicesPage::NAMED.map(ServicesPage::name).join(", ")
        ),
      });
    }
  };
  let id = string(args, "id")?.ok_or_else(|| ToolError::InvalidParam {
    param: "id",
    reason: format!("the {view} page needs the {view}'s id"),
  })?;
  let id = id.parse().map_err(|_| ToolError::InvalidParam {
    param: "id",
    reason: format!("`{id}` is not an id"),
  })?;
  Ok(ProjectPage::Services(item(id)))
}

/// What `kind` adds, an agent provider when left out.
pub(super) fn new_connection(args: &Map<String, Value>) -> Result<NewConnection, ToolError> {
  let Some(name) = string(args, "kind")? else {
    return Ok(NewConnection::default());
  };
  NewConnection::from_name(name).ok_or_else(|| ToolError::InvalidParam {
    param: "kind",
    reason: format!(
      "`{name}` is not one of {}",
      NewConnection::ALL.map(NewConnection::name).join(", ")
    ),
  })
}
