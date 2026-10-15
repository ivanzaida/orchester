//! `list_tools`, `get_tool`, `save_tool_draft` and `publish_tool`: a project's tools as the tools library
//! lists and opens them, a command tool made and its draft saved as its editor saves it
//! ([`super::tool_input`]), and published as Publish does, so pipeline steps can call it. Only command
//! tools are made and changed here; an HTTP or database tool is listed by its kind and description alone,
//! nothing of the secrets it uses. Tools are not removed or archived in this version: the project data
//! keeps every resource.

use orchester_core::{
  id::ProjectId,
  resource::{CommandAction, CommandArg, PublishError, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Spec, Translate,
    across::{InProjects, across},
    parse,
  },
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
  links::Link,
  messages::{self, Message},
  tasks::outside::Outside,
  tools::data::{ProjectTools, ToolEntry},
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_tools",
    description: "A project's tools, which pipeline steps call: id, name, kind (command, http or database), \
                  description, where each stands (draft, stale_draft or published, with its published version), \
                  whether a step can run it, a command tool's command line, and its link in the app (its \
                  editor). Nothing of the secrets an HTTP or database tool uses is listed. Without `project`, \
                  every project's, each naming its project (unreadable_projects lists the projects that could \
                  not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_tool",
    description: "One tool as its editor shows it (its draft if it has one, otherwise its latest version): a \
                  command tool's program, arguments, timeout, exit codes that pass, environment variables and \
                  output limit; where it stands; which published pipeline steps call it; and its link in the \
                  app. A command tool runs \
                  in the worktree of the step that calls it, without a shell, with the person's rights, network \
                  access and no sandbox.",
    schema: schema_for_type::<InTool>,
    read_only: true,
  },
  Spec {
    name: "save_tool_draft",
    description: "Saves a command tool's draft as its editor does; without `tool` a new one is made under \
                  `name`. On an existing tool a field left out keeps its value; a new tool needs `program` and \
                  otherwise gets no description, no arguments, timeout 600 seconds, exit code 0, no variables \
                  and output limit 256 KiB. The program is a name found on the search path (the tool's PATH, \
                  else the project's, else the app's; relative folders are skipped) or an absolute path to an \
                  existing file outside the project's repository, and is stored as the file found, so the \
                  program itself is a fixed file; a path inside the repository is refused (run a script through \
                  its interpreter: program powershell, arguments -File tools/check.ps1). That is not a sandbox: \
                  the step runs the program in its worktree with your rights, network access and no sandbox, \
                  and the program reads files the agents before it wrote (scripts, build files, configuration). \
                  Each argument is passed as it is, no shell reads it. Variables are plain text: never a secret. \
                  Unless a person turned on Let automation change agent permissions, making a tool is refused as \
                  person_only, as is changing its program or arguments, letting more exit codes pass, or \
                  changing any variable but the harmless ones set_project_environment names. Its description, \
                  timeout and output limit, and fewer exit codes, are always taken. Publish it with publish_tool.",
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
                  version, lets more exit codes pass or changes a variable but the harmless ones: a step that \
                  calls it runs its program with the person's rights, network access and no sandbox, on files \
                  the agents before it wrote.",
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
pub(super) async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_tools" => list_tools(services, parse(arguments)?, translate).await,
    "get_tool" => get_tool(services, parse(arguments)?).await,
    "save_tool_draft" => tool_input::save_tool_draft(services, parse(arguments)?).await,
    "publish_tool" => publish_tool(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_tools(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || across(&services, args.project.as_deref(), |_, db| Ok(ProjectTools::load(db)?))).await?;
  Ok(read.json("tools", translate, |open, tools| {
    tools
      .tools
      .iter()
      .map(|entry| {
        let mut value = summary_json(entry, open.project.id());
        value["command_line"] = json!(command_of(&entry.tool).map(|command| command.command_line()));
        value
      })
      .collect()
  }))
}

async fn get_tool(services: &Services, args: InTool) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let tools = ProjectTools::load_in(&open.repo_root)?;
    let entry = tool_named(&tools, &args.tool)?;
    Ok(json!({ "tool": tool_json(&tools, entry, open.project.id()) }))
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
    services.changes.record(open.project.id(), Outside::Tools);
    let tools = ProjectTools::load(&db)?;
    let entry = tools
      .tool(id)
      .ok_or_else(|| ToolError::not_found(Missing::Tool, id.to_string()))?;
    Ok(json!({ "tool": tool_json(&tools, entry, open.project.id()) }))
  })
  .await
}

/// The project's tool named by its id or else, ignoring case, by its name.
pub(super) fn tool_named<'a>(tools: &'a ProjectTools, named: &str) -> Result<&'a ToolEntry, ToolError> {
  let lowered = named.to_lowercase();
  // An id names one tool for certain; a name could also be another tool's id.
  let by_id = tools.tools.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| tools.tools.iter().find(|entry| entry.name.to_lowercase() == lowered))
    .ok_or_else(|| ToolError::not_found(Missing::Tool, named))
}

/// A tool of `project` as `get_tool` shows it.
pub(super) fn tool_json(tools: &ProjectTools, entry: &ToolEntry, project: ProjectId) -> Value {
  let mut value = summary_json(entry, project);
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

/// A tool of `project` as `list_tools` lists it, with its link: its editor.
fn summary_json(entry: &ToolEntry, project: ProjectId) -> Value {
  json!({
    "id": entry.id.to_string(),
    "name": &*entry.name,
    "kind": entry.tool.kind(),
    "description": entry.tool.description(),
    "state": state_json(entry.state),
    "runnable": entry.runnable(),
    "link": Link::Tool(project, entry.id).path(),
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
