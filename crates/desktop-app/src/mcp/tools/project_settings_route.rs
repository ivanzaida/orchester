//! navigate's Project settings: its tabs, and Connections and Secrets, which are tabs of it with routes of their own.

use serde_json::{Map, Value};

use super::{ToolError, string};
use crate::routes::{ConnectionsView, NewConnection, ProjectPage, ProjectSettingsTab, SecretsView};

/// Project settings' tabs as navigate names them: its own, then the two with routes of their own.
pub(super) const SETTINGS_TABS: [&str; 6] = ["general", "runs", "context", "connections", "secrets", "sharing"];

/// Project settings › Connections, or the page of the connection `selected` names.
pub(super) fn connections_view(args: &Map<String, Value>) -> Result<ConnectionsView, ToolError> {
  if args.contains_key("selected") {
    return Ok(ConnectionsView::of(super::navigate::connection(args, "selected")?));
  }
  Ok(ConnectionsView::default())
}

/// Project settings on the tab `tab` names: one of its own, or Connections or Secrets, whose routes are their own;
/// with tab runs and `view` rules, *Approved for every run*.
pub(super) fn project_settings_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let rules = match string(args, "view")? {
    None => false,
    Some("rules") => true,
    Some(other) => {
      return Err(ToolError::InvalidParam {
        param: "view",
        reason: format!("`{other}` is not rules, the only page of Project settings' Runs"),
      });
    }
  };
  let page = match string(args, "tab")? {
    Some("connections") => ProjectPage::Connections(ConnectionsView::default()),
    Some("secrets") => ProjectPage::Secrets(SecretsView::default()),
    _ => ProjectPage::Settings(super::navigate::tab(
      args,
      ProjectSettingsTab::from_name,
      &SETTINGS_TABS,
    )?),
  };
  match page {
    ProjectPage::Settings(ProjectSettingsTab::Runs) if rules => Ok(ProjectPage::ProjectRules),
    page if !rules => Ok(page),
    _ => Err(ToolError::InvalidParam {
      param: "view",
      reason: "rules is a page of the runs tab".to_owned(),
    }),
  }
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
