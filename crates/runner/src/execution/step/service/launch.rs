//! Starting one service: its program found as a command tool's is, its placeholders filled with its own port and URL
//! and those of the services already running, its secrets read from the keychain, its tree started and held, its
//! output kept in a fresh log, and the wait until it is ready. A start that fails after the program started stops the
//! whole tree first, and its error says so with the last lines of its output.

use std::{path::Path, sync::Arc, time::Instant};

use orchester_core::{
  command::SecretVariable,
  connection::invocation::Platform,
  resource::{ResourceBody, VariableName},
  secret::SecretName,
  service::{RunValues, Service, ServiceLog, ServicePhase, Stack},
  store::{ResourceStore, SecretStore},
};
use orchester_exec::process_tree::ProcessTree;
use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;

use super::{Running, StepServices, Watch, order::Address, readiness};

/// How many of its last lines of output a failed start's error shows.
const FAILED_START_LINES: u32 = 20;

/// The latest published version of each of the project's services at `root`, removed ones left out.
pub(in crate::execution::step) fn published(root: &Path) -> Result<Vec<Service>, StoreError> {
  latest(root)
}

/// The latest published version of each of the project's stacks at `root`, removed ones left out.
pub(in crate::execution::step) fn published_stacks(root: &Path) -> Result<Vec<Stack>, StoreError> {
  latest(root)
}

/// The latest published body of each resource of kind `T` at `root`, removed ones left out.
fn latest<T: ResourceBody + Clone>(root: &Path) -> Result<Vec<T>, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  let mut bodies = Vec::new();
  for resource in db.resources(T::KIND)? {
    bodies.extend(
      db.latest_version::<T>(resource.id())?
        .map(|version| version.body().clone()),
    );
  }
  Ok(bodies)
}

impl StepServices {
  /// Starts `service` with the addresses of the services already running, `wired`, and waits until it is ready, giving
  /// up when `ended` says the invocation ended.
  pub(super) fn launch(
    &self,
    service: &Service,
    wired: &[Address],
    ended: &dyn Fn() -> bool,
  ) -> Result<Running, String> {
    let name = service.name();
    let lease = self
      .services
      .ports
      .lease()
      .map_err(|error| format!("no free loopback port for the service `{name}`: {error}"))?;
    let port = lease.port();
    let url = format!("http://127.0.0.1:{port}");
    let values = wired
      .iter()
      .fold(RunValues::new(self.run, port, &url), |values, (other, port, url)| {
        values.with_service(other.clone(), *port, url)
      });
    let (mut tree, secrets) = self.spawn(service, &values)?;
    let log = Arc::new(Mutex::new(ServiceLog::new(secrets)));
    self.state.watched.lock().insert(
      name.clone(),
      Watch {
        log: Arc::clone(&log),
        port,
        url: url.clone(),
        started: Instant::now(),
        phase: ServicePhase::Starting,
        exit: None,
      },
    );
    let ready = readiness::wait(&mut tree, (service, &values), port, (&log, ended));
    match ready {
      Ok(()) => Ok(Running { tree, url, lease }),
      Err(why) => {
        // A program that cannot be watched any more is stopped all the same; its status then says stopped.
        let exit = tree.exited().unwrap_or(None).map(|status| status.code());
        self.state.note(name, ServicePhase::Stopped, exit);
        let stopped = match tree.stop() {
          Ok(()) => "it was stopped with everything it started".to_owned(),
          Err(error) => format!("stopping it with everything it started failed: {error}"),
        };
        let output = log.lock().excerpt(FAILED_START_LINES, 0);
        Err(format!(
          "the service `{name}` {why}; {stopped}. Its last output: {output}"
        ))
      }
    }
  }

  /// Starts `service`'s program with `values`, held with everything it starts; and the secrets it was given.
  fn spawn(&self, service: &Service, values: &RunValues) -> Result<(ProcessTree, Vec<SecretVariable>), String> {
    let name = service.name();
    let folder = service.folder_in(&self.worktree);
    if !folder.is_dir() {
      return Err(format!(
        "the working folder `{}` of the service `{name}` is not in the worktree",
        service.working_folder().as_str()
      ));
    }
    let platform = Platform::current();
    let inherited_path = self
      .inherited
      .iter()
      .find(|(variable, _)| variable.to_string_lossy().eq_ignore_ascii_case("PATH"))
      .map(|(_, value)| value.clone());
    let path = service.search_path(&self.project, inherited_path);
    let program = service
      .program_candidates(&self.worktree, path.as_deref(), platform)
      .into_iter()
      .find(|candidate| candidate.is_file())
      .ok_or_else(|| {
        format!(
          "the program `{}` of the service `{name}` was not found",
          service.program().as_str()
        )
      })?;
    service.program().check_place(&program, &self.root).map_err(|_| {
      format!("the program of the service `{name}` now leads into the project's repository or off this computer")
    })?;
    let process = service
      .process(
        (&program, &self.worktree),
        (self.inherited.iter().cloned(), &self.project),
        platform,
        values,
      )
      .map_err(|error| format!("the service `{name}` cannot be started: {error}"))?;
    let mut command = process.command;
    let mut secrets = Vec::new();
    for (variable, secret) in &process.secrets {
      let secret = self.secret(variable, secret)?;
      command = command.with_secret_env(secret.clone());
      secrets.push(secret);
    }
    let tree = ProcessTree::start(&command).map_err(|error| format!("the service `{name}` did not start: {error}"))?;
    Ok((tree, secrets))
  }

  /// The value of the project secret `secret` for the variable `variable`, read from the keychain.
  fn secret(&self, variable: &VariableName, secret: &SecretName) -> Result<SecretVariable, String> {
    let missing = || format!("the variable `{variable}` names the secret `{secret}`, which the project does not have");
    let db = ProjectDb::open_read_only(&self.root).map_err(|error| format!("the secrets cannot be read: {error}"))?;
    let found = db
      .secrets()
      .map_err(|error| format!("the secrets cannot be read: {error}"))?
      .into_iter()
      .find(|found| found.name() == secret)
      .ok_or_else(missing)?;
    if found.is_archived() {
      return Err(format!(
        "the variable `{variable}` names the archived secret `{secret}`"
      ));
    }
    let value = self
      .vault
      .read(&found.vault_key(db.project().id()))
      .map_err(|error| format!("the secret `{secret}` cannot be read from the keychain: {error}"))?;
    Ok(SecretVariable::new(variable.as_str(), Arc::new(value)))
  }
}
