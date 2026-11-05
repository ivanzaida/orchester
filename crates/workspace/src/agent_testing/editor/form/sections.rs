//! The parts of the service editor after Program and Needs (`02 · Service editor` `TGRTt`): when a started service is
//! ready and how long its start waits, what runs once it is ready (HTTP requests with captures, a command, or
//! nothing), the data a run gets of its own, how agents reach its UI and where `ui_open` starts, and the repository
//! paths it owns. Each holds what the person typed, and is checked by the core types it becomes.

use std::fmt;

use orchester_core::{
  resource::{CommandProgram, FieldName, HttpMethod},
  service::{
    Capture, CapturePath, HttpPath, LogPattern, OwnedPath, Readiness, ReadyCheck, ReadyTimeout, RunData, RunFolder,
    Seed, SeedHeader, SeedRequest, SeedRequests, ServiceError, UiDriver, UiStart,
  },
};

use super::service_error;
use crate::{
  messages::Message,
  tools::form::{parse_arguments, program_message},
};

/// How long a new service's start waits for it, in seconds, as an imported draft does (`05 · Import draft`
/// `DFWRJ`).
const NEW_READY_SECS: &str = "60";

/// The HTTP methods a seed request is sent with, by the names HTTP gives them.
pub const METHODS: [(HttpMethod, &str); 5] = [
  (HttpMethod::Get, "GET"),
  (HttpMethod::Post, "POST"),
  (HttpMethod::Put, "PUT"),
  (HttpMethod::Patch, "PATCH"),
  (HttpMethod::Delete, "DELETE"),
];

/// The name HTTP gives `method`.
pub fn method_name(method: HttpMethod) -> &'static str {
  METHODS
    .iter()
    .find(|(each, _)| *each == method)
    .map_or("GET", |(_, name)| name)
}

/// Which check tells that a started service is ready.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadyKind {
  #[default]
  Port,
  Http,
  Log,
}

/// The Readiness section as typed: the check chosen, the path and the pattern each kept while another is chosen, and
/// the timeout in seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessForm {
  pub kind: ReadyKind,
  pub http_path: String,
  pub log_pattern: String,
  pub timeout: String,
}

impl Default for ReadinessForm {
  fn default() -> Self {
    Self {
      kind: ReadyKind::Port,
      http_path: String::new(),
      log_pattern: String::new(),
      timeout: NEW_READY_SECS.to_owned(),
    }
  }
}

/// Why the Readiness section makes no readiness: the chosen check's value, the timeout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadinessProblems {
  pub check: Option<Message>,
  pub timeout: Option<Message>,
}

impl ReadinessForm {
  pub fn of(readiness: &Readiness) -> Self {
    let (kind, http_path, log_pattern) = match readiness.check() {
      ReadyCheck::PortOpen => (ReadyKind::Port, String::new(), String::new()),
      ReadyCheck::Http(path) => (ReadyKind::Http, path.as_str().to_owned(), String::new()),
      ReadyCheck::LogLine(pattern) => (ReadyKind::Log, String::new(), pattern.as_str().to_owned()),
    };
    Self {
      kind,
      http_path,
      log_pattern,
      timeout: readiness.timeout().secs().to_string(),
    }
  }

  pub(super) fn readiness(&self) -> Result<Readiness, ReadinessProblems> {
    let check = match self.kind {
      ReadyKind::Port => Ok(ReadyCheck::PortOpen),
      ReadyKind::Http => HttpPath::new(self.http_path.trim())
        .map(ReadyCheck::Http)
        .map_err(|error| section_error(&error)),
      ReadyKind::Log => LogPattern::new(&self.log_pattern)
        .map(ReadyCheck::LogLine)
        .map_err(|error| section_error(&error)),
    };
    let timeout = self
      .timeout
      .trim()
      .parse::<u64>()
      .ok()
      .and_then(|secs| ReadyTimeout::from_secs(secs).ok())
      .ok_or_else(|| section_error(&ServiceError::ReadyTimeout));
    match (check, timeout) {
      (Ok(check), Ok(timeout)) => Ok(Readiness::new(check, timeout)),
      (check, timeout) => Err(ReadinessProblems {
        check: check.err(),
        timeout: timeout.err(),
      }),
    }
  }
}

/// What runs once a started service is ready.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeedKind {
  Requests,
  Command,
  #[default]
  None,
}

/// A value a request's answer gives, as typed: its name and where in the answer it is read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureLine {
  pub name: String,
  pub path: String,
}

/// One seed request as typed. Its headers are not edited here and are kept as stored.
#[derive(Clone, PartialEq, Eq)]
pub struct RequestLine {
  pub method: HttpMethod,
  pub url: String,
  /// Empty sends none.
  pub body: String,
  pub captures: Vec<CaptureLine>,
  pub headers: Vec<SeedHeader>,
}

/// A body may carry a password for the seed's own account, so `Debug` shows only how long it is.
impl fmt::Debug for RequestLine {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RequestLine")
      .field("method", &self.method)
      .field("url", &self.url)
      .field("body", &format_args!("<{} characters>", self.body.chars().count()))
      .field("captures", &self.captures)
      .field("headers", &self.headers)
      .finish()
  }
}

/// A new request: a `POST`, as a seed mostly makes things, with nothing typed yet.
impl Default for RequestLine {
  fn default() -> Self {
    Self {
      method: HttpMethod::Post,
      url: String::new(),
      body: String::new(),
      captures: Vec::new(),
      headers: Vec::new(),
    }
  }
}

impl RequestLine {
  fn of(request: &SeedRequest) -> Self {
    Self {
      method: request.method(),
      url: request.url().to_owned(),
      body: request.body().unwrap_or_default().to_owned(),
      captures: request
        .captures()
        .iter()
        .map(|capture| CaptureLine {
          name: capture.name().as_str().to_owned(),
          path: capture.path().as_str().to_owned(),
        })
        .collect(),
      headers: request.headers().to_vec(),
    }
  }

  fn request(&self) -> Result<SeedRequest, Message> {
    let captures = self
      .captures
      .iter()
      .filter(|capture| !(capture.name.trim().is_empty() && capture.path.trim().is_empty()))
      .map(|capture| {
        let name = FieldName::new(capture.name.trim()).map_err(|_| {
          Message::new("services.editor.errors.captureName").with("name", capture.name.trim().to_owned())
        })?;
        let path = CapturePath::new(capture.path.trim()).map_err(|error| section_error(&error))?;
        Ok(Capture::new(name, path))
      })
      .collect::<Result<Vec<_>, Message>>()?;
    let body = (!self.body.trim().is_empty()).then(|| self.body.clone());
    SeedRequest::new(self.method, self.url.trim(), body, self.headers.clone(), captures)
      .map_err(|error| section_error(&error))
  }
}

/// The Seed section as typed: the kind chosen, and the requests and the command each kept while another is chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedForm {
  pub kind: SeedKind,
  pub requests: Vec<RequestLine>,
  pub program: String,
  /// One argument per line.
  pub arguments: String,
}

/// Why the Seed section makes no seed, with the request it is about, counted from 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedProblem {
  pub request: Option<usize>,
  pub message: Message,
}

impl SeedForm {
  pub fn of(seed: &Seed) -> Self {
    match seed {
      Seed::None => Self::default(),
      Seed::Requests(requests) => Self {
        kind: SeedKind::Requests,
        requests: requests.as_slice().iter().map(RequestLine::of).collect(),
        ..Self::default()
      },
      Seed::Command { program, arguments } => Self {
        kind: SeedKind::Command,
        program: program.as_str().to_owned(),
        arguments: arguments
          .iter()
          .map(|argument| argument.as_str().to_owned())
          .collect::<Vec<_>>()
          .join("\n"),
        ..Self::default()
      },
    }
  }

  pub(super) fn seed(&self) -> Result<Seed, SeedProblem> {
    let whole = |message| SeedProblem { request: None, message };
    match self.kind {
      SeedKind::None => Ok(Seed::None),
      SeedKind::Command => {
        let program = CommandProgram::new(&self.program).map_err(|error| whole(program_message(&error)))?;
        let arguments = parse_arguments(&self.arguments).map_err(whole)?;
        Ok(Seed::Command { program, arguments })
      }
      SeedKind::Requests => {
        let requests = self
          .requests
          .iter()
          .enumerate()
          .map(|(at, line)| {
            line.request().map_err(|message| SeedProblem {
              request: Some(at),
              message,
            })
          })
          .collect::<Result<Vec<_>, _>>()?;
        SeedRequests::new(requests)
          .map(Seed::Requests)
          .map_err(|error| whole(section_error(&error)))
      }
    }
  }
}

/// The data a run gets of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataKind {
  #[default]
  None,
  Container,
  Folder,
}

/// The Per-run data section as typed: the kind chosen and the run folder, kept while another is chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataForm {
  pub kind: DataKind,
  pub folder: String,
}

impl DataForm {
  pub fn of(data: &RunData) -> Self {
    match data {
      RunData::None => Self::default(),
      RunData::DatabaseContainer => Self {
        kind: DataKind::Container,
        folder: String::new(),
      },
      RunData::RunFolder(folder) => Self {
        kind: DataKind::Folder,
        folder: folder.as_str().to_owned(),
      },
    }
  }

  pub(super) fn run_data(&self) -> Result<RunData, Message> {
    match self.kind {
      DataKind::None => Ok(RunData::None),
      DataKind::Container => Ok(RunData::DatabaseContainer),
      DataKind::Folder => RunFolder::new(&self.folder)
        .map(RunData::RunFolder)
        .map_err(|error| section_error(&error)),
    }
  }
}

/// The UI driver section as typed: the driver and where `ui_open` starts; empty leaves that to the driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiForm {
  pub driver: UiDriver,
  pub start: String,
}

impl Default for UiForm {
  fn default() -> Self {
    Self {
      driver: UiDriver::None,
      start: String::new(),
    }
  }
}

impl UiForm {
  pub fn of(driver: UiDriver, start: Option<&UiStart>) -> Self {
    Self {
      driver,
      start: start.map(|start| start.as_str().to_owned()).unwrap_or_default(),
    }
  }

  pub(super) fn start(&self) -> Result<Option<UiStart>, Message> {
    let start = self.start.trim();
    if start.is_empty() {
      return Ok(None);
    }
    UiStart::new(start).map(Some).map_err(|error| section_error(&error))
  }
}

/// The owned paths as typed, each a repository path pattern.
pub(super) fn owned_paths(paths: &[String]) -> Result<Vec<OwnedPath>, Message> {
  paths
    .iter()
    .map(|path| OwnedPath::new(path).map_err(|error| section_error(&error)))
    .collect()
}

/// Why a part of these sections is not valid. The URL and the place `ui_open` starts are not repeated: their query
/// strings may hold a token.
pub(super) fn section_error(error: &ServiceError) -> Message {
  match error {
    ServiceError::HttpPath(path) => Message::new("services.editor.errors.httpPath").with("path", path.clone()),
    ServiceError::LogPatternTooLong => {
      Message::new("services.editor.errors.logPatternTooLong").with("max", LogPattern::MAX_CHARS.to_string())
    }
    ServiceError::LogPattern { pattern, .. } if pattern.is_empty() => {
      Message::new("services.editor.errors.logPatternRequired")
    }
    ServiceError::LogPattern { pattern, .. } => {
      Message::new("services.editor.errors.logPattern").with("pattern", pattern.clone())
    }
    ServiceError::ReadyTimeout => {
      Message::new("services.editor.errors.readyTimeout").with("max", ReadyTimeout::MAX_SECS.to_string())
    }
    ServiceError::SeedUrl => Message::new("services.editor.errors.seedUrl"),
    ServiceError::CapturePath(path) => Message::new("services.editor.errors.capturePath").with("path", path.clone()),
    ServiceError::DuplicateCapture(name) => {
      Message::new("services.editor.errors.duplicateCapture").with("name", name.as_str().to_owned())
    }
    ServiceError::NoSeedRequests => Message::new("services.editor.errors.noSeedRequests"),
    ServiceError::DuplicateHeader(name) => {
      Message::new("services.editor.errors.duplicateHeader").with("name", name.clone())
    }
    ServiceError::RunFolderIsRoot => Message::new("services.editor.errors.runFolderIsRoot"),
    ServiceError::OutsideWorktree(folder) => {
      Message::new("services.editor.errors.runFolder").with("folder", folder.clone())
    }
    ServiceError::UiStart => Message::new("services.editor.errors.uiStart"),
    ServiceError::OwnedPath(path) => Message::new("services.editor.errors.ownedPath").with("path", path.clone()),
    ServiceError::DuplicateOwnedPath(path) => {
      Message::new("services.editor.errors.duplicateOwnedPath").with("path", path.clone())
    }
    other => service_error(other),
  }
}
