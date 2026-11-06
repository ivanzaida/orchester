//! The test suite editor's fields as a person leaves them (`04 · Test suite editor` `uxYcD`): a new suite's name, the
//! program, its arguments one per line, the filter (the one argument that holds `{{filter}}`, kept where it stood
//! among the others), the working folder, the variables, the report's format and where it is read from, the layer,
//! what it needs (nothing, services, or a stack), the artifacts it keeps, its timeout in minutes and whether every Test
//! step runs it. Every value is checked by the core type it becomes.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{
    ArtifactGlob, CommandArg, CommandError, CommandProgram, CommandTimeout, ReportFormat, ReportOutput, Slug,
    SuiteCommand, SuiteNeeds, SuitePath, SuiteVariable, SuiteVariables, TestLayer, TestReport, TestSuite,
    TestSuiteError, VariableName,
  },
  secret::SecretName,
};

use super::super::editor::form::VariableLine;
use crate::{
  messages::Message,
  tools::form::{parse_arguments, program_message, variable_error},
};

/// What stands in the filter's argument.
const FILTER: &str = "{{filter}}";

/// Where the report is read from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadFrom {
  #[default]
  Printed,
  File,
}

/// What the suite needs started before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NeedsKind {
  #[default]
  Nothing,
  Services,
  Stack,
}

/// The fields as the person left them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteForm {
  /// Only a new suite's; a suite keeps the name it was made with.
  pub name: String,
  pub program: String,
  /// One argument per line, the filter's left out; empty lines are left out.
  pub arguments: String,
  /// The argument that holds `{{filter}}`; empty for none.
  pub filter: String,
  /// Where among the arguments the filter's stands, from 0; past the last, it comes last.
  pub filter_at: usize,
  pub working_folder: String,
  pub variables: Vec<VariableLine>,
  pub format: ReportFormat,
  pub read_from: ReadFrom,
  /// The report's file, while it is read from one.
  pub report_path: String,
  pub layer: TestLayer,
  pub needs: NeedsKind,
  /// The services it needs, in order, while it needs services.
  pub services: Vec<String>,
  /// The stack it needs, while it needs a stack.
  pub stack: String,
  pub artifacts: Vec<String>,
  /// Whole minutes.
  pub timeout: String,
  pub always: bool,
}

impl Default for SuiteForm {
  fn default() -> Self {
    Self {
      name: String::new(),
      program: String::new(),
      arguments: String::new(),
      filter: String::new(),
      filter_at: usize::MAX,
      working_folder: String::new(),
      variables: Vec::new(),
      format: ReportFormat::CargoJson,
      read_from: ReadFrom::Printed,
      report_path: String::new(),
      layer: TestLayer::Unit,
      needs: NeedsKind::Nothing,
      services: Vec::new(),
      stack: String::new(),
      artifacts: Vec::new(),
      timeout: minutes(CommandTimeout::DEFAULT),
      always: false,
    }
  }
}

/// Why the fields do not make a suite yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuiteProblems {
  pub name: Option<Message>,
  pub program: Option<Message>,
  pub arguments: Option<Message>,
  pub filter: Option<Message>,
  pub working_folder: Option<Message>,
  pub variables: Option<Message>,
  pub report: Option<Message>,
  pub needs: Option<Message>,
  pub artifacts: Option<Message>,
  pub timeout: Option<Message>,
}

/// `result`'s value, or `None` with its error kept in `slot`.
fn kept<T>(result: Result<T, Message>, slot: &mut Option<Message>) -> Option<T> {
  result.map_err(|error| *slot = Some(error)).ok()
}

/// `timeout` in the field's words: whole minutes, or minutes and seconds (`1:30`) when it is not whole, which the field
/// keeps while it is left as it is.
fn minutes(timeout: CommandTimeout) -> String {
  let secs = timeout.secs();
  if secs.is_multiple_of(60) {
    (secs / 60).to_string()
  } else {
    format!("{}:{:02}", secs / 60, secs % 60)
  }
}

impl SuiteForm {
  /// The fields of `suite` as stored; a need that is one of `stacks` alone is a stack's.
  pub fn of(suite: &TestSuite, stacks: &[&str]) -> Self {
    let args = suite.command().args();
    let filter_at = args.iter().position(|arg| arg.as_str().contains(FILTER));
    let needs: Vec<String> = suite
      .needs()
      .as_slice()
      .iter()
      .map(|need| need.as_str().to_owned())
      .collect();
    let (kind, services, stack) = match needs.as_slice() {
      [] => (NeedsKind::Nothing, Vec::new(), String::new()),
      [one] if stacks.contains(&one.as_str()) => (NeedsKind::Stack, Vec::new(), one.clone()),
      _ => (NeedsKind::Services, needs, String::new()),
    };
    Self {
      name: String::new(),
      program: suite.command().program().as_str().to_owned(),
      arguments: args
        .iter()
        .enumerate()
        .filter(|(at, _)| Some(*at) != filter_at)
        .map(|(_, arg)| arg.as_str())
        .collect::<Vec<_>>()
        .join("\n"),
      filter: filter_at.map_or_else(String::new, |at| args[at].as_str().to_owned()),
      filter_at: filter_at.unwrap_or(usize::MAX),
      working_folder: suite
        .working_folder()
        .map_or_else(String::new, |path| path.as_str().to_owned()),
      variables: suite
        .variables()
        .iter()
        .map(|(name, value)| match value {
          SuiteVariable::Plain(text) => VariableLine {
            name: name.as_str().to_owned(),
            value: text.clone(),
            secret: false,
          },
          SuiteVariable::Secret(secret) => VariableLine {
            name: name.as_str().to_owned(),
            value: secret.as_str().to_owned(),
            secret: true,
          },
        })
        .collect(),
      format: suite.report().format(),
      read_from: if suite.report().path().is_some() {
        ReadFrom::File
      } else {
        ReadFrom::Printed
      },
      report_path: suite
        .report()
        .path()
        .map_or_else(String::new, |path| path.as_str().to_owned()),
      layer: suite.layer(),
      needs: kind,
      services,
      stack,
      artifacts: suite.artifacts().iter().map(|glob| glob.as_str().to_owned()).collect(),
      timeout: minutes(suite.timeout()),
      always: suite.always(),
    }
  }

  /// Where a format's report is read from once it is chosen: where it must be, or as it was where it may be either.
  pub const fn read_from_for(format: ReportFormat, was: ReadFrom) -> ReadFrom {
    match format.output() {
      ReportOutput::Printed => ReadFrom::Printed,
      ReportOutput::File => ReadFrom::File,
      ReportOutput::PrintedOrFile => was,
    }
  }

  /// The names the suite needs, as the chosen kind of need says.
  pub fn needed(&self) -> Vec<String> {
    match self.needs {
      NeedsKind::Nothing => Vec::new(),
      NeedsKind::Services => self.services.clone(),
      NeedsKind::Stack => vec![self.stack.clone()],
    }
  }

  /// The suite these fields describe. `base` is the stored suite, whose name it keeps and whose timeout stays exact
  /// while its field shows it unchanged; without one, a new suite under the typed name.
  pub fn suite(&self, base: Option<&TestSuite>) -> Result<TestSuite, Box<SuiteProblems>> {
    let mut problems = SuiteProblems::default();
    let named = base.is_some() || kept(suite_name(&self.name), &mut problems.name).is_some();
    let command = self.command(&mut problems);
    let folder = kept(optional_path(&self.working_folder), &mut problems.working_folder);
    let variables = kept(self.variables(), &mut problems.variables);
    let report = kept(self.report(), &mut problems.report);
    let needs = kept(self.needs(), &mut problems.needs);
    let artifacts = kept(self.artifacts(), &mut problems.artifacts);
    let timeout = kept(self.timeout(base), &mut problems.timeout);
    let (true, Some(command), Some(folder), Some(variables), Some(report), Some(needs), Some(artifacts), Some(timeout)) =
      (named, command, folder, variables, report, needs, artifacts, timeout)
    else {
      return Err(Box::new(problems));
    };
    Ok(
      TestSuite::new(command, report, self.layer)
        .with_working_folder(folder)
        .with_variables(variables)
        .with_needs(needs)
        .with_artifacts(artifacts)
        .with_timeout(timeout)
        .run_always(self.always),
    )
  }

  /// The program with its arguments and the filter's among them, each field's problem kept where it is.
  fn command(&self, problems: &mut SuiteProblems) -> Option<SuiteCommand> {
    let program = CommandProgram::new(&self.program).map_err(|error| program_message(&error));
    let program = kept(program, &mut problems.program);
    let arguments = kept(parse_arguments(&self.arguments), &mut problems.arguments);
    let filter = kept(self.filter_argument(), &mut problems.filter);
    let (program, arguments, filter) = (program?, arguments?, filter?);
    if let Err(error) = SuiteCommand::new(program.clone(), arguments.clone()) {
      let slot = match error {
        TestSuiteError::PlaceholderInProgram => &mut problems.program,
        _ => &mut problems.arguments,
      };
      *slot = Some(suite_error(&error));
      return None;
    }
    let mut all = arguments;
    if let Some(filter) = filter {
      all.insert(self.filter_at.min(all.len()), filter);
    }
    SuiteCommand::new(program, all)
      .map_err(|error| problems.filter = Some(suite_error(&error)))
      .ok()
  }

  /// The filter's argument; `None` for a blank field.
  fn filter_argument(&self) -> Result<Option<CommandArg>, Message> {
    let filter = self.filter.trim();
    if filter.is_empty() {
      return Ok(None);
    }
    if !filter.contains(FILTER) {
      return Err(Message::new("services.suite.errors.filterPlaceholder").with("filter", FILTER.to_owned()));
    }
    CommandArg::new(filter)
      .map(Some)
      .map_err(|error| suite_error(&error.into()))
  }

  fn variables(&self) -> Result<SuiteVariables, Message> {
    let mut variables = BTreeMap::new();
    let lines = self
      .variables
      .iter()
      .filter(|line| !(line.name.trim().is_empty() && line.value.trim().is_empty()));
    for line in lines {
      let name = VariableName::new(line.name.trim()).map_err(|error| variable_error(&error))?;
      let value = if line.secret {
        SecretName::new(&line.value).map(SuiteVariable::Secret).map_err(|_| {
          Message::new("services.editor.errors.secretName")
            .with("name", name.as_str().to_owned())
            .with("max", SecretName::MAX_CHARS.to_string())
        })?
      } else {
        SuiteVariable::Plain(line.value.clone())
      };
      if variables.insert(name.clone(), value).is_some() {
        return Err(Message::new("tools.errors.duplicateVariable").with("name", name.as_str().to_owned()));
      }
    }
    SuiteVariables::new(variables).map_err(|error| suite_error(&error))
  }

  fn report(&self) -> Result<TestReport, Message> {
    let path = match self.read_from {
      ReadFrom::Printed => None,
      ReadFrom::File if self.report_path.trim().is_empty() => {
        return Err(Message::new("services.suite.errors.reportPathRequired"));
      }
      ReadFrom::File => Some(SuitePath::new(&self.report_path).map_err(|error| suite_error(&error))?),
    };
    TestReport::new(self.format, path).map_err(|error| suite_error(&error))
  }

  fn needs(&self) -> Result<SuiteNeeds, Message> {
    if self.needs == NeedsKind::Stack && self.stack.trim().is_empty() {
      return Err(Message::new("services.suite.errors.stackRequired"));
    }
    if self.needs == NeedsKind::Services && self.services.is_empty() {
      return Err(Message::new("services.suite.errors.servicesRequired"));
    }
    let needs = self
      .needed()
      .iter()
      .map(|need| Slug::new(need.trim()).map_err(|error| suite_error(&error.into())))
      .collect::<Result<Vec<_>, _>>()?;
    SuiteNeeds::new(needs).map_err(|error| suite_error(&error))
  }

  fn artifacts(&self) -> Result<Vec<ArtifactGlob>, Message> {
    self
      .artifacts
      .iter()
      .map(|glob| ArtifactGlob::new(glob).map_err(|error| suite_error(&error)))
      .collect()
  }

  fn timeout(&self, base: Option<&TestSuite>) -> Result<CommandTimeout, Message> {
    if let Some(base) = base.filter(|base| minutes(base.timeout()) == self.timeout.trim()) {
      return Ok(base.timeout());
    }
    self
      .timeout
      .trim()
      .parse::<u64>()
      .ok()
      .and_then(|minutes| minutes.checked_mul(60))
      .and_then(|secs| CommandTimeout::from_secs(secs).ok())
      .ok_or_else(|| {
        Message::new("services.suite.errors.timeout").with("max", (CommandTimeout::MAX_HOURS * 60).to_string())
      })
  }
}

/// A folder inside the worktree; `None` for the worktree itself.
fn optional_path(text: &str) -> Result<Option<SuitePath>, Message> {
  if text.trim().is_empty() {
    return Ok(None);
  }
  SuitePath::new(text).map(Some).map_err(|error| suite_error(&error))
}

/// The name of a new suite as typed.
pub fn suite_name(text: &str) -> Result<Slug, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Err(Message::new("services.suite.errors.nameRequired"));
  }
  Slug::new(text).map_err(|_| {
    Message::new("services.suite.errors.name")
      .with("name", text.to_owned())
      .with("max", Slug::MAX_CHARS.to_string())
  })
}

/// Why a part of a suite is not valid, in the words of the core's error. The words that name the placeholder take it
/// as `{{filter}}`'s value, so the locale does not read it as one of its own.
pub fn suite_error(error: &TestSuiteError) -> Message {
  let with = |key: &'static str, name: &str| Message::new(key).with("value", name.to_owned());
  let placeholder = |key: &'static str| Message::new(key).with("filter", FILTER.to_owned());
  match error {
    TestSuiteError::Command(CommandError::BlankProgram) => Message::new("tools.errors.programRequired"),
    TestSuiteError::Command(CommandError::DuplicateVariable(name)) => {
      Message::new("tools.errors.duplicateVariable").with("name", name.clone())
    }
    TestSuiteError::Command(CommandError::VariableValue(name)) => {
      Message::new("tools.errors.variableValue").with("name", name.clone())
    }
    TestSuiteError::Command(CommandError::TooLong(_)) => Message::new("tools.errors.tooLong"),
    TestSuiteError::Command(_) => Message::new("tools.errors.controlCharacter"),
    TestSuiteError::PlaceholderInProgram => placeholder("services.suite.errors.placeholderInProgram"),
    TestSuiteError::UnknownPlaceholder(value) => {
      with("services.suite.errors.unknownPlaceholder", value).with("filter", FILTER.to_owned())
    }
    TestSuiteError::UnclosedPlaceholder(value) => with("services.suite.errors.unclosedPlaceholder", value),
    TestSuiteError::RepeatedFilter => placeholder("services.suite.errors.repeatedFilter"),
    TestSuiteError::UnknownReportFormat(value) => with("services.suite.errors.unknownFormat", value),
    TestSuiteError::UnknownLayer(value) => with("services.suite.errors.unknownLayer", value),
    TestSuiteError::ReportPathMissing(_) => Message::new("services.suite.errors.reportPathRequired"),
    TestSuiteError::ReportPrinted(_) => Message::new("services.suite.errors.reportPrinted"),
    TestSuiteError::BlankPath => Message::new("services.suite.errors.blankPath"),
    TestSuiteError::AbsolutePath(value) => with("services.suite.errors.absolutePath", value),
    TestSuiteError::PathLeavesWorktree(value) => with("services.suite.errors.leavesWorktree", value),
    TestSuiteError::SecretName(_) => Message::new("services.suite.errors.secretName"),
    TestSuiteError::Need(_) => Message::new("services.suite.errors.need"),
    TestSuiteError::RepeatedNeed(value) => with("services.suite.errors.repeatedNeed", value),
  }
}
