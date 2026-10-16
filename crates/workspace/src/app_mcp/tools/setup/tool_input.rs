//! `save_tool_draft`: a command tool's editor through Orchester's MCP server. Every field is checked by
//! the core type it becomes, with the editor's words, and the program is kept as a fixed file
//! ([`super::tool_program`]). A command step runs its program with the person's rights, network access
//! and no sandbox, on files the agents before it wrote, so making a tool, or changing what it runs, is a
//! person's decision unless a person lets automation make it.

use std::path::Path;

use orchester_core::{
  name::Name,
  resource::{
    AccessPolicy, CommandAction, CommandArg, CommandEnvironment, CommandProgram, CommandTimeout, DraftSaved, ExitCodes,
    OutputLimit, Tool, ToolAction,
  },
  store::SettingsStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  project::{Variable, environment},
  project_tools::{tool_json, tool_named, tools_with_revision},
  tool_program::found_program,
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::save_draft_expecting_in,
  messages::{self, Message},
  tools::{
    data::{ProjectTools, ToolEntry, create_tool},
    form::program_message,
  },
};

/// On an existing tool, a field left out keeps its value; on a new one it takes the default named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveToolDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The command tool to change, by id or name; left out, a new one is made.
  pub tool: Option<String>,
  /// A new tool's name, unique among the project's tools; a tool keeps the name it was made with.
  pub name: Option<String>,
  /// What the tool does, for the people and agents who choose it; a new tool's default is none.
  pub description: Option<String>,
  /// The program: a name found on the search path (`cargo`, `powershell`) or an absolute path to a file
  /// outside the project's repository. It is stored as the file found. Required for a new tool.
  pub program: Option<String>,
  /// The arguments, each passed to the program as it is: no shell reads them. A new tool's default is
  /// none.
  pub arguments: Option<Vec<String>>,
  /// How long it may run, in seconds (1 up to 24 hours); a new tool's default is 600.
  pub timeout_secs: Option<u64>,
  /// The exit codes that pass; a new tool's default is 0 only.
  pub exit_codes: Option<Vec<i64>>,
  /// Every variable the tool adds for its program, replacing those it has; plain text, so never a
  /// secret. A new tool's default is none.
  pub environment: Option<Vec<Variable>>,
  /// How much of its output the run keeps, in KiB (1 up to 16384); a new tool's default is 256.
  pub output_limit_kib: Option<u32>,
  /// The tool's `revision` as get_tool or the last save returned it: the draft is saved only while it is
  /// unchanged, else refused as draft_changed. Left out, the save goes over a change saved meanwhile.
  pub expected_revision: Option<String>,
}

pub async fn save_tool_draft(services: &Services, args: SaveToolDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let tools = ProjectTools::load(&db)?;
    let project = db.project_environment()?;
    let existing = args
      .tool
      .as_deref()
      .map(|named| tool_named(&tools, named))
      .transpose()?;
    let (id, saved) = match existing {
      Some(entry) => {
        let tool = changed_tool(&services, &args, entry, (&project, &open.repo_root))?;
        let expected = args.expected_revision.as_deref();
        (
          entry.id,
          save_draft_expecting_in(&open.repo_root, entry.id, tool, expected)?,
        )
      }
      None if args.expected_revision.is_some() => {
        return Err(ToolError::InvalidArguments(
          "expected_revision names a tool's draft; a new tool has none".into(),
        ));
      }
      None => {
        let action = command(&args, None, &project, &open.repo_root)?;
        let name = new_name(&tools, args.name.as_deref())?;
        widening::allowed(&services, vec![Message::new("appMcp.projectTools.widens.newTool")])?;
        let tool = command_tool(args.description.as_deref().unwrap_or_default(), action, None);
        (
          create_tool(&mut db, &name, &tool, Timestamp::now())?,
          DraftSaved::Stored,
        )
      }
    };
    let (tools, revision) = tools_with_revision(&open.repo_root, id)?;
    let entry = tools
      .tool(id)
      .ok_or_else(|| ToolError::not_found(Missing::Tool, id.to_string()))?;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::Tools);
    let mut value = tool_json(&tools, entry, open.project.id());
    value["draft_kept"] = json!(saved == DraftSaved::Stored);
    value["revision"] = json!(revision);
    Ok(json!({ "tool": value }))
  })
  .await
}

/// `entry` with the fields `args` give, unless it widens what the tool runs and no person lets it.
fn changed_tool(
  services: &Services,
  args: &SaveToolDraft,
  entry: &ToolEntry,
  (project, repository): (&CommandEnvironment, &Path),
) -> Result<Tool, ToolError> {
  if args.name.is_some() {
    return Err(refused(
      "name_not_editable",
      Message::new("appMcp.projectTools.errors.nameNotEditable"),
    ));
  }
  let ToolAction::Command(before) = entry.tool.action() else {
    return Err(not_a_command_tool(&entry.name));
  };
  let action = command(args, Some(before), project, repository)?;
  widening::allowed(services, widenings(before, &action))?;
  let description = args.description.as_deref().unwrap_or(entry.tool.description());
  Ok(command_tool(description, action, entry.tool.access().cloned()))
}

pub(super) use crate::tools::widening::widenings;

/// The command `args` describe over `before` (the tool's command; none for a new tool), its program
/// found as a step would start it with `project`'s variables, outside `repository`.
fn command(
  args: &SaveToolDraft,
  before: Option<&CommandAction>,
  project: &CommandEnvironment,
  repository: &Path,
) -> Result<CommandAction, ToolError> {
  let invalid = |reason: Message| refused("invalid_tool", reason);
  let environment = match &args.environment {
    Some(variables) => environment(variables.clone())?,
    None => before.map(|before| before.environment().clone()).unwrap_or_default(),
  };
  let program = match (&args.program, before) {
    (Some(text), _) => {
      let program = CommandProgram::new(text).map_err(|error| invalid(program_message(&error)))?;
      // The PATH a step would look the program up in: the tool's own, the project's, or the app's.
      let path = CommandAction::new(program.clone(), Vec::new())
        .with_environment(environment.clone())
        .search_path(project, orchester_exec::search_path::search_path());
      found_program(&program, path.as_deref(), repository)?
    }
    (None, Some(before)) => before.program().clone(),
    (None, None) => return Err(invalid(Message::new("tools.errors.programRequired"))),
  };
  let arguments = match &args.arguments {
    Some(arguments) => arguments
      .iter()
      .map(|argument| CommandArg::new(argument).map_err(|error| invalid(program_message(&error))))
      .collect::<Result<Vec<_>, _>>()?,
    None => before.map(|before| before.args().to_vec()).unwrap_or_default(),
  };
  let timeout = match args.timeout_secs {
    Some(secs) => CommandTimeout::from_secs(secs)
      .map_err(|_| invalid(Message::new("tools.errors.timeout").with("max", CommandTimeout::MAX_HOURS.to_string())))?,
    None => before.map_or(CommandTimeout::DEFAULT, CommandAction::timeout),
  };
  let exit_codes = match &args.exit_codes {
    Some(codes) => ExitCodes::new(codes.iter().copied().collect()).map_err(|_| exit_codes_error(codes))?,
    None => before.map_or_else(ExitCodes::success, |before| before.exit_codes().clone()),
  };
  let output_limit = match args.output_limit_kib {
    Some(kib) => OutputLimit::from_kib(kib)
      .map_err(|_| invalid(Message::new("tools.errors.outputLimit").with("max", OutputLimit::MAX_KIB.to_string())))?,
    None => before.map_or(OutputLimit::DEFAULT, CommandAction::output_limit),
  };
  Ok(
    CommandAction::new(program, arguments)
      .with_timeout(timeout)
      .with_exit_codes(exit_codes)
      .with_output_limit(output_limit)
      .with_environment(environment),
  )
}

fn exit_codes_error(codes: &[i64]) -> ToolError {
  let reason = if codes.is_empty() {
    Message::new("tools.errors.noExitCodes")
  } else {
    Message::new("tools.errors.tooManyExitCodes").with("max", ExitCodes::MAX.to_string())
  };
  refused("invalid_tool", reason)
}

/// A new tool's name: given, valid, and no other tool's, ignoring case, so tools are named apart.
fn new_name(tools: &ProjectTools, name: Option<&str>) -> Result<Name, ToolError> {
  let name =
    Name::new(name.unwrap_or_default()).map_err(|error| refused("invalid_name", messages::name_error(&error)))?;
  let lowered = name.as_str().to_lowercase();
  if tools.tools.iter().any(|entry| entry.name.to_lowercase() == lowered) {
    return Err(refused(
      "name_taken",
      Message::new("appMcp.projectTools.errors.nameTaken").with("name", name.as_str().to_owned()),
    ));
  }
  Ok(name)
}

fn command_tool(description: &str, action: CommandAction, access: Option<AccessPolicy>) -> Tool {
  Tool::new(description, ToolAction::Command(action), access).expect("a command tool needs no access policy")
}

pub(super) fn not_a_command_tool(name: &str) -> ToolError {
  refused(
    "not_a_command_tool",
    Message::new("appMcp.projectTools.errors.notACommandTool").with("tool", name.to_owned()),
  )
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
