//! Where a run's per-run data lives and how its database containers start (`docs/architecture/agent-testing.md`,
//! "Service › per-run data"). A run folder lies in the run's data folder in its storage, made before the service
//! starts and deleted with the rest of that folder when the run ends. A service whose per-run data is a fresh
//! database container *is* that container: its program names the image, its arguments go to the container's command,
//! its variables become the container's environment, and the image's lowest exposed TCP port is published on the
//! service's own loopback port, so `{{port}}` and `{{services.<name>.port}}` reach it. Each container has a name of
//! its run, service and port and carries labels naming its project, run and service; the run's storage notes each
//! one started, so what a runner left when it died is found and removed on its next start.

use std::path::{Path, PathBuf};

use super::{PlaceholderError, RunFolder, RunValues, Service, ServiceName, ServiceValue};
use crate::{
  id::{ProjectId, RunId},
  resource::VariableName,
  secret::SecretName,
  workspaces::artifacts_folder,
};

/// The label of a container that names its project's id.
pub const CONTAINER_PROJECT_LABEL: &str = "orchester.project";
/// The label of a container that names its run's id.
pub const CONTAINER_RUN_LABEL: &str = "orchester.run";
/// The label of a container that names its service.
pub const CONTAINER_SERVICE_LABEL: &str = "orchester.service";

/// The folder of the run's storage that holds its services' run folders: `.orchester/runs/<run>/data/` in the
/// repository at `repo`. It is deleted when the run ends.
pub fn run_data_folder(repo: &Path, run: RunId) -> PathBuf {
  artifacts_folder(repo, run).join("data")
}

/// The folder of the run's storage that notes, one empty file each, the containers its services started and that
/// were not removed yet: `.orchester/runs/<run>/containers/`.
pub fn container_notes_folder(repo: &Path, run: RunId) -> PathBuf {
  artifacts_folder(repo, run).join("containers")
}

impl RunFolder {
  /// Where this folder lies for `run` in the repository at `repo`.
  pub fn path_in(&self, repo: &Path, run: RunId) -> PathBuf {
    let mut path = run_data_folder(repo, run);
    path.extend(self.as_str().split('/'));
    path
  }
}

/// The name of a run's container of one service, started on one port: `orchester-<run>-<service>-<port>`. No two
/// services of a runner hold one port at once, so a start's name is never that of a container another start left.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContainerName(String);

impl ContainerName {
  pub fn of(run: RunId, service: &ServiceName, port: u16) -> Self {
    Self(format!("orchester-{run}-{service}-{port}"))
  }

  /// `value`, a name read from the run's notes, when it is one [`Self::of`] gives a container of `run`: a note is a
  /// file anyone may write, and only Orchester's own containers are removed.
  pub fn noted(run: RunId, value: &str) -> Option<Self> {
    let (service, port) = value.strip_prefix(&format!("orchester-{run}-"))?.rsplit_once('-')?;
    let service = ServiceName::new(service).ok()?;
    let port = port.parse::<u16>().ok()?;
    let name = Self::of(run, &service, port);
    (name.0 == value).then_some(name)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What one start of a service's container is given besides the service.
#[derive(Clone, Copy, Debug)]
pub struct ContainerStart<'a> {
  pub name: &'a ContainerName,
  pub project: ProjectId,
  pub run: RunId,
  /// The service's own loopback port, which the container's port is published on.
  pub host_port: u16,
  /// The port the image exposes.
  pub container_port: u16,
}

/// The `docker` arguments that start a service's container, and the variables whose value is the secret of that
/// name: the docker program gets each in its environment, and the arguments name only the variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerProcess {
  pub arguments: Vec<String>,
  pub secrets: Vec<(VariableName, SecretName)>,
}

impl Service {
  /// The arguments of `docker` that run this service's container in the foreground, removed once it stops
  /// (`--rm`), with its name and labels, its port published on loopback only, its plain variables filled by `values`
  /// and set by value, its secret variables by name only, then its image and its filled arguments.
  pub fn container_process(
    &self,
    start: &ContainerStart<'_>,
    values: &RunValues,
  ) -> Result<ContainerProcess, PlaceholderError> {
    let mut arguments: Vec<String> = vec!["run".into(), "--rm".into(), "--name".into(), start.name.as_str().into()];
    let labels = [
      (CONTAINER_PROJECT_LABEL, start.project.to_string()),
      (CONTAINER_RUN_LABEL, start.run.to_string()),
      (CONTAINER_SERVICE_LABEL, self.name().to_string()),
    ];
    for (label, value) in labels {
      arguments.extend(["--label".to_owned(), format!("{label}={value}")]);
    }
    arguments.extend([
      "--publish".to_owned(),
      format!("127.0.0.1:{}:{}", start.host_port, start.container_port),
    ]);
    let mut secrets = Vec::new();
    for (name, value) in values.variables(self)? {
      let set = match value {
        ServiceValue::Plain(text) => format!("{}={text}", name.as_str()),
        ServiceValue::Secret(secret) => {
          let set = name.as_str().to_owned();
          secrets.push((name, secret));
          set
        }
      };
      arguments.extend(["--env".to_owned(), set]);
    }
    arguments.push(self.program().as_str().to_owned());
    arguments.extend(values.arguments(self)?);
    Ok(ContainerProcess { arguments, secrets })
  }
}
