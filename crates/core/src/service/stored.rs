//! The JSON shape of a service's and a stack's body in the project database. Reading it runs every constructor
//! again, so a stored body that breaks a rule is refused instead of loaded. Fields a definition may leave out
//! (arguments, variables, needs, seed, run data, UI driver, UI start, owned paths) read as their defaults.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  Capture, CapturePath, HttpPath, LogPattern, OwnedPath, Readiness, ReadyCheck, ReadyTimeout, RelativeFolder, RunData,
  RunFolder, Seed, SeedHeader, SeedRequest, SeedRequests, Service, ServiceError, ServiceName, ServiceValue,
  ServiceVariables, Stack, StackName, UiDriver, UiStart,
};
use crate::{
  resource::{CommandArg, CommandError, CommandProgram, FieldName, FieldNameError, HttpMethod, VariableName},
  secret::{SecretName, SecretNameError},
};

/// Why a stored body is not a valid service or stack.
#[derive(Debug, Error)]
enum StoredError {
  #[error(transparent)]
  Service(#[from] ServiceError),
  #[error(transparent)]
  Command(#[from] CommandError),
  #[error(transparent)]
  Secret(#[from] SecretNameError),
  #[error(transparent)]
  FieldName(#[from] FieldNameError),
}

#[derive(Serialize, Deserialize)]
struct ServiceFields {
  name: String,
  program: String,
  #[serde(default)]
  arguments: Vec<String>,
  /// Empty for the worktree itself.
  #[serde(default)]
  working_folder: String,
  #[serde(default)]
  variables: BTreeMap<String, ValueFields>,
  #[serde(default)]
  needs: Vec<String>,
  readiness: ReadinessFields,
  #[serde(default)]
  seed: SeedFields,
  #[serde(default)]
  run_data: RunDataFields,
  #[serde(default)]
  ui_driver: UiDriverFields,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  ui_start: Option<String>,
  #[serde(default)]
  owned_paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ValueFields {
  Plain(String),
  Secret(String),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReadinessFields {
  PortOpen { timeout_secs: u64 },
  Http { path: String, timeout_secs: u64 },
  LogLine { pattern: String, timeout_secs: u64 },
}

#[derive(Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SeedFields {
  #[default]
  None,
  Requests {
    requests: Vec<RequestFields>,
  },
  Command {
    program: String,
    #[serde(default)]
    arguments: Vec<String>,
  },
}

#[derive(Serialize, Deserialize)]
struct RequestFields {
  method: HttpMethod,
  url: String,
  #[serde(default)]
  body: Option<String>,
  #[serde(default)]
  headers: Vec<HeaderFields>,
  #[serde(default)]
  captures: Vec<CaptureFields>,
}

#[derive(Serialize, Deserialize)]
struct HeaderFields {
  name: String,
  value: ValueFields,
}

#[derive(Serialize, Deserialize)]
struct CaptureFields {
  name: String,
  path: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RunDataFields {
  #[default]
  None,
  DatabaseContainer,
  RunFolder {
    folder: String,
  },
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UiDriverFields {
  #[default]
  None,
  Native,
  Mcp,
}

#[derive(Serialize, Deserialize)]
struct StackFields {
  name: String,
  services: Vec<String>,
}

impl Serialize for Service {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    ServiceFields::from(self).serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for Service {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = ServiceFields::deserialize(deserializer)?;
    Self::try_from(fields).map_err(serde::de::Error::custom)
  }
}

impl Serialize for Stack {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    StackFields {
      name: self.name().as_str().to_owned(),
      services: self.services().iter().map(|name| name.as_str().to_owned()).collect(),
    }
    .serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for Stack {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = StackFields::deserialize(deserializer)?;
    let stack = || {
      let services = fields
        .services
        .iter()
        .map(|name| ServiceName::new(name))
        .collect::<Result<_, _>>()?;
      Self::new(StackName::new(&fields.name)?, services)
    };
    stack().map_err(serde::de::Error::custom)
  }
}

impl From<&Service> for ServiceFields {
  fn from(service: &Service) -> Self {
    Self {
      name: service.name().as_str().to_owned(),
      program: service.program().as_str().to_owned(),
      arguments: texts(service.arguments()),
      working_folder: service.working_folder().as_str().to_owned(),
      variables: service
        .variables()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), ValueFields::from(value)))
        .collect(),
      needs: service.needs().iter().map(|need| need.as_str().to_owned()).collect(),
      readiness: ReadinessFields::from(service.readiness()),
      seed: SeedFields::from(service.seed()),
      run_data: match service.run_data() {
        RunData::None => RunDataFields::None,
        RunData::DatabaseContainer => RunDataFields::DatabaseContainer,
        RunData::RunFolder(folder) => RunDataFields::RunFolder {
          folder: folder.as_str().to_owned(),
        },
      },
      ui_driver: match service.ui_driver() {
        UiDriver::None => UiDriverFields::None,
        UiDriver::Native => UiDriverFields::Native,
        UiDriver::Mcp => UiDriverFields::Mcp,
      },
      ui_start: service.ui_start().map(|start| start.as_str().to_owned()),
      owned_paths: service
        .owned_paths()
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect(),
    }
  }
}

impl TryFrom<ServiceFields> for Service {
  type Error = StoredError;

  fn try_from(fields: ServiceFields) -> Result<Self, StoredError> {
    let variables = fields
      .variables
      .into_iter()
      .map(|(name, value)| Ok((VariableName::new(&name)?, value.try_into()?)))
      .collect::<Result<_, StoredError>>()?;
    let needs = fields
      .needs
      .iter()
      .map(|need| ServiceName::new(need))
      .collect::<Result<_, _>>()?;
    let run_data = match fields.run_data {
      RunDataFields::None => RunData::None,
      RunDataFields::DatabaseContainer => RunData::DatabaseContainer,
      RunDataFields::RunFolder { folder } => RunData::RunFolder(RunFolder::new(&folder)?),
    };
    let ui_driver = match fields.ui_driver {
      UiDriverFields::None => UiDriver::None,
      UiDriverFields::Native => UiDriver::Native,
      UiDriverFields::Mcp => UiDriver::Mcp,
    };
    let owned_paths = fields
      .owned_paths
      .iter()
      .map(|path| OwnedPath::new(path))
      .collect::<Result<_, _>>()?;
    let service = Self::new(
      ServiceName::new(&fields.name)?,
      CommandProgram::new(&fields.program)?,
      fields.readiness.try_into()?,
    )
    .with_arguments(arguments(&fields.arguments)?)
    .with_working_folder(RelativeFolder::new(&fields.working_folder)?)
    .with_variables(ServiceVariables::new(variables)?)
    .with_needs(needs)?
    .with_seed(fields.seed.try_into()?)
    .with_run_data(run_data)
    .with_ui_driver(ui_driver)
    .with_owned_paths(owned_paths)?;
    Ok(match fields.ui_start {
      Some(start) => service.with_ui_start(UiStart::new(&start)?),
      None => service,
    })
  }
}

impl From<&ServiceValue> for ValueFields {
  fn from(value: &ServiceValue) -> Self {
    match value {
      ServiceValue::Plain(text) => Self::Plain(text.clone()),
      ServiceValue::Secret(name) => Self::Secret(name.as_str().to_owned()),
    }
  }
}

impl TryFrom<ValueFields> for ServiceValue {
  type Error = StoredError;

  fn try_from(value: ValueFields) -> Result<Self, StoredError> {
    Ok(match value {
      ValueFields::Plain(text) => Self::Plain(text),
      ValueFields::Secret(name) => Self::Secret(SecretName::new(&name)?),
    })
  }
}

impl From<&Readiness> for ReadinessFields {
  fn from(readiness: &Readiness) -> Self {
    let timeout_secs = readiness.timeout().secs();
    match readiness.check() {
      ReadyCheck::PortOpen => Self::PortOpen { timeout_secs },
      ReadyCheck::Http(path) => Self::Http {
        path: path.as_str().to_owned(),
        timeout_secs,
      },
      ReadyCheck::LogLine(pattern) => Self::LogLine {
        pattern: pattern.as_str().to_owned(),
        timeout_secs,
      },
    }
  }
}

impl TryFrom<ReadinessFields> for Readiness {
  type Error = StoredError;

  fn try_from(fields: ReadinessFields) -> Result<Self, StoredError> {
    let (check, timeout_secs) = match fields {
      ReadinessFields::PortOpen { timeout_secs } => (ReadyCheck::PortOpen, timeout_secs),
      ReadinessFields::Http { path, timeout_secs } => (ReadyCheck::Http(HttpPath::new(&path)?), timeout_secs),
      ReadinessFields::LogLine { pattern, timeout_secs } => {
        (ReadyCheck::LogLine(LogPattern::new(&pattern)?), timeout_secs)
      }
    };
    Ok(Self::new(check, ReadyTimeout::from_secs(timeout_secs)?))
  }
}

impl From<&Seed> for SeedFields {
  fn from(seed: &Seed) -> Self {
    match seed {
      Seed::None => Self::None,
      Seed::Requests(requests) => Self::Requests {
        requests: requests.as_slice().iter().map(RequestFields::from).collect(),
      },
      Seed::Command { program, arguments } => Self::Command {
        program: program.as_str().to_owned(),
        arguments: texts(arguments),
      },
    }
  }
}

impl TryFrom<SeedFields> for Seed {
  type Error = StoredError;

  fn try_from(fields: SeedFields) -> Result<Self, StoredError> {
    Ok(match fields {
      SeedFields::None => Self::None,
      SeedFields::Requests { requests } => {
        let requests = requests
          .into_iter()
          .map(SeedRequest::try_from)
          .collect::<Result<_, _>>()?;
        Self::Requests(SeedRequests::new(requests)?)
      }
      SeedFields::Command {
        program,
        arguments: args,
      } => Self::Command {
        program: CommandProgram::new(&program)?,
        arguments: arguments(&args)?,
      },
    })
  }
}

impl From<&SeedRequest> for RequestFields {
  fn from(request: &SeedRequest) -> Self {
    Self {
      method: request.method(),
      url: request.url().to_owned(),
      body: request.body().map(str::to_owned),
      headers: request
        .headers()
        .iter()
        .map(|header| HeaderFields {
          name: header.name().to_owned(),
          value: ValueFields::from(header.value()),
        })
        .collect(),
      captures: request
        .captures()
        .iter()
        .map(|capture| CaptureFields {
          name: capture.name().as_str().to_owned(),
          path: capture.path().as_str().to_owned(),
        })
        .collect(),
    }
  }
}

impl TryFrom<RequestFields> for SeedRequest {
  type Error = StoredError;

  fn try_from(fields: RequestFields) -> Result<Self, StoredError> {
    let headers = fields
      .headers
      .into_iter()
      .map(|header| Ok(SeedHeader::new(&header.name, header.value.try_into()?)?))
      .collect::<Result<_, StoredError>>()?;
    let captures = fields
      .captures
      .iter()
      .map(|capture| {
        Ok(Capture::new(
          FieldName::new(&capture.name)?,
          CapturePath::new(&capture.path)?,
        ))
      })
      .collect::<Result<_, StoredError>>()?;
    Ok(Self::new(fields.method, &fields.url, fields.body, headers, captures)?)
  }
}

fn texts(arguments: &[CommandArg]) -> Vec<String> {
  arguments.iter().map(|argument| argument.as_str().to_owned()).collect()
}

fn arguments(texts: &[String]) -> Result<Vec<CommandArg>, CommandError> {
  texts.iter().map(|text| CommandArg::new(text)).collect()
}
