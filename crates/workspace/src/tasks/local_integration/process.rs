use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  id::RunId,
  integration::local::{LocalError, LocalProcessStore, ProcessIdentity, ProcessJournal},
};
use orchester_store::ProjectDb;

use super::{IntegrationError, LocalIntegration};

struct Journal {
  root: PathBuf,
  run: RunId,
  authorize: Option<super::LocalAuthorization>,
  _store_owner: Arc<orchester_exec::local_git::owned::StoreOwnership>,
}

impl ProcessJournal for Journal {
  fn begin(&self) -> Result<u64, LocalError> {
    if let Some(authorize) = &self.authorize {
      authorize()?;
    }
    open_db(&self.root)
      .and_then(|mut db| db.begin_local_process(self.run))
      .map_err(|_| LocalError::Reconcile)
  }
  fn started(&self, sequence: u64, identity: Option<&ProcessIdentity>) -> Result<(), LocalError> {
    ProjectDb::open(&self.root)
      .and_then(|mut db| db.local_process_started(self.run, sequence, identity))
      .map_err(|_| LocalError::Reconcile)
  }
  fn finished(&self, sequence: u64) -> Result<(), LocalError> {
    ProjectDb::open(&self.root)
      .and_then(|mut db| db.finish_local_process(self.run, sequence))
      .map_err(|_| LocalError::Reconcile)
  }
}

impl LocalIntegration {
  pub(super) fn bind_process_journal(&mut self) -> Result<(), IntegrationError> {
    if let Some(process) = self.db.local_process(self.run.id())?
      && !process.finished
    {
      let identity = process.identity.ok_or(LocalError::Reconcile)?;
      if !orchester_exec::local_git::family::reconcile_stopped(&identity)? {
        return Err(LocalError::ProcessUncertain.into());
      }
      // Parent absence alone never reconciles descendants. Legacy receipts stay visibly uncertain.
      self.ensure_authorized()?;
      self.db.finish_local_process(self.run.id(), process.sequence)?;
    }
    let journal: Arc<dyn ProcessJournal> = Arc::new(Journal {
      root: self.main.root().to_path_buf(),
      run: self.run.id(),
      authorize: self.authorization.clone(),
      _store_owner: self._store_owner.clone(),
    });
    self.main.bind_journal(journal.clone());
    if let Some(task) = &mut self.task {
      task.bind_journal(journal);
    }
    Ok(())
  }
}
fn open_db(root: &std::path::Path) -> Result<ProjectDb, orchester_store::StoreError> {
  orchester_exec::local_git::owned_store(root)
    .map_err(|_| orchester_store::StoreError::Unrepresentable("local checkout ownership"))?;
  ProjectDb::open(root)
}
