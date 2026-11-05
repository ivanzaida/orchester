//! The service editor's fields as a person leaves them (`02 · Service editor` `TGRTt`): the name of a new service, the
//! program, its arguments one per line, the working folder, the variables as rows of a name and a plain value or a
//! project secret's name, and the services it needs in order. Every value is checked by the core type it becomes;
//! what the page does not edit yet (readiness, seed, per-run data, UI driver, owned paths) is kept as stored.

use std::{collections::BTreeMap, fmt};

use orchester_core::{
  resource::{CommandArg, CommandProgram, VariableName},
  secret::{SecretName, SecretNameError},
  service::{
    Readiness, ReadyCheck, ReadyTimeout, RelativeFolder, Service, ServiceError, ServiceName, ServiceValue,
    ServiceVariables,
  },
};

use crate::{
  messages::Message,
  tools::form::{parse_arguments, program_message, variable_error},
};

/// How long a new service's start waits for its port, until the Readiness section edits it: a new draft starts with
/// port readiness, as an imported one does (`05 · Import draft` `DFWRJ`).
const NEW_READY_SECS: u64 = 60;

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
    }
  }

  /// The service these fields describe. `base` is the stored service, whose name and other parts it keeps; without
  /// one, a new service under the typed name with port readiness and nothing else.
  pub fn service(&self, base: Option<&Service>) -> Result<Service, Box<ServiceProblems>> {
    let name = match base {
      Some(base) => Ok(base.name().clone()),
      None => service_name(&self.name),
    };
    let program = CommandProgram::new(&self.program).map_err(|error| program_message(&error));
    let arguments = parse_arguments(&self.arguments);
    let folder = RelativeFolder::new(&self.working_folder).map_err(|error| service_error(&error));
    let variables = self.variables();
    let needs = self.needs();
    let (Ok(name), Ok(program), Ok(arguments), Ok(folder), Ok(variables), Ok(needs)) =
      (&name, &program, &arguments, &folder, &variables, &needs)
    else {
      return Err(Box::new(ServiceProblems {
        name: name.err(),
        program: program.err(),
        arguments: arguments.err(),
        working_folder: folder.err(),
        variables: variables.err(),
        needs: needs.err(),
      }));
    };
    let readiness = base.map_or_else(new_readiness, |base| base.readiness().clone());
    let service = Service::new(name.clone(), program.clone(), readiness)
      .with_arguments(arguments.clone())
      .with_working_folder(folder.clone())
      .with_variables(variables.clone())
      .with_needs(needs.clone())
      .map_err(|error| {
        Box::new(ServiceProblems {
          needs: Some(service_error(&error)),
          ..ServiceProblems::default()
        })
      })?;
    Ok(match base {
      Some(base) => kept(service, base),
      None => service,
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

/// A new service's port readiness.
fn new_readiness() -> Readiness {
  Readiness::new(
    ReadyCheck::PortOpen,
    ReadyTimeout::from_secs(NEW_READY_SECS).expect("a minute is a readiness timeout"),
  )
}

/// `service` with what the page does not edit taken from `base`.
fn kept(service: Service, base: &Service) -> Service {
  let service = service
    .with_seed(base.seed().clone())
    .with_run_data(base.run_data().clone())
    .with_ui_driver(base.ui_driver())
    .with_owned_paths(base.owned_paths().to_vec())
    .expect("a stored service owns each path once");
  match base.ui_start() {
    Some(start) => service.with_ui_start(start.clone()),
    None => service,
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
