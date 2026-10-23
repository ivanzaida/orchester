//! A shared project's modules (`docs/architecture/server.md`, "Modules"): a module is made and changed on the server,
//! under its revision, and which module a task is in changes with the task's revision; the cache takes what the server
//! confirmed. A task's module, like its organization, is put on the task as it is now when it changed meanwhile, and
//! sent again without asking. The server's refusals are core's module errors, said as they are for a local project.
//! While the server cannot be reached nothing is sent and the change fails at once (until the outbox, S4).

use orchester_api::{
  ErrorCode, ModuleCreate, ModuleFields, ModulePatch, Modules, SharedModule, SharedTask, SharingErrorDetails,
  TaskChanges,
};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, TaskId},
  module::{Module, ModuleDetails, ModuleError},
  task::{TaskUpdate, organization::ChangeActor},
};
use orchester_store::StoreError;

use super::{convert, store::RemoteTaskStore};
use crate::sharing::SharingError;

/// How many times a move is put on a task that changed meanwhile before Orchester stops.
const ATTEMPTS: usize = 3;

/// The module a refused request was about: the one its path names, or the one a task was to move to.
#[derive(Clone, Copy)]
enum Asked {
  New,
  Module(ModuleId),
  MoveTo(Option<ModuleId>),
}

impl RemoteTaskStore {
  /// A new module of `details`, made on the server with an id made here.
  pub fn create_module(&mut self, details: &ModuleDetails) -> Result<Module, SharingError> {
    self.server.writable()?;
    let create = ModuleCreate {
      idempotency_key: IdempotencyKey::generate(),
      id: ModuleId::generate(),
      module: ModuleFields::from_core(details),
    };
    let answer = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.create_module(&token, project, &create).await }),
    );
    let made = answer.map_err(|error| self.refusal(error, Asked::New))?;
    self.cache_module(&made)
  }

  /// Replaces the details of module `id`, read as `base`, with `details`; refused when they changed since, here or on
  /// the server.
  pub fn update_module(
    &mut self,
    id: ModuleId,
    base: &ModuleDetails,
    details: &ModuleDetails,
  ) -> Result<Module, SharingError> {
    let missing = || SharingError::from(StoreError::from(ModuleError::NotFound(id)));
    let cached = self.db.module(id)?.ok_or_else(missing)?;
    if cached.details() != base {
      return Err(StoreError::from(ModuleError::ChangedMeanwhile).into());
    }
    let revision = self.db.module_revision(id)?.ok_or_else(missing)?;
    self.server.writable()?;
    let patch = ModulePatch {
      idempotency_key: IdempotencyKey::generate(),
      revision,
      module: ModuleFields::from_core(details),
    };
    let answer = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.update_module(&token, project, id, &patch).await }),
    );
    let changed = answer.map_err(|error| self.refusal(error, Asked::Module(id)))?;
    self.cache_module(&changed)
  }

  /// Puts `task` in `module`, or in none, as `actor` asked; whether it moved. A task that changed meanwhile on the
  /// server is moved as it is now.
  pub fn set_task_module(
    &mut self,
    task: TaskId,
    module: Option<ModuleId>,
    actor: ChangeActor,
  ) -> Result<bool, SharingError> {
    if let Some(module) = module
      && self.db.module(module)?.is_none()
    {
      return Err(StoreError::from(ModuleError::NotFound(module)).into());
    }
    for _ in 0..ATTEMPTS {
      let current = self.cached(task)?;
      if current.module() == module {
        return Ok(false);
      }
      let changes = TaskChanges {
        module: Some(module),
        actor: Some(actor),
        ..TaskChanges::default()
      };
      let update = self
        .patch(task, current.revision(), changes)
        .map_err(|error| self.refusal(error, Asked::MoveTo(module)))?;
      if let TaskUpdate::Saved(_) = update {
        self.refresh_modules()?;
        return Ok(true);
      }
    }
    Err(SharingError::KeptChanging)
  }

  /// Reads the project's modules and the moves after the last one the cache has into the cache. The tasks the moves
  /// are of are in the cache already.
  pub(super) fn refresh_modules(&mut self) -> Result<(), SharingError> {
    let read = self.read_modules()?;
    self.cache_modules(&read)?;
    let changes: Vec<_> = read.changes.iter().map(convert::cached_change).collect();
    self.db.cache_module_changes(&changes)?;
    Ok(())
  }

  /// Reads the project's modules into the cache when `tasks` name one it does not have: another member made it since.
  /// Their moves are read with the tasks' next refresh.
  pub(super) fn ensure_modules_of(&mut self, tasks: &[SharedTask]) -> Result<(), SharingError> {
    for module in tasks.iter().filter_map(|task| task.content.module) {
      if self.db.module(module)?.is_none() {
        let read = self.read_modules()?;
        return self.cache_modules(&read);
      }
    }
    Ok(())
  }

  fn read_modules(&self) -> Result<Modules, SharingError> {
    let after = self.db.last_module_change()?;
    self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.modules(&token, project, after).await }),
    )
  }

  fn cache_modules(&mut self, read: &Modules) -> Result<(), SharingError> {
    let modules = read
      .modules
      .iter()
      .map(convert::cached_module)
      .collect::<Result<Vec<_>, _>>()?;
    self.db.cache_shared_modules(&modules)?;
    Ok(())
  }

  fn cache_module(&mut self, module: &SharedModule) -> Result<Module, SharingError> {
    let cached = convert::cached_module(module)?;
    self.db.cache_shared_modules(std::slice::from_ref(&cached))?;
    Ok(cached.module)
  }

  /// The server's refusal of what was `asked` as core's module error, as a local project says it; a module that changed
  /// meanwhile is cached as it is now. Every other error stays as it is.
  fn refusal(&mut self, error: SharingError, asked: Asked) -> SharingError {
    let details = match &error {
      SharingError::Client(client) => client.details().cloned(),
      _ => None,
    };
    let refused = match (error.code(), details, asked) {
      (Some(ErrorCode::ModuleExists), ..) => ModuleError::DuplicateName,
      (Some(ErrorCode::RevisionConflict), Some(SharingErrorDetails::ModuleRevision { current }), _) => {
        if let Err(cache) = self.cache_module(&current) {
          return cache;
        }
        ModuleError::ChangedMeanwhile
      }
      (Some(ErrorCode::Module), Some(SharingErrorDetails::Reason { reason }), _) if reason == "target_before_start" => {
        ModuleError::TargetBeforeStart
      }
      (Some(ErrorCode::Module), _, Asked::MoveTo(Some(module)))
      | (Some(ErrorCode::NotFound), _, Asked::Module(module)) => ModuleError::NotFound(module),
      _ => return error,
    };
    StoreError::from(refused).into()
  }
}
