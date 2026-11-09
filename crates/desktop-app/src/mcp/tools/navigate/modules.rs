//! What navigate makes of `route` modules: the list on a tab, New module, a module's page on a tab, or Edit module;
//! of `route` new_task with `module`: New task in that module; and of `route` agents with `agent`: a profile's page or
//! a new profile, or with `filter`, `item` and `server`: the Agents hub, or with `view` new the form that makes a new one
//! of the kind `filter` holds. A `tab` or `view` where it names
//! nothing is refused, not left out.

use orchester_core::id::{ConnectionId, ModuleId, ResourceId};
use serde_json::{Map, Value};

use super::{ToolError, string, tab};
use crate::routes::{AgentTab, HubFilter, HubView, ModuleListTab, ModuleTab, ProjectPage};

/// The profiles' list; with `agent` new, a new profile; with a profile's id, its page. The Agents page is the hub:
/// `filter` (all, profiles, mcp_servers, instructions, skills or tools, and archived in a local project) and `item`, a
/// resource's id its panel shows, or `server`, a local MCP server's connection id.
pub(super) fn agent_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let (filter, item, server) = (string(args, "filter")?, string(args, "item")?, string(args, "server")?);
  match string(args, "agent")? {
    None => match string(args, "view")? {
      None => hub_page(filter, (item, server)),
      Some("new") => hub_form(filter, (item, server)),
      Some(other) => Err(ToolError::UnknownTab {
        name: other.to_owned(),
        known: vec!["new"],
      }),
    },
    Some(_) if filter.is_some() || item.is_some() || server.is_some() => Err(ToolError::InvalidParam {
      param: "agent",
      reason: "a profile's page has no filter, item or server".to_owned(),
    }),
    Some("new") => Ok(ProjectPage::NewAgent(AgentTab::default())),
    Some(agent) => agent
      .parse::<ResourceId>()
      .map(|id| ProjectPage::Agent(id, AgentTab::default()))
      .map_err(|error| ToolError::InvalidParam {
        param: "agent",
        reason: error.to_string(),
      }),
  }
}

/// The Agents hub on the form that makes a new one of the kind `filter` holds; an item or a server has no form.
fn hub_form(filter: Option<&str>, (item, server): (Option<&str>, Option<&str>)) -> Result<ProjectPage, ToolError> {
  if item.is_some() || server.is_some() {
    return Err(ToolError::InvalidParam {
      param: "view",
      reason: "a form has no item or server".to_owned(),
    });
  }
  let kinds = [
    HubFilter::Profiles,
    HubFilter::McpServers,
    HubFilter::Instructions,
    HubFilter::Skills,
    HubFilter::Tools,
  ];
  let named = filter.and_then(HubFilter::from_name).filter(|found| kinds.contains(found));
  named
    .map(|filter| ProjectPage::hub(HubView::adding(filter)))
    .ok_or_else(|| ToolError::InvalidParam {
      param: "filter",
      reason: format!(
        "a form makes one kind: {}",
        kinds.map(HubFilter::name).join(", ")
      ),
    })
}

/// The Agents hub on `filter` (All when left out) showing the resource `item` or the local MCP server `server`.
fn hub_page(filter: Option<&str>, (item, server): (Option<&str>, Option<&str>)) -> Result<ProjectPage, ToolError> {
  let filter = match filter {
    None => HubFilter::default(),
    Some(name) => HubFilter::from_name(name).ok_or_else(|| ToolError::UnknownTab {
      name: name.to_owned(),
      known: HubFilter::LOCAL.map(HubFilter::name).to_vec(),
    })?,
  };
  let item = item
    .map(|item| {
      item.parse::<ResourceId>().map_err(|error| ToolError::InvalidParam {
        param: "item",
        reason: error.to_string(),
      })
    })
    .transpose()?;
  let server = server
    .map(|server| {
      server.parse::<ConnectionId>().map_err(|error| ToolError::InvalidParam {
        param: "server",
        reason: error.to_string(),
      })
    })
    .transpose()?;
  match (item, server) {
    (Some(_), Some(_)) => Err(ToolError::InvalidParam {
      param: "server",
      reason: "the panel shows an item or a server, not both".to_owned(),
    }),
    (None, Some(server)) => Ok(ProjectPage::hub(HubView::on_server(filter, server))),
    (item, None) => Ok(ProjectPage::hub(HubView::new(filter, item))),
  }
}

/// The Modules list on `tab`; with `module` new or `view` new, New module; with a module's id, its page on `tab`, or
/// with `view` edit, Edit module.
pub(super) fn module_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let view = string(args, "view")?;
  let Some(module) = string(args, "module")? else {
    return match view {
      None => {
        let known = ModuleListTab::ALL.map(ModuleListTab::name);
        Ok(ProjectPage::Modules(tab(args, ModuleListTab::from_name, &known)?))
      }
      Some("new") => no_tab(args, "New module has no tabs").map(|()| ProjectPage::NewModule),
      Some(other) => Err(ToolError::InvalidParam {
        param: "view",
        reason: format!("`{other}` needs `module`: without one the modules' one view is new, for New module"),
      }),
    };
  };
  if module == "new" {
    if view.is_some() {
      return Err(ToolError::InvalidParam {
        param: "view",
        reason: "New module has no views".to_owned(),
      });
    }
    return no_tab(args, "New module has no tabs").map(|()| ProjectPage::NewModule);
  }
  let id = module_id(module)?;
  match view {
    None => {
      let known = ModuleTab::ALL.map(ModuleTab::name);
      Ok(ProjectPage::Module(id, tab(args, ModuleTab::from_name, &known)?))
    }
    Some("edit") => no_tab(args, "Edit module has no tabs").map(|()| ProjectPage::EditModule(id)),
    Some(other) => Err(ToolError::InvalidParam {
      param: "view",
      reason: format!("`{other}` names no view of a module: its one view is edit"),
    }),
  }
}

/// New task, or with `module` a module's id, New task in that module.
pub(super) fn new_task_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match string(args, "module")? {
    None => Ok(ProjectPage::NewTask),
    Some(module) => module_id(module).map(ProjectPage::NewTaskInModule),
  }
}

fn module_id(module: &str) -> Result<ModuleId, ToolError> {
  module.parse::<ModuleId>().map_err(|error| ToolError::InvalidParam {
    param: "module",
    reason: error.to_string(),
  })
}

/// Refuses a `tab` on a page that has none, saying `why`.
fn no_tab(args: &Map<String, Value>, why: &str) -> Result<(), ToolError> {
  if args.contains_key("tab") {
    return Err(ToolError::InvalidParam {
      param: "tab",
      reason: why.to_owned(),
    });
  }
  Ok(())
}
