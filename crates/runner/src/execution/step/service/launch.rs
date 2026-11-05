//! Starting one service: its per-run data prepared ([`super::data`]), its program found as a command tool's is (or,
//! for a database container, `docker` run with its image), its placeholders filled with its own port and URL, its run
//! folder and those of the services already running, its secrets and those of its seed's headers read from the
//! keychain, its tree started and held, its output kept in a fresh log, the wait until it is ready, and its seed
//! ([`super::seed`]). A start that fails after the program started stops the whole tree first, and its error says so
//! with the last lines of its output; its container is removed after.

use std::{ffi::OsString, path::Path, sync::Arc, time::Instant};

use orchester_core::{
  command::SecretVariable,
  connection::invocation::Platform,
  resource::{ResourceBody, VariableName},
  secret::{SecretName, SecretValue, SecretVault},
  service::{RunValues, Service, ServiceLog, ServicePhase, Stack},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_exec::process_tree::ProcessTree;
use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;

use super::{
  Running, StepServices, Watch,
  data::{Container, Data},
  order::Address,
  readiness,
};

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
  /// Starts `service` with the addresses of the services already running, `wired`, waits until it is ready and runs
  /// its seed, giving up when `ended` says the invocation ended.
  pub(super) fn launch(
    &self,
    service: &Service,
    wired: &[Address],
    ended: &dyn Fn() -> bool,
  ) -> Result<Running, String> {
    let name = service.name();
    let cannot = |why: String| format!("the service `{name}` cannot be started: {why}");
    let data = self.prepare_data(service).map_err(cannot)?;
    let headers = self.header_secrets(service).map_err(cannot)?;
    let lease = self
      .services
      .ports
      .lease()
      .map_err(|error| format!("no free loopback port for the service `{name}`: {error}"))?;
    let port = lease.port();
    let url = format!("http://127.0.0.1:{port}");
    let mut values = wired
      .iter()
      .fold(RunValues::new(self.run, port, &url), |values, (other, port, url)| {
        values.with_service(other.clone(), *port, url)
      });
    if let Data::Folder(folder) = &data {
      values = values.with_data(&folder.to_string_lossy());
    }
    let (mut tree, mut secrets, container) = self.start_program(service, &values, data, port)?;
    secrets.extend(headers.iter().map(|(_, secret)| secret.clone()));
    let log = Arc::new(Mutex::new(ServiceLog::new(secrets.clone())));
    let origin = self.state.origin.lock().clone();
    self.state.watched.lock().insert(
      name.clone(),
      Watch {
        log: Arc::clone(&log),
        port,
        url: url.clone(),
        started: Instant::now(),
        started_at: Timestamp::now(),
        ended_at: None,
        origin,
        phase: ServicePhase::Starting,
        exit: None,
        stopped_for: None,
      },
    );
    let ready = readiness::wait(&mut tree, (service, &values), port, (&log, ended)).and_then(|output| {
      self
        .seed(service, &mut values, (&headers, &secrets), (&log, ended))
        .map(|captured| (output, captured))
    });
    match ready {
      Ok((output, captured)) => Ok(Running {
        tree,
        _container: container,
        url,
        lease,
        captured,
        needs: service.needs().to_vec(),
        output,
      }),
      Err(why) => {
        // A program that cannot be watched any more is stopped all the same; its status then says stopped.
        let exit = tree.exited().unwrap_or(None).map(|status| status.code());
        self.state.note(name, ServicePhase::Stopped, exit);
        let stopped = match tree.stop() {
          Ok(()) => "it was stopped with everything it started".to_owned(),
          Err(error) => format!("stopping it with everything it started failed: {error}"),
        };
        // Its container goes once the tree is stopped.
        drop(container);
        let output = log.lock().excerpt(FAILED_START_LINES, 0);
        Err(format!(
          "the service `{name}` {why}; {stopped}. Its last output: {output}"
        ))
      }
    }
  }

  /// Starts `service`'s program with `values` on `port`, as its per-run data `data` says: its own program, or its
  /// container through `docker`. Its tree, the secrets it was given and its container.
  fn start_program(
    &self,
    service: &Service,
    values: &RunValues,
    data: Data,
    port: u16,
  ) -> Result<(ProcessTree, Vec<SecretVariable>, Option<Container>), String> {
    match data {
      Data::Container { docker, port: exposed } => {
        let (tree, secrets, container) = self
          .spawn_container(service, values, (docker, exposed), port)
          .map_err(|why| format!("the service `{}` cannot be started: {why}", service.name()))?;
        Ok((tree, secrets, Some(container)))
      }
      Data::None | Data::Folder(_) => {
        let (tree, secrets) = self.spawn(service, values)?;
        Ok((tree, secrets, None))
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
    let path = service.search_path(&self.project, self.inherited_path());
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

  /// The `PATH` of the environment the runner runs in.
  pub(super) fn inherited_path(&self) -> Option<OsString> {
    self
      .inherited
      .iter()
      .find(|(variable, _)| variable.to_string_lossy().eq_ignore_ascii_case("PATH"))
      .map(|(_, value)| value.clone())
  }

  pub(super) fn secret(&self, variable: &VariableName, secret: &SecretName) -> Result<SecretVariable, String> {
    project_secret((&self.root, self.vault.as_ref()), variable, secret)
  }
}

/// The value of the secret `secret` of the project at `root` for the variable `variable`, read from `vault`, the
/// keychain.
pub(in crate::execution::step) fn project_secret(
  (root, vault): (&Path, &dyn SecretVault),
  variable: &VariableName,
  secret: &SecretName,
) -> Result<SecretVariable, String> {
  let value = secret_value((root, vault), &format!("the variable `{variable}`"), secret)?;
  Ok(SecretVariable::new(variable.as_str(), value))
}

/// The value of the secret `secret` of the project at `root`, which `user` (e.g. "the variable `API_TOKEN`") names,
/// read from `vault`, the keychain.
pub(super) fn secret_value(
  (root, vault): (&Path, &dyn SecretVault),
  user: &str,
  secret: &SecretName,
) -> Result<Arc<SecretValue>, String> {
  let missing = || format!("{user} names the secret `{secret}`, which the project does not have");
  let db = ProjectDb::open_read_only(root).map_err(|error| format!("the secrets cannot be read: {error}"))?;
  let found = db
    .secrets()
    .map_err(|error| format!("the secrets cannot be read: {error}"))?
    .into_iter()
    .find(|found| found.name() == secret)
    .ok_or_else(missing)?;
  if found.is_archived() {
    return Err(format!("{user} names the archived secret `{secret}`"));
  }
  let value = vault
    .read(&found.vault_key(db.project().id()))
    .map_err(|error| format!("the secret `{secret}` cannot be read from the keychain: {error}"))?;
  Ok(Arc::new(value))
}
