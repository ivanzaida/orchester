//! The `docker` CLI, for the database containers of a run's services (`docs/architecture/agent-testing.md`,
//! "Service › per-run data"). `docker` is looked up in the folders of a `PATH` as a service's program is (on Windows
//! with its program extensions), so on macOS any Docker that puts its CLI there works: Docker Desktop, OrbStack or
//! colima. It runs with the runner's own environment, as git does, so its contexts, `DOCKER_HOST` and credential
//! helpers are its own. Before a container starts, the engine must answer `docker version` within
//! [`ANSWER_TIMEOUT`]; when `docker` is missing or its engine does not answer, the start fails at once with
//! [`DockerUnavailable`], which names what was checked, and never waits on it longer.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::invocation::Platform,
  resource::CommandProgram,
};
use thiserror::Error;

use crate::ProcessRunner;

/// How long the engine may take to answer `docker version`, or a container to be looked at or removed.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(15);
/// How long pulling an image the computer does not have yet may take.
pub const PULL_TIMEOUT: Duration = Duration::from_secs(600);
/// The most characters of Docker's answer an error shows.
const MAX_ANSWER_CHARS: usize = 300;

/// Why Docker cannot start a container on this computer, naming what was checked.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DockerUnavailable {
  #[error("Docker is not available: no `docker` program was found in the folders of the PATH ({})", folders(.searched))]
  NotFound { searched: Vec<PathBuf> },
  #[error("Docker is not available: `{} version` did not answer within {} s", .program.display(), .waited.as_secs())]
  NoAnswer { program: PathBuf, waited: Duration },
  /// The CLI is there, but its engine is not running or refused: Docker Desktop stopped, a socket that is not there.
  #[error("Docker is not available: the Docker engine did not answer `{} version`: {answer}", .program.display())]
  NotRunning { program: PathBuf, answer: String },
}

fn folders(searched: &[PathBuf]) -> String {
  if searched.is_empty() {
    return "the PATH is empty".to_owned();
  }
  let shown: Vec<String> = searched.iter().map(|folder| folder.display().to_string()).collect();
  shown.join(", ")
}

/// Why `docker` could not look at an image or a container, or remove one.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ContainerError {
  #[error("`docker {action}` did not finish within {} s", .waited.as_secs())]
  TimedOut { action: String, waited: Duration },
  #[error("`docker {action}` could not be run: {why}")]
  NotRun { action: String, why: String },
  #[error("`docker {action}` failed: {answer}")]
  Failed { action: String, answer: String },
  /// The image's configuration lists no TCP port, so Orchester has none to publish.
  #[error("the image `{image}` exposes no TCP port, so its container has no port to publish")]
  NoExposedPort { image: String },
}

/// The `docker` CLI found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Docker {
  program: PathBuf,
}

impl Docker {
  /// `docker` in the folders of `path`, the `PATH` a service's program is looked up in; its engine is not asked.
  pub fn find(path: Option<&OsStr>) -> Result<Self, DockerUnavailable> {
    let program = CommandProgram::new("docker").expect("`docker` is a valid program name");
    let candidates = program.candidates(Path::new(""), path, Platform::current());
    match candidates.into_iter().find(|candidate| candidate.is_file()) {
      Some(program) => Ok(Self { program }),
      None => Err(DockerUnavailable::NotFound {
        searched: path
          .map(|path| {
            std::env::split_paths(path)
              .filter(|folder| folder.is_absolute())
              .collect()
          })
          .unwrap_or_default(),
      }),
    }
  }

  /// [`Self::find`], once its engine answered `docker version` within [`ANSWER_TIMEOUT`].
  pub fn ready(path: Option<&OsStr>) -> Result<Self, DockerUnavailable> {
    let docker = Self::find(path)?;
    let asked = docker.command(["version", "--format", "{{.Server.Version}}"]);
    let program = docker.program.clone();
    match ProcessRunner::new(ANSWER_TIMEOUT).run(&asked) {
      Ok(output) if output.success() && !output.stdout.trim().is_empty() => Ok(docker),
      Ok(output) => Err(DockerUnavailable::NotRunning {
        program,
        answer: answer_of(&output),
      }),
      Err(CommandError::TimedOut) => Err(DockerUnavailable::NoAnswer {
        program,
        waited: ANSWER_TIMEOUT,
      }),
      Err(error) => Err(DockerUnavailable::NotRunning {
        program,
        answer: error.to_string(),
      }),
    }
  }

  /// The `docker` program file.
  pub fn program(&self) -> &Path {
    &self.program
  }

  /// `docker` with `arguments`, in the runner's own environment.
  pub fn command<S: Into<String>>(&self, arguments: impl IntoIterator<Item = S>) -> Command {
    Command::new(&self.program, arguments)
  }

  /// The lowest TCP port `image` exposes, pulled first when the computer does not have it yet.
  pub fn exposed_port(&self, image: &str) -> Result<u16, ContainerError> {
    let inspect = ["image", "inspect", "--format", "{{json .Config.ExposedPorts}}", image];
    let output = match self.run(&inspect, ANSWER_TIMEOUT) {
      Ok(output) => output,
      Err(ContainerError::Failed { .. }) => {
        self.run(&["pull", "--quiet", image], PULL_TIMEOUT)?;
        self.run(&inspect, ANSWER_TIMEOUT)?
      }
      Err(error) => return Err(error),
    };
    lowest_tcp_port(&output.stdout).ok_or_else(|| ContainerError::NoExposedPort {
      image: image.to_owned(),
    })
  }

  /// Removes the container `name` with its anonymous volumes, stopping it first; one that is not there is removed.
  pub fn remove(&self, name: &str) -> Result<(), ContainerError> {
    match self.run(&["rm", "--force", "--volumes", name], ANSWER_TIMEOUT) {
      Err(ContainerError::Failed { answer, .. }) if answer.contains("No such container") => Ok(()),
      other => other.map(drop),
    }
  }

  /// The names of the containers, running or not, whose label `label` is `value`.
  pub fn labelled(&self, label: &str, value: &str) -> Result<Vec<String>, ContainerError> {
    let filter = format!("label={label}={value}");
    let output = self.run(
      &["ps", "--all", "--filter", &filter, "--format", "{{.Names}}"],
      ANSWER_TIMEOUT,
    )?;
    Ok(
      output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  }

  fn run(&self, arguments: &[&str], timeout: Duration) -> Result<CommandOutput, ContainerError> {
    let action = arguments.iter().take(2).copied().collect::<Vec<_>>().join(" ");
    match ProcessRunner::new(timeout).run(&self.command(arguments.iter().copied())) {
      Ok(output) if output.success() => Ok(output),
      Ok(output) => Err(ContainerError::Failed {
        action,
        answer: answer_of(&output),
      }),
      Err(CommandError::TimedOut) => Err(ContainerError::TimedOut {
        action,
        waited: timeout,
      }),
      Err(error) => Err(ContainerError::NotRun {
        action,
        why: error.to_string(),
      }),
    }
  }
}

/// The lowest TCP port of `exposed`, an image's `.Config.ExposedPorts` as JSON (`{"5432/tcp":{}}`, or `null`).
pub fn lowest_tcp_port(exposed: &str) -> Option<u16> {
  let ports: serde_json::Map<String, serde_json::Value> = serde_json::from_str(exposed.trim()).ok()?;
  ports
    .keys()
    .filter_map(|port| {
      let (number, protocol) = port.split_once('/').unwrap_or((port, "tcp"));
      (protocol == "tcp").then(|| number.parse::<u16>().ok()).flatten()
    })
    .filter(|port| *port != 0)
    .min()
}

/// The first line of what `docker` said on standard error, else on standard output, shortened; or its exit code.
fn answer_of(output: &CommandOutput) -> String {
  let said = [&output.stderr, &output.stdout]
    .into_iter()
    .find_map(|text| text.lines().map(str::trim).find(|line| !line.is_empty()));
  match said {
    Some(line) if line.chars().count() > MAX_ANSWER_CHARS => {
      format!("{}…", line.chars().take(MAX_ANSWER_CHARS).collect::<String>())
    }
    Some(line) => line.to_owned(),
    None => match output.code {
      Some(code) => format!("it exited with {code} and said nothing"),
      None => "it was ended and said nothing".to_owned(),
    },
  }
}
