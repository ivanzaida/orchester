//! Orchester's domain tools on the app's MCP server: automation sets a project up (connections, agent
//! profiles, pipelines, instructions, project settings) and drives its work (tasks, runs, approvals,
//! questions, plans, merges) through data, beside lurq's window tools. The tools themselves are
//! `orchester-workspace`'s (`app_mcp`), which go through the services the screens use, so they follow
//! the same rules and write the same records; this module only serves them. Each runs on the server's
//! runtime (`McpTool::async_handler`), off the event loop, and the screens learn what it changed through
//! the session's [`OutsideChanges`](orchester_workspace::tasks::outside::OutsideChanges).
//!
//! Reads are in lurq's Observe scope. Writes are in the [`DATA_SCOPE`], which is always granted: changing
//! project data is not controlling the window, and the tools keep the app's own safety rules (automation
//! answers approvals only while a person allows it, and never widens a run's permissions).

use std::sync::Arc;

use lurq::{
  app::i18n::I18n,
  mcp::{McpTool, Scope, rmcp::model::Tool},
};
use orchester_workspace::{
  app_mcp::{self, Translate},
  i18n::NAMESPACE,
};
use serde_json::Value;

use crate::services::Services;

/// The scope of the domain tools that change data.
pub const DATA_SCOPE: &str = "data";

/// Every domain tool on the session's `services`, explaining refusals in the words of `i18n`.
pub fn tools(services: &Services, i18n: I18n) -> Vec<McpTool> {
  let domain = Arc::new(
    app_mcp::Services::new(
      services.workspace.clone(),
      services.runners.clone(),
      services.servers.clone(),
      services.programs.clone(),
      (
        services.changes.clone(),
        services.activity.clone(),
        services.jobs.clone(),
      ),
    )
    .with_vault(services.vault.clone()),
  );
  let translate = Arc::new(Translate::new(move |message| {
    let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
    i18n.t_ns_args(NAMESPACE, message.key, args).to_string()
  }));
  let work = app_mcp::listed().into_iter().map(|tool| (Set::Work, tool));
  let setup = app_mcp::setup::listed().into_iter().map(|tool| (Set::Setup, tool));
  work
    .chain(setup)
    .map(|(set, tool)| served(set, &tool, &domain, &translate))
    .collect()
}

/// Which of `orchester-workspace`'s lists a tool is in: the tools that drive work, which the deprecated
/// desktop serves too, or those that set a project up, which only this app serves.
#[derive(Clone, Copy)]
enum Set {
  Work,
  Setup,
}

/// `tool` as lurq serves it: its result as JSON, or its error's `{code, message, reasons}`.
fn served(set: Set, tool: &Tool, services: &Arc<app_mcp::Services>, translate: &Arc<Translate>) -> McpTool {
  let read_only = tool.annotations.as_ref().and_then(|hints| hints.read_only_hint) == Some(true);
  let name = tool.name.to_string();
  let mut served = McpTool::new(name.clone())
    .description(tool.description.as_deref().unwrap_or_default())
    .input_schema(Value::Object((*tool.input_schema).clone()));
  served = if read_only {
    served.scope(Scope::Observe).read_only()
  } else {
    served.scope(DATA_SCOPE)
  };
  let (services, translate) = (Arc::clone(services), Arc::clone(translate));
  served.async_handler(move |arguments| {
    let (services, translate, name) = (Arc::clone(&services), Arc::clone(&translate), name.clone());
    async move { call(set, (&services, &translate), &name, arguments).await }
  })
}

async fn call(
  set: Set,
  (services, translate): (&app_mcp::Services, &Translate),
  name: &str,
  arguments: Value,
) -> Result<Value, String> {
  let arguments = match arguments {
    Value::Object(arguments) => arguments,
    Value::Null => serde_json::Map::new(),
    _ => return Err(invalid("the arguments must be a JSON object")),
  };
  let result = match set {
    Set::Work => app_mcp::call(services, translate, name, arguments).await,
    Set::Setup => app_mcp::setup::call(services, translate, name, arguments).await,
  };
  match result {
    Some(Ok(value)) => Ok(value),
    Some(Err(error)) => {
      tracing::info!(tool = %name, ?error, "a domain tool refused");
      let refused = error.to_json(|message| translate.text(message));
      Err(refused.get("error").unwrap_or(&refused).to_string())
    }
    None => Err(invalid(&format!("no tool is named {name}"))),
  }
}

fn invalid(message: &str) -> String {
  serde_json::json!({ "code": "invalid_arguments", "message": message, "reasons": [] }).to_string()
}
