//! The command tool editor's fields (06.02 `YEWFo`, for a command) as entered, and their mapping to
//! and from a [`Tool`]: arguments one per line, exit codes separated by commas or spaces and
//! environment variables as `NAME=value` lines. Every value is checked by the core type it becomes.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  name::Name,
  resource::{
    AccessPolicy, CommandAction, CommandArg, CommandEnvironment, CommandError, CommandProgram, CommandTimeout,
    ExitCodes, OutputLimit, Tool, ToolAction, VariableName,
  },
};

use crate::messages::{self, Message};

/// The fields as the user left them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandText {
  pub description: String,
  pub program: String,
  /// One argument per line; empty lines are left out.
  pub arguments: String,
  /// Whole seconds.
  pub timeout: String,
  /// Codes separated by commas or spaces.
  pub exit_codes: String,
  /// Whole KiB.
  pub output_limit: String,
  /// `NAME=value` per line; empty lines are left out.
  pub environment: String,
}

impl CommandText {
  /// A new command tool: no program yet and the defaults of [`CommandAction::new`].
  pub fn new_tool() -> Self {
    Self {
      timeout: CommandTimeout::DEFAULT.secs().to_string(),
      exit_codes: "0".to_owned(),
      output_limit: OutputLimit::DEFAULT.kib().to_string(),
      ..Self::default()
    }
  }

  /// The fields of `tool`'s command; `None` for a tool of another kind.
  pub fn of(tool: &Tool) -> Option<Self> {
    let ToolAction::Command(command) = tool.action() else {
      return None;
    };
    Some(Self {
      description: tool.description().to_owned(),
      program: command.program().as_str().to_owned(),
      arguments: command
        .args()
        .iter()
        .map(CommandArg::as_str)
        .collect::<Vec<_>>()
        .join("\n"),
      timeout: command.timeout().secs().to_string(),
      exit_codes: command
        .exit_codes()
        .iter()
        .map(|code| code.to_string())
        .collect::<Vec<_>>()
        .join(", "),
      output_limit: command.output_limit().kib().to_string(),
      environment: environment_text(command.environment()),
    })
  }

  /// The tool these fields describe. `access` is kept as the stored tool had it; the editor does
  /// not change access rules.
  pub fn tool(&self, access: Option<AccessPolicy>) -> Result<Tool, Box<CommandErrors>> {
    let program = CommandProgram::new(&self.program).map_err(|error| program_message(&error));
    let arguments = self.arguments();
    let timeout = self.timeout();
    let exit_codes = self.exit_codes();
    let output_limit = self.output_limit();
    let environment = self.environment();
    let (Ok(program), Ok(arguments), Ok(timeout), Ok(exit_codes), Ok(output_limit), Ok(environment)) =
      (&program, &arguments, &timeout, &exit_codes, &output_limit, &environment)
    else {
      return Err(Box::new(CommandErrors {
        name: None,
        program: program.err(),
        arguments: arguments.err(),
        timeout: timeout.err(),
        exit_codes: exit_codes.err(),
        output_limit: output_limit.err(),
        environment: environment.err(),
      }));
    };
    let action = CommandAction::new(program.clone(), arguments.clone())
      .with_timeout(*timeout)
      .with_exit_codes(exit_codes.clone())
      .with_output_limit(*output_limit)
      .with_environment(environment.clone());
    Ok(
      Tool::new(&self.description, ToolAction::Command(action), access).expect("a command tool needs no access policy"),
    )
  }

  fn arguments(&self) -> Result<Vec<CommandArg>, Message> {
    parse_arguments(&self.arguments)
  }

  fn timeout(&self) -> Result<CommandTimeout, Message> {
    self
      .timeout
      .trim()
      .parse::<u64>()
      .ok()
      .and_then(|secs| CommandTimeout::from_secs(secs).ok())
      .ok_or_else(|| Message::new("tools.errors.timeout").with("max", CommandTimeout::MAX_HOURS.to_string()))
  }

  fn exit_codes(&self) -> Result<ExitCodes, Message> {
    let codes = self
      .exit_codes
      .split([',', ' '])
      .filter(|code| !code.trim().is_empty())
      .map(|code| code.trim().parse::<i64>())
      .collect::<Result<BTreeSet<i64>, _>>()
      .map_err(|_| Message::new("tools.errors.exitCodeNotNumber"))?;
    ExitCodes::new(codes).map_err(|error| match error {
      CommandError::TooManyExitCodes => {
        Message::new("tools.errors.tooManyExitCodes").with("max", ExitCodes::MAX.to_string())
      }
      _ => Message::new("tools.errors.noExitCodes"),
    })
  }

  fn output_limit(&self) -> Result<OutputLimit, Message> {
    self
      .output_limit
      .trim()
      .parse::<u32>()
      .ok()
      .and_then(|kib| OutputLimit::from_kib(kib).ok())
      .ok_or_else(|| Message::new("tools.errors.outputLimit").with("max", OutputLimit::MAX_KIB.to_string()))
  }

  fn environment(&self) -> Result<CommandEnvironment, Message> {
    parse_environment(&self.environment)
  }
}

/// Arguments one per line, as a command tool and an MCP server take them; empty lines are left out.
pub fn parse_arguments(text: &str) -> Result<Vec<CommandArg>, Message> {
  lines(text)
    .map(|line| CommandArg::new(line).map_err(|error| token_error(&error)))
    .collect()
}

/// Variables as `NAME=value` lines, as a command tool and Project settings › Environment show them.
pub fn environment_text(environment: &CommandEnvironment) -> String {
  environment
    .iter()
    .map(|(name, value)| format!("{name}={value}"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The variables of `NAME=value` lines; empty lines are left out.
pub fn parse_environment(text: &str) -> Result<CommandEnvironment, Message> {
  let mut variables = BTreeMap::new();
  for line in lines(text) {
    let (name, value) = line
      .split_once('=')
      .ok_or_else(|| Message::new("tools.errors.variableLine").with("line", line.to_owned()))?;
    let name = VariableName::new(name.trim()).map_err(|error| variable_error(&error))?;
    if variables.insert(name, value.to_owned()).is_some() {
      return Err(Message::new("tools.errors.duplicateVariable").with("name", name_of(line)));
    }
  }
  CommandEnvironment::new(variables).map_err(|error| variable_error(&error))
}

/// Why the tool cannot be saved yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandErrors {
  pub name: Option<Message>,
  pub program: Option<Message>,
  pub arguments: Option<Message>,
  pub timeout: Option<Message>,
  pub exit_codes: Option<Message>,
  pub output_limit: Option<Message>,
  pub environment: Option<Message>,
}

/// The name of a new tool as typed.
pub fn tool_name(text: &str) -> Result<Name, Message> {
  Name::new(text).map_err(|error| messages::name_error(&error))
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
  text.lines().filter(|line| !line.trim().is_empty())
}

fn name_of(line: &str) -> String {
  line.split_once('=').map_or(line, |(name, _)| name).trim().to_owned()
}

/// Why a command tool's or an MCP server's program cannot be used.
pub fn program_message(error: &CommandError) -> Message {
  match error {
    CommandError::BlankProgram => Message::new("tools.errors.programRequired"),
    CommandError::OutsideWorktree(_) => Message::new("tools.errors.programOutsideWorktree"),
    other => token_error(other),
  }
}

fn token_error(error: &CommandError) -> Message {
  match error {
    CommandError::TooLong(_) => Message::new("tools.errors.tooLong"),
    CommandError::EmptyArgument => Message::new("tools.errors.emptyArgument"),
    _ => Message::new("tools.errors.controlCharacter"),
  }
}

fn variable_error(error: &CommandError) -> Message {
  match error {
    CommandError::DuplicateVariable(name) => Message::new("tools.errors.duplicateVariable").with("name", name.clone()),
    CommandError::VariableValue(name) => Message::new("tools.errors.variableValue").with("name", name.clone()),
    CommandError::VariableName(name) => Message::new("tools.errors.variableName").with("name", name.clone()),
    _ => Message::new("tools.errors.variableLine").with("line", String::new()),
  }
}
