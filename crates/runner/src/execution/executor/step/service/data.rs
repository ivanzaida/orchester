//! A service's per-run data as its start prepares it (`orchester_core::service::run_data`): its run folder made in the
//! run's storage before it starts, which it gets as `{{data}}`; or, for a fresh database container, Docker checked,
//! the image's port read and the container started through `docker` on the service's own port. The container lives
//! as long as the service: [`Container`] removes it once the service's tree stopped, and the run's storage notes it
//! meanwhile, so what is left when the runner dies is removed on its next start (`crate::execution::run_data`).
//!
//! Docker that is missing or whose engine does not answer fails the start at once with
//! [`orchester_exec::docker::DockerUnavailable`], before a port is taken or anything starts.

use std::{ffi::OsString, fs, io, path::PathBuf};

use orchester_core::{
  command::SecretVariable,
  service::{ContainerName, ContainerStart, RunData, RunValues, Service, StartFailureCause, container_notes_folder},
};
use orchester_exec::{
  docker::{Docker, DockerUnavailable},
  process_tree::ProcessTree,
};

use super::{Failed, StepServices};

/// What a service's per-run data gives its start.
pub(super) enum Data {
  None,
  /// Its run folder, made.
  Folder(PathBuf),
  /// Docker, which answered, and the port the image exposes.
  Container {
    docker: Docker,
    port: u16,
  },
}

/// A service's container while it may run: dropping it removes the container and then its note.
pub(super) struct Container {
  docker: Docker,
  name: ContainerName,
  note: PathBuf,
}

impl Drop for Container {
  fn drop(&mut self) {
    let name = self.name.as_str();
    if let Err(error) = self.docker.remove(name) {
      tracing::warn!(%error, container = name, "a service's container was not removed; the run's end tries again");
      return;
    }
    if let Err(error) = fs::remove_file(&self.note)
      && error.kind() != io::ErrorKind::NotFound
    {
      tracing::warn!(%error, note = %self.note.display(), "a removed container's note stays");
    }
  }
}

impl StepServices {
  /// Prepares `service`'s per-run data before it starts: its run folder made, or Docker asked and its image's port
  /// read (the image pulled first when the computer does not have it). Docker that is not available is a failure the
  /// person is shown.
  pub(super) fn prepare_data(&self, service: &Service) -> Result<Data, Failed> {
    match service.run_data() {
      RunData::None => Ok(Data::None),
      RunData::RunFolder(folder) => {
        let path = folder.path_in(&self.root, self.run);
        fs::create_dir_all(&path)
          .map_err(|error| format!("its run folder {} cannot be made: {error}", path.display()))?;
        Ok(Data::Folder(path))
      }
      RunData::DatabaseContainer => {
        let docker = Docker::ready(self.docker_path().as_deref())
          .map_err(|error| Failed::shown(error.to_string(), docker_cause(&error)))?;
        let image = service.program().as_str();
        let port = docker.exposed_port(image).map_err(|error| error.to_string())?;
        Ok(Data::Container { docker, port })
      }
    }
  }

  /// The `PATH` `docker` is looked up in: the one the project adds, else the runner's. A service's own `PATH` is the
  /// container's.
  fn docker_path(&self) -> Option<OsString> {
    self
      .project
      .get("PATH")
      .map(OsString::from)
      .or_else(|| self.inherited_path())
  }

  /// Starts `service`'s container with `values` through `docker`, the image's port `container_port` published on
  /// `host_port`; its tree, the secrets it was given and the container. A container of the same name that a runner
  /// which died left is removed first: no service of this runner holds its port.
  pub(super) fn spawn_container(
    &self,
    service: &Service,
    values: &RunValues,
    (docker, container_port): (Docker, u16),
    host_port: u16,
  ) -> Result<(ProcessTree, Vec<SecretVariable>, Container), String> {
    let name = ContainerName::of(self.run, service.name(), host_port);
    docker
      .remove(name.as_str())
      .map_err(|error| format!("a container of the same name cannot be removed: {error}"))?;
    let start = ContainerStart {
      name: &name,
      project: self.project_id,
      run: self.run,
      host_port,
      container_port,
    };
    let process = service
      .container_process(&start, values)
      .map_err(|error| format!("its container cannot be started: {error}"))?;
    let mut command = docker.command(process.arguments);
    let mut secrets = Vec::new();
    for (variable, secret) in &process.secrets {
      let secret = self.secret(variable, secret)?;
      command = command.with_secret_env(secret.clone());
      secrets.push(secret);
    }
    let notes = container_notes_folder(&self.root, self.run);
    let note = notes.join(name.as_str());
    fs::create_dir_all(&notes)
      .and_then(|()| fs::write(&note, ""))
      .map_err(|error| format!("its container cannot be noted in the run's storage: {error}"))?;
    // Held before `docker run` starts, so a start that fails removes whatever it created.
    let container = Container { docker, name, note };
    let tree =
      ProcessTree::start(&command).map_err(|error| format!("its container did not start through docker: {error}"))?;
    Ok((tree, secrets, container))
  }
}

/// What the conversation notes of Docker that is not available: missing, or there and not answering.
fn docker_cause(error: &DockerUnavailable) -> StartFailureCause {
  let detail = error.to_string();
  match error {
    DockerUnavailable::NotFound { .. } => StartFailureCause::DockerMissing { detail },
    DockerUnavailable::NoAnswer { .. } | DockerUnavailable::NotRunning { .. } => {
      StartFailureCause::DockerNotAnswering { detail }
    }
  }
}
