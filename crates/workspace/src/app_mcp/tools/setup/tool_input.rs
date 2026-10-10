//! `save_tool_draft`: a command tool's editor through Orchester's MCP server. Every field is checked by
//! the core type it becomes, with the editor's words; the program is looked up as a step would start it
//! and stored as found, so what a person allows is the file that runs. A command tool runs a program on
//! this computer whenever a pipeline step calls it, so making one, or changing what it runs, is a
//! person's decision unless a person lets automation make it.

use std::{ffi::OsStr, path::Path};

use orchester_core::{
  connection::invocation::Platform,
  name::Name,
  resource::{
    AccessPolicy, CommandAction, CommandArg, CommandEnvironment, CommandProgram, CommandTimeout, DraftSaved, ExitCodes,
    OutputLimit, ProgramLocation, Tool, ToolAction,
  },
  store::SettingsStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  project::{Variable, changed_variables, environment},
  project_tools::{tool_json, tool_named},
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::save_draft_in,
  messages::{self, Message},
  tools::{
    data::{ProjectTools, create_tool},
    form::program_message,
  },
};

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
  /// What the tool does, for the people and agents who choose it.
  #[serde(default)]
  pub description: String,
  /// The program: a name found on the search path (`cargo`, `powershell`) or an absolute path to a file.
  /// It is stored as the file found.
  pub program: String,
  /// The arguments, each passed to the program as it is: no shell reads them.
  #[serde(default)]
  pub arguments: Vec<String>,
  /// How long it may run, in seconds (1 up to 24 hours); left out, 600.
  pub timeout_secs: Option<u64>,
  /// The exit codes that pass; left out, 0 only.
  pub exit_codes: Option<Vec<i64>>,
  /// Variables the tool adds for its program; plain text, so never a secret.
  #[serde(default)]
  pub environment: Vec<Variable>,
  /// How much of its output the run keeps, in KiB (1 up to 16384); left out, 256.
  pub output_limit_kib: Option<u32>,
}

pub async fn save_tool_draft(services: &Services, args: SaveToolDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let tools = ProjectTools::load(&db)?;
    let existing = args
      .tool
      .as_deref()
      .map(|named| tool_named(&tools, named))
      .transpose()?;
    let action = command(&args, &db.project_environment()?)?;
    let (id, saved) = match existing {
      Some(_) if args.name.is_some() => {
        return Err(refused(
          "name_not_editable",
          Message::new("appMcp.projectTools.errors.nameNotEditable"),
        ));
      }
      Some(entry) => {
        let ToolAction::Command(before) = entry.tool.action() else {
          return Err(not_a_command_tool(&entry.name));
        };
        widening::allowed(&services, widenings(before, &action))?;
        let tool = command_tool(&args.description, action, entry.tool.access().cloned());
        (entry.id, save_draft_in(&open.repo_root, entry.id, tool)?)
      }
      None => {
        let name = new_name(&tools, args.name.as_deref())?;
        widening::allowed(&services, vec![Message::new("appMcp.projectTools.widens.newTool")])?;
        let tool = command_tool(&args.description, action, None);
        (
          create_tool(&mut db, &name, &tool, Timestamp::now())?,
          DraftSaved::Stored,
        )
      }
    };
    let tools = ProjectTools::load_in(&open.repo_root)?;
    let entry = tools
      .tool(id)
      .ok_or_else(|| ToolError::not_found(Missing::Tool, id.to_string()))?;
    let mut value = tool_json(&tools, entry);
    value["draft_kept"] = json!(saved == DraftSaved::Stored);
    Ok(json!({ "tool": value }))
  })
  .await
}

/// What `after` runs that `before` did not: another program, other arguments, exit codes that now pass,
/// or variables changed but harmless ones. Narrower exit codes, the description, the timeout and the
/// output limit change nothing a person must decide.
pub(super) fn widenings(before: &CommandAction, after: &CommandAction) -> Vec<Message> {
  let mut reasons = Vec::new();
  if before.program() != after.program() {
    reasons.push(
      Message::new("appMcp.projectTools.widens.program")
        .with("program", after.program().as_str().to_owned())
        .with("before", before.program().as_str().to_owned()),
    );
  }
  if before.args() != after.args() {
    reasons.push(Message::new("appMcp.projectTools.widens.arguments"));
  }
  let passing: Vec<String> = after
    .exit_codes()
    .iter()
    .filter(|code| !before.exit_codes().contains(*code))
    .map(|code| code.to_string())
    .collect();
  if !passing.is_empty() {
    reasons.push(Message::new("appMcp.projectTools.widens.exitCodes").with("codes", passing.join(", ")));
  }
  reasons.extend(changed_variables(before.environment(), after.environment()));
  reasons
}

/// The command `args` describe, its program found as a step would start it with `project`'s variables.
fn command(args: &SaveToolDraft, project: &CommandEnvironment) -> Result<CommandAction, ToolError> {
  let invalid = |reason: Message| refused("invalid_tool", reason);
  let program = CommandProgram::new(&args.program).map_err(|error| invalid(program_message(&error)))?;
  let arguments = args
    .arguments
    .iter()
    .map(|argument| CommandArg::new(argument).map_err(|error| invalid(program_message(&error))))
    .collect::<Result<Vec<_>, _>>()?;
  let timeout = CommandTimeout::from_secs(args.timeout_secs.unwrap_or(CommandTimeout::DEFAULT.secs()))
    .map_err(|_| invalid(Message::new("tools.errors.timeout").with("max", CommandTimeout::MAX_HOURS.to_string())))?;
  let exit_codes = match &args.exit_codes {
    None => ExitCodes::success(),
    Some(codes) => ExitCodes::new(codes.iter().copied().collect()).map_err(|_| exit_codes_error(codes))?,
  };
  let output_limit = OutputLimit::from_kib(args.output_limit_kib.unwrap_or(OutputLimit::DEFAULT.kib()))
    .map_err(|_| invalid(Message::new("tools.errors.outputLimit").with("max", OutputLimit::MAX_KIB.to_string())))?;
  let environment = environment(args.environment.clone())?;
  // The PATH a step would look the program up in: the tool's own, the project's, or the app's.
  let path = CommandAction::new(program.clone(), Vec::new())
    .with_environment(environment.clone())
    .search_path(project, orchester_exec::search_path::search_path());
  let found = found_program(&program, path.as_deref())?;
  Ok(
    CommandAction::new(found, arguments)
      .with_timeout(timeout)
      .with_exit_codes(exit_codes)
      .with_output_limit(output_limit)
      .with_environment(environment),
  )
}

/// The file `program` is: the first copy on `path` for a name, an absolute path as it is (on Windows
/// also with a program extension). A path in the worktree is refused: each run's worktree holds its own
/// copy, which the agents before the step could change, so a person could not tell what runs.
fn found_program(program: &CommandProgram, path: Option<&OsStr>) -> Result<CommandProgram, ToolError> {
  if matches!(program.location(), ProgramLocation::InWorktree(_)) {
    return Err(refused(
      "program_in_worktree",
      Message::new("appMcp.projectTools.errors.programInWorktree"),
    ));
  }
  let not_found = || {
    refused(
      "program_not_found",
      Message::new("appMcp.projectTools.errors.programNotFound").with("program", program.as_str().to_owned()),
    )
  };
  // The worktree is not looked in: a name or an absolute path does not depend on it.
  let found = program
    .candidates(Path::new(""), path, Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(not_found)?;
  let found = found.to_str().ok_or_else(not_found)?;
  CommandProgram::new(found).map_err(|error| refused("invalid_tool", program_message(&error)))
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
