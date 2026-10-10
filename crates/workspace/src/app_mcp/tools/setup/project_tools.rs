//! `list_tools`, `get_tool`, `save_tool_draft` and `publish_tool`: a project's tools as the tools library
//! lists and opens them, a command tool made and its draft saved as its editor saves it
//! ([`super::tool_input`]), and published as Publish does, so pipeline steps can call it. Only command
//! tools are made and changed here; an HTTP or database tool is listed by its kind and description alone,
//! nothing of the secrets it uses. Tools are not removed or archived in this version: the project data
//! keeps every resource.

use orchester_core::{
  resource::{CommandAction, CommandArg, PublishError, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Spec, parse, projects::InProject},
  profiles::{no_draft, state_json},
  project::environment_json,
  tool_input::{self, not_a_command_tool, widenings},
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::{self, Message},
  tools::data::{ProjectTools, ToolEntry},
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_tools",
    description: "A project's tools, which pipeline steps call: id, name, kind (command, http or database), \
                  description, where each stands (draft, stale_draft or published, with its published version), \
                  whether a step can run it, and a command tool's command line. Nothing of the secrets an HTTP \
                  or database tool uses is listed.",
    schema: schema_for_type::<InProject>,
    read_only: true,
  },
  Spec {
    name: "get_tool",
    description: "One tool as its editor shows it (its draft if it has one, otherwise its latest version): a \
                  command tool's program, arguments, timeout, exit codes that pass, environment variables and \
                  output limit; where it stands; and which published pipeline steps call it. A command tool runs \
                  in the worktree of the step that calls it, without a shell.",
    schema: schema_for_type::<InTool>,
    read_only: true,
  },
  Spec {
    name: "save_tool_draft",
    description: "Saves a command tool's draft with every field of its editor; without `tool` a new one is made \
                  under `name`. The program is a name found on the search path (the tool's PATH, else the \
                  project's, else the app's) or an absolute path to an existing file, and is stored as the file \
                  found; a path inside the repository is refused (run a script through its interpreter: program \
                  powershell, arguments -File tools/check.ps1). Each argument is passed as it is, no shell reads \
                  it; the step's worktree is the working directory. Left out: timeout 600 seconds, exit code 0, \
                  no variables, output limit 256 KiB. Variables are plain text: never a secret. A command tool \
                  runs a program on this computer whenever a step calls it, so unless a person turned on Let \
                  automation change agent permissions, making one is refused as person_only, as is changing its \
                  program or arguments, letting more exit codes pass, or changing any variable but the harmless \
                  ones set_project_environment names. Its description, timeout and output limit, and fewer exit \
                  codes, are always taken. Publish it with publish_tool.",
    schema: schema_for_type::<tool_input::SaveToolDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_tool",
    description: "Publishes a command tool's draft as its next version, as Publish does, so pipeline steps can call \
                  it (save_pipeline_draft, a step with `tool`). Refused for a draft started from an older version \
                  than the latest, and as draft_changed for a draft that changed while it was checked. Unless a \
                  person turned on Let automation change agent permissions, a tool's first version is refused as \
                  person_only, and so is a draft that runs another program or other arguments than the latest \
                  version, lets more exit codes pass or changes a variable but the harmless ones.",
    schema: schema_for_type::<InTool>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InTool {
  /// The project's id, or its name.
  pub project: String,
  /// The tool's id, or its name.
  pub tool: String,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "list_tools" => list_tools(services, parse(arguments)?).await,
    "get_tool" => get_tool(services, parse(arguments)?).await,
    "save_tool_draft" => tool_input::save_tool_draft(services, parse(arguments)?).await,
    "publish_tool" => publish_tool(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_tools(services: &Services, args: InProject) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let tools = ProjectTools::load_in(&open.repo_root)?;
    let listed: Vec<Value> = tools
      .tools
      .iter()
      .map(|entry| {
        let mut value = summary_json(entry);
        value["command_line"] = json!(command_of(&entry.tool).map(|command| command.command_line()));
        value
      })
      .collect();
    Ok(json!({ "tools": listed }))
  })
  .await
}

async fn get_tool(services: &Services, args: InTool) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let tools = ProjectTools::load_in(&open.repo_root)?;
    let entry = tool_named(&tools, &args.tool)?;
    Ok(json!({ "tool": tool_json(&tools, entry) }))
  })
  .await
}

async fn publish_tool(services: &Services, args: InTool) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let tools = ProjectTools::load(&db)?;
    let entry = tool_named(&tools, &args.tool)?;
    let id = entry.id;
    let draft = db.draft::<Tool>(id)?.ok_or_else(no_draft)?;
    let Some(after) = command_of(draft.body()) else {
      return Err(not_a_command_tool(&entry.name));
    };
    let latest = db.latest_version::<Tool>(id)?;
    let reasons = match latest.as_ref().and_then(|version| command_of(version.body())) {
      Some(before) => widenings(before, after),
      None => vec![Message::new("appMcp.projectTools.widens.firstVersion")],
    };
    widening::allowed(&services, reasons)?;
    // Published only while the draft is the one just checked.
    db.publish_draft_matching(id, draft.body(), Timestamp::now())
      .map_err(|error| publish_error(&error))?;
    let tools = ProjectTools::load(&db)?;
    let entry = tools
      .tool(id)
      .ok_or_else(|| ToolError::not_found(Missing::Tool, id.to_string()))?;
    Ok(json!({ "tool": tool_json(&tools, entry) }))
  })
  .await
}

/// The project's tool named by its id or, ignoring case, its name.
pub(super) fn tool_named<'a>(tools: &'a ProjectTools, named: &str) -> Result<&'a ToolEntry, ToolError> {
  let lowered = named.to_lowercase();
  tools
    .tools
    .iter()
    .find(|entry| entry.id.to_string() == named || entry.name.to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Tool, named))
}

/// A tool as `get_tool` shows it.
pub(super) fn tool_json(tools: &ProjectTools, entry: &ToolEntry) -> Value {
  let mut value = summary_json(entry);
  value["command"] = json!(command_of(&entry.tool).map(|command| json!({
    "program": command.program().as_str(),
    "arguments": command.args().iter().map(CommandArg::as_str).collect::<Vec<_>>(),
    "timeout_secs": command.timeout().secs(),
    "exit_codes": command.exit_codes().iter().collect::<Vec<_>>(),
    "environment": environment_json(command.environment()),
    "output_limit_kib": command.output_limit().kib(),
    "command_line": command.command_line(),
  })));
  value["used_by_steps"] = tools
    .used_by(entry.id)
    .map(|used| json!({ "pipeline": &*used.pipeline_name, "version": used.version.get(), "step": &*used.step }))
    .collect();
  value
}

fn summary_json(entry: &ToolEntry) -> Value {
  json!({
    "id": entry.id.to_string(),
    "name": &*entry.name,
    "kind": entry.tool.kind(),
    "description": entry.tool.description(),
    "state": state_json(entry.state),
    "runnable": entry.tool.kind().runnable() && entry.tool.access().is_none(),
  })
}

fn command_of(tool: &Tool) -> Option<&CommandAction> {
  match tool.action() {
    ToolAction::Command(command) => Some(command),
    ToolAction::Http(_) | ToolAction::DatabaseQuery(_) => None,
  }
}

fn publish_error(error: &StoreError) -> ToolError {
  let code = match error {
    StoreError::NoDraft(_) => return no_draft(),
    StoreError::Publish(PublishError::StaleDraft { .. }) => "stale_draft",
    StoreError::DraftChanged(_) => "draft_changed",
    StoreError::Publish(PublishError::VersionLimit) => "version_limit",
    _ => "project_data",
  };
  ToolError::refused(code, vec![messages::store_error(error)])
}
