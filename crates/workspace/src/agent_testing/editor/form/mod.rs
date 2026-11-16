//! The service editor's fields as a person leaves them (`02 · Service editor` `TGRTt`): the name of a new service, the
//! program, its arguments one per line, the working folder, the variables as rows of a name and a plain value or a
//! project secret's name, the services it needs in order, and the sections after them ([`sections`]): readiness,
//! seed, per-run data, UI driver and owned paths. Every value is checked by the core type it becomes.

mod sections;

use std::{collections::BTreeMap, fmt};

use orchester_core::{
  resource::{CommandArg, CommandProgram, VariableName},
  secret::{SecretName, SecretNameError},
  service::{
    OwnedPath, Readiness, RelativeFolder, RunData, Seed, Service, ServiceError, ServiceName, ServiceValue,
    ServiceVariables, UiStart,
  },
};

pub use self::sections::{
  CaptureLine, DataForm, DataKind, METHODS, ReadinessForm, ReadinessProblems, ReadyKind, RequestLine, SeedForm,
  SeedKind, SeedProblem, UiForm, method_name,
};
use crate::{
  messages::Message,
  tools::form::{parse_arguments, program_message, variable_error},
};

/// One variable as its row holds it: a plain value as written, or the name of a project secret, whose value the
/// runner reads when it starts the service and which no screen shows.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct VariableLine {
  pub name: String,
  pub value: String,
  pub secret: bool,
}

/// A plain value may hold a token typed by mistake, so `Debug` shows only how long it is.
impl fmt::Debug for VariableLine {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("VariableLine")
      .field("name", &self.name)
      .field("value", &format_args!("<{} characters>", self.value.chars().count()))
      .field("secret", &self.secret)
      .finish()
  }
}

impl VariableLine {
  /// Neither a name nor a value: the row is left out.
  fn is_blank(&self) -> bool {
    self.name.trim().is_empty() && self.value.trim().is_empty()
  }
}

/// The fields as the person left them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceForm {
  /// Only a new service's; a service keeps the name it was made with.
  pub name: String,
  pub program: String,
  /// One argument per line; empty lines are left out.
  pub arguments: String,
  pub working_folder: String,
  pub variables: Vec<VariableLine>,
  /// The services it needs, in the order they start.
  pub needs: Vec<String>,
  pub readiness: ReadinessForm,
  pub seed: SeedForm,
  pub run_data: DataForm,
  pub ui: UiForm,
  /// The repository path patterns it owns, in the order they were added.
  pub owned_paths: Vec<String>,
}

/// Why the fields do not make a service yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceProblems {
  pub name: Option<Message>,
  pub program: Option<Message>,
  pub arguments: Option<Message>,
  pub working_folder: Option<Message>,
  pub variables: Option<Message>,
  pub needs: Option<Message>,
  /// The chosen readiness check's path or pattern.
  pub readiness: Option<Message>,
  pub timeout: Option<Message>,
  pub seed: Option<SeedProblem>,
  pub run_data: Option<Message>,
  pub ui_start: Option<Message>,
  pub owned_paths: Option<Message>,
}

/// What each field makes, before they make one service.
struct Parts {
  name: ServiceName,
  program: CommandProgram,
  arguments: Vec<CommandArg>,
  folder: RelativeFolder,
  variables: ServiceVariables,
  needs: Vec<ServiceName>,
  readiness: Readiness,
  seed: Seed,
  run_data: RunData,
  ui_start: Option<UiStart>,
  owned_paths: Vec<OwnedPath>,
}

/// `result`'s value, or `None` with its error kept in `slot`.
fn kept<T>(result: Result<T, Message>, slot: &mut Option<Message>) -> Option<T> {
  result.map_err(|error| *slot = Some(error)).ok()
}

impl ServiceForm {
  /// The fields of `service` as stored.
  pub fn of(service: &Service) -> Self {
    Self {
      name: service.name().as_str().to_owned(),
      program: service.program().as_str().to_owned(),
      arguments: service
        .arguments()
        .iter()
        .map(CommandArg::as_str)
        .collect::<Vec<_>>()
        .join("\n"),
      working_folder: service.working_folder().as_str().to_owned(),
      variables: service
        .variables()
        .iter()
        .map(|(name, value)| match value {
          ServiceValue::Plain(text) => VariableLine {
            name: name.as_str().to_owned(),
            value: text.clone(),
            secret: false,
          },
          ServiceValue::Secret(secret) => VariableLine {
            name: name.as_str().to_owned(),
            value: secret.as_str().to_owned(),
            secret: true,
          },
        })
        .collect(),
      needs: service.needs().iter().map(|need| need.as_str().to_owned()).collect(),
      readiness: ReadinessForm::of(service.readiness()),
      seed: SeedForm::of(service.seed()),
      run_data: DataForm::of(service.run_data()),
      ui: UiForm::of(service.ui_driver(), service.ui_start()),
      owned_paths: service
        .owned_paths()
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect(),
    }
  }

  /// The service these fields describe. `base` is the stored service, whose name it keeps; without one, a new service
  /// under the typed name.
  pub fn service(&self, base: Option<&Service>) -> Result<Service, Box<ServiceProblems>> {
    let parts = self.parts(base)?;
    let service = Service::new(parts.name, parts.program, parts.readiness)
      .with_arguments(parts.arguments)
      .with_working_folder(parts.folder)
      .with_variables(parts.variables)
      .with_seed(parts.seed)
      .with_run_data(parts.run_data)
      .with_ui_driver(self.ui.driver);
    let service = match parts.ui_start {
      Some(start) => service.with_ui_start(start),
      None => service,
    };
    service
      .with_needs(parts.needs)
      .map_err(|error| {
        Box::new(ServiceProblems {
          needs: Some(service_error(&error)),
          ..ServiceProblems::default()
        })
      })?
      .with_owned_paths(parts.owned_paths)
      .map_err(|error| {
        Box::new(ServiceProblems {
          owned_paths: Some(sections::section_error(&error)),
          ..ServiceProblems::default()
        })
      })
  }

  /// What each field makes, or every field's problem.
  fn parts(&self, base: Option<&Service>) -> Result<Parts, Box<ServiceProblems>> {
    let mut problems = ServiceProblems::default();
    let name = base.map_or_else(|| service_name(&self.name), |base| Ok(base.name().clone()));
    let name = kept(name, &mut problems.name);
    let program = CommandProgram::new(&self.program).map_err(|error| program_message(&error));
    let program = kept(program, &mut problems.program);
    let arguments = kept(parse_arguments(&self.arguments), &mut problems.arguments);
    let folder = RelativeFolder::new(&self.working_folder).map_err(|error| service_error(&error));
    let folder = kept(folder, &mut problems.working_folder);
    let variables = kept(self.variables(), &mut problems.variables);
    let needs = kept(self.needs(), &mut problems.needs);
    let readiness = self
      .readiness
      .readiness()
      .map_err(|wrong| {
        problems.readiness = wrong.check;
        problems.timeout = wrong.timeout;
      })
      .ok();
    let seed = self.seed.seed().map_err(|wrong| problems.seed = Some(wrong)).ok();
    let run_data = kept(self.run_data.run_data(), &mut problems.run_data);
    let ui_start = kept(self.ui.start(), &mut problems.ui_start);
    let owned_paths = kept(sections::owned_paths(&self.owned_paths), &mut problems.owned_paths);
    let (
      Some(name),
      Some(program),
      Some(arguments),
      Some(folder),
      Some(variables),
      Some(needs),
      Some(readiness),
      Some(seed),
      Some(run_data),
      Some(ui_start),
      Some(owned_paths),
    ) = (
      name,
      program,
      arguments,
      folder,
      variables,
      needs,
      readiness,
      seed,
      run_data,
      ui_start,
      owned_paths,
    )
    else {
      return Err(Box::new(problems));
    };
    Ok(Parts {
      name,
      program,
      arguments,
      folder,
      variables,
      needs,
      readiness,
      seed,
      run_data,
      ui_start,
      owned_paths,
    })
  }

  fn variables(&self) -> Result<ServiceVariables, Message> {
    let mut variables = BTreeMap::new();
    for line in self.variables.iter().filter(|line| !line.is_blank()) {
      let name = VariableName::new(line.name.trim()).map_err(|error| variable_error(&error))?;
      let value = if line.secret {
        let secret = SecretName::new(&line.value).map_err(|error| secret_error(&error, name.as_str()))?;
        ServiceValue::Secret(secret)
      } else {
        ServiceValue::Plain(line.value.clone())
      };
      if variables.insert(name.clone(), value).is_some() {
        return Err(Message::new("tools.errors.duplicateVariable").with("name", name.as_str().to_owned()));
      }
    }
    ServiceVariables::new(variables).map_err(|error| service_error(&error))
  }

  fn needs(&self) -> Result<Vec<ServiceName>, Message> {
    self
      .needs
      .iter()
      .map(|need| ServiceName::new(need).map_err(|error| service_error(&error)))
      .collect()
  }
}

/// The name of a new service as typed.
fn service_name(text: &str) -> Result<ServiceName, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Err(Message::new("services.editor.errors.nameRequired"));
  }
  ServiceName::new(text).map_err(|error| service_error(&error))
}

/// Why a part of a service is not valid, for the parts the page edits.
pub fn service_error(error: &ServiceError) -> Message {
  match error {
    ServiceError::Name(name) => Message::new("services.editor.errors.name")
      .with("name", name.clone())
      .with("max", ServiceName::MAX_CHARS.to_string()),
    ServiceError::OutsideWorktree(folder) => {
      Message::new("services.editor.errors.outsideWorktree").with("folder", folder.clone())
    }
    ServiceError::DuplicateVariable(name) => Message::new("tools.errors.duplicateVariable").with("name", name.clone()),
    ServiceError::VariableValue(name) => Message::new("tools.errors.variableValue").with("name", name.clone()),
    ServiceError::NeedsItself(name) => {
      Message::new("services.editor.errors.needsItself").with("name", name.to_string())
    }
    ServiceError::DuplicateNeed(name) => {
      Message::new("services.editor.errors.duplicateNeed").with("name", name.to_string())
    }
    _ => Message::new("services.editor.errors.invalid"),
  }
}

fn secret_error(error: &SecretNameError, variable: &str) -> Message {
  let key = match error {
    SecretNameError::Blank => "services.editor.errors.secretRequired",
    SecretNameError::TooLong { .. } | SecretNameError::Invalid => "services.editor.errors.secretName",
  };
  Message::new(key)
    .with("name", variable.to_owned())
    .with("max", SecretName::MAX_CHARS.to_string())
}
